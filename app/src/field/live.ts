// The Field's live updates (ts-0182-ad; Stage 6a, "Live updates are quiet"; note 14 of the board mock). An event from
// the daemon says that it is time to look, never what to show: the Field reads the board again (`loadField`), once for
// a burst of events, and the daemon computes the view's members, its closure and its counts as it always did. The
// board stays as it is while the read is on its way, so the selection, the scroll and the folds are not touched. The
// cards that changed flash once, and the Live stamp says when the board last changed. Nothing here is a toast.
//
// This is plain code over an `OwnerClient` and a clock the caller gives, so each rule is tested with no browser and no
// real time. `useField.ts` wires it to React.
import { CONNECTION_EVENT, type ChannelEvent, type OwnerClient } from '../owner/client.ts'
import type { ViewInput } from '../owner/generated.ts'
import { loadField } from './load.ts'
import { lanesOf } from './model.ts'
import type { FieldData } from './types.ts'

/** Events that come close together are read once, this long after the first of them. */
export const LIVE_DEBOUNCE_MS = 300

/** A card flashes for this long. The animation (`index.css`, `field-flash`) is two seconds. */
export const FLASH_MS = 2100

/**
 * How long a board that is still settling waits before it is read again. The daemon announces a task that changed, and
 * a board with no task has none to announce when its first listing ends, so a settling board is asked for again.
 */
export const SETTLE_POLL_MS = 1500

/** The daemon events that say a task of a source changed. The rest (a run, the Queue) are not the Field's. */
const TASK_EVENTS: ReadonlySet<string> = new Set([
  'task.changed',
  'task.renamed',
  'task.removed',
  'task.invalid',
  'task.written',
])

/**
 * Whether an event is a change to the board shown. An event carries the name of its source and no board, so it is the
 * board's when the board has that source: an event for a source the board does not have is another board's. Two
 * boards that both have a `plan` source cannot be told apart: one extra read, which finds nothing and shows nothing.
 * Every task event counts when nothing has been read (`sources` is null, since a read that failed may work now) and
 * while the board is `settling`, since a source that is coming up is not in `sources` yet.
 */
export function isBoardEvent(event: ChannelEvent, sources: readonly string[] | null, settling = false): boolean {
  if (event.method !== 'daemon.event') return false
  const { body } = event.params
  if (!TASK_EVENTS.has(body.event)) return false
  const source = 'source' in body ? body.source : null
  return sources === null || settling || (source !== null && sources.includes(source))
}

/**
 * The cards that a read changed: a card that is new, or in another column, or whose title, stage, status or chips
 * differ. A card that is gone is nothing to flash. Cards are named by their `id`, as the page does.
 */
export function changedCards(before: FieldData, after: FieldData, groupBy?: string | null): ReadonlySet<string> {
  const seen = new Map<string, string>()
  for (const lane of lanesOf(before, groupBy).lanes) {
    for (const card of lane.cards) seen.set(card.id, JSON.stringify([lane.column.key, card]))
  }
  const changed = new Set<string>()
  for (const lane of lanesOf(after, groupBy).lanes) {
    for (const card of lane.cards) {
      if (seen.get(card.id) !== JSON.stringify([lane.column.key, card])) changed.add(card.id)
    }
  }
  return changed
}

/** The words of the Live stamp: how long ago the board last changed. */
export function agoWords(updatedAt: number, now: number): string {
  const seconds = Math.max(0, Math.round((now - updatedAt) / 1000))
  if (seconds < 5) return 'just now'
  if (seconds < 60) return `${seconds} s ago`
  return `${Math.floor(seconds / 60)} min ago`
}

/** The clock. A test gives its own, so that nothing waits for real time. */
export interface Clock {
  setTimeout(run: () => void, ms: number): unknown
  clearTimeout(handle: unknown): void
}

export const realClock: Clock = {
  setTimeout: (run, ms) => setTimeout(run, ms),
  clearTimeout: (handle) => clearTimeout(handle as ReturnType<typeof setTimeout>),
}

/**
 * Runs `job` once for a burst of `poke`s: `ms` after the first poke of a burst, so that a stream that never ends is
 * still read every `ms`. A poke while the job runs is met with one more run after it, never several.
 */
export class Burst {
  private timer: unknown = null
  private running: Promise<void> | null = null
  private again = false
  private stopped = false

  constructor(
    private readonly clock: Clock,
    private readonly ms: number,
    private readonly job: () => Promise<void>,
  ) {}

  poke(): void {
    if (this.stopped) return
    if (this.running !== null) {
      this.again = true
      return
    }
    this.timer ??= this.clock.setTimeout(() => {
      this.timer = null
      this.start()
    }, this.ms)
  }

  /** Finishes when no job is running. A burst that is waiting for its time is not waited for. */
  async idle(): Promise<void> {
    while (this.running !== null) await this.running
  }

  stop(): void {
    this.stopped = true
    if (this.timer !== null) this.clock.clearTimeout(this.timer)
    this.timer = null
  }

  private start(): void {
    this.running = this.job()
      .catch(() => undefined)
      .finally(() => {
        this.running = null
        if (this.again) {
          this.again = false
          this.poke()
        }
      })
  }
}

/** What the Field holds after a read. */
export interface FieldRead {
  /** The board as the last read gave it, or null while the first read is on its way or when it failed. */
  readonly data: FieldData | null
  /** The words of a first read that failed. A read after the first that fails leaves the board as it was. */
  readonly problem: string | null
  /** The cards that changed in the last reads and are still flashing. */
  readonly flashed: ReadonlySet<string>
  /** When the board last changed, on the `now` of the options. Null before the first read. */
  readonly updatedAt: number | null
}

export interface FeedOptions {
  readonly owner: OwnerClient
  readonly boardId: string
  readonly view: string | null
  /** The `layout.groupBy` of the view, which decides what a card's column is. Asked at each read. */
  readonly groupBy: () => string | null
  /**
   * The view to ask the daemon to walk, with a goal's closure added (`walk.ts`, `inlineView`), or null for no walk.
   * Asked at each read, so a goal that changes is read with the next one.
   */
  readonly walk?: () => ViewInput | null
  /**
   * The expression of the filter bar, which narrows the view at every read (`filters/model.ts`). A filter that changes
   * is a new feed, so it is the one this feed was made with. Null for none.
   */
  readonly filter?: string | null
  /** Called with each new state. Never after `stop`. */
  readonly onRead: (read: FieldRead) => void
  readonly clock?: Clock
  readonly now?: () => number
  readonly debounceMs?: number
  readonly flashMs?: number
}

const words = (failure: unknown) => (failure instanceof Error ? failure.message : String(failure))

const sameData = (a: FieldData, b: FieldData) => JSON.stringify(a) === JSON.stringify(b)

/** One board and view, read once and then again when the daemon says a task changed. */
export class FieldFeed {
  private readonly options: FeedOptions
  private readonly clock: Clock
  private readonly now: () => number
  private readonly burst: Burst
  private stopListening: (() => void) | null = null
  private stopped = false
  private data: FieldData | null = null
  private flashed: ReadonlySet<string> = new Set()
  private flashTimer: unknown = null
  private settleTimer: unknown = null
  private updatedAt: number | null = null

  constructor(options: FeedOptions) {
    this.options = options
    this.clock = options.clock ?? realClock
    this.now = options.now ?? Date.now
    this.burst = new Burst(this.clock, options.debounceMs ?? LIVE_DEBOUNCE_MS, () => this.read())
  }

  /** Listens, and then reads the board. It finishes when the first read has. */
  async start(): Promise<void> {
    // Listen first, so that an event between the subscription and the read is not missed.
    let stop: () => void
    try {
      stop = await this.options.owner.subscribe((event) => this.heard(event))
    } catch (failure) {
      this.publish({ problem: words(failure) })
      return
    }
    if (this.stopped) stop()
    else this.stopListening = stop
    await this.read()
  }

  stop(): void {
    this.stopped = true
    this.burst.stop()
    if (this.flashTimer !== null) this.clock.clearTimeout(this.flashTimer)
    this.flashTimer = null
    if (this.settleTimer !== null) this.clock.clearTimeout(this.settleTimer)
    this.settleTimer = null
    this.stopListening?.()
    this.stopListening = null
  }

  /** Finishes when no read is running. */
  idle(): Promise<void> {
    return this.burst.idle()
  }

  private heard(event: ChannelEvent): void {
    if (event.method === CONNECTION_EVENT) {
      // The events between may be gone from the daemon's log, so the board is read again.
      if (event.params.state === 'back') this.burst.poke()
    } else if (isBoardEvent(event, this.data?.sources.map((source) => source.name) ?? null, this.data?.settling)) {
      this.burst.poke()
    }
  }

  private async read(): Promise<void> {
    const { owner, boardId, view, groupBy, walk, filter } = this.options
    let next: FieldData
    try {
      next = await loadField(owner, boardId, view, walk?.() ?? null, filter ?? null)
    } catch (failure) {
      // A board that was read stays as it was: the next event reads it again. Only a first read has no board to keep.
      if (this.data === null) this.publish({ problem: words(failure) })
      return
    }
    if (this.stopped) return
    const before = this.data
    if (before !== null && sameData(before, next)) {
      this.watchSettling()
      return
    }
    this.data = next
    this.updatedAt = this.now()
    // A board that was still coming in is filling, not changing: its cards do not flash.
    if (before !== null && !before.settling) this.flash(changedCards(before, next, groupBy()))
    this.publish({})
    this.watchSettling()
  }

  /** While the board is settling, reads it again after `SETTLE_POLL_MS`, whether or not an event came. */
  private watchSettling(): void {
    if (this.stopped || this.settleTimer !== null || this.data === null || !this.data.settling) return
    this.settleTimer = this.clock.setTimeout(() => {
      this.settleTimer = null
      this.burst.poke()
    }, SETTLE_POLL_MS)
  }

  /** Marks cards as flashing, and takes the mark off `flashMs` after the last change. */
  private flash(cards: ReadonlySet<string>): void {
    if (cards.size === 0) return
    this.flashed = new Set([...this.flashed, ...cards])
    if (this.flashTimer !== null) this.clock.clearTimeout(this.flashTimer)
    this.flashTimer = this.clock.setTimeout(() => {
      this.flashTimer = null
      this.flashed = new Set()
      this.publish({})
    }, this.options.flashMs ?? FLASH_MS)
  }

  private publish(over: { problem?: string }): void {
    if (this.stopped) return
    this.options.onRead({
      data: this.data,
      problem: over.problem ?? null,
      flashed: this.flashed,
      updatedAt: this.updatedAt,
    })
  }
}
