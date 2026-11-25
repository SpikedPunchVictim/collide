// The Changes part follows the daemon (ts-0189-fg; Stage 6a, "an active task shows its edits as they land"). Like the
// Field's feed (`../field/live.ts`) it is told by an event that it is time to look, never what to show: it reads
// `task.changes` again, once for a burst of events. An edit in a workspace sends no event of its own, so while a run is
// live it also looks again every few seconds. Plain code over an `OwnerClient` and a clock the caller gives, so each
// rule is tested with no browser and no real time; `useChanges.ts` wires it to React.
import { CONNECTION_EVENT, type ChannelEvent, type OwnerClient } from '../owner/client.ts'
import { Burst, type Clock, LIVE_DEBOUNCE_MS, realClock } from '../field/live.ts'
import { type Changes, hasLiveRun } from './changes.ts'

/** While a run is live its changes are read this often, with or without an event. */
export const LIVE_REFRESH_MS = 5000

/**
 * The daemon events that can mean a run's files changed: a run or a phase moved, a gate answered, the agent was asked
 * to use a tool or was let (it edits between the two), or a task file changed.
 */
const CHANGES_EVENTS: ReadonlySet<string> = new Set([
  'run.transition',
  'phase.transition',
  'workspace.transition',
  'gate.verdict',
  'permission.requested',
  'permission.answered',
  'task.changed',
])

/** Whether an event is a reason to read the changes again. The stream coming back is: events between may be gone. */
export function isChangesEvent(event: ChannelEvent): boolean {
  if (event.method === CONNECTION_EVENT) return event.params.state === 'back'
  return event.method === 'daemon.event' && CHANGES_EVENTS.has(event.params.body.event)
}

/** What the feed holds after a read. */
export interface ChangesRead {
  /** The changes as the last good read gave them, or null before the first. */
  readonly changes: Changes | null
  /** The words of the last read if it failed. What was read before stays shown. */
  readonly problem: string | null
}

export interface ChangesFeedOptions {
  readonly owner: OwnerClient
  readonly boardId: string
  /** The task's key, as `task.get` gives it. */
  readonly task: string
  /** Called with each new state. Never after `stop`. */
  readonly onRead: (read: ChangesRead) => void
  readonly clock?: Clock
  readonly debounceMs?: number
  readonly refreshMs?: number
}

const words = (failure: unknown) => (failure instanceof Error ? failure.message : String(failure))

export class ChangesFeed {
  private readonly options: ChangesFeedOptions
  private readonly clock: Clock
  private readonly burst: Burst
  private stopListening: (() => void) | null = null
  private refresh: unknown = null
  private stopped = false
  private changes: Changes | null = null

  constructor(options: ChangesFeedOptions) {
    this.options = options
    this.clock = options.clock ?? realClock
    this.burst = new Burst(this.clock, options.debounceMs ?? LIVE_DEBOUNCE_MS, () => this.read())
  }

  /** Listens, and then reads. It finishes when the first read has. */
  async start(): Promise<void> {
    // Listen first, so that an event between the subscription and the read is not missed.
    let stop: () => void
    try {
      stop = await this.options.owner.subscribe((event) => this.heard(event))
    } catch (failure) {
      this.publish(words(failure))
      return
    }
    if (this.stopped) stop()
    else this.stopListening = stop
    await this.read()
  }

  stop(): void {
    this.stopped = true
    this.burst.stop()
    this.clearRefresh()
    this.stopListening?.()
    this.stopListening = null
  }

  /** Finishes when no read is running. */
  idle(): Promise<void> {
    return this.burst.idle()
  }

  private heard(event: ChannelEvent): void {
    if (isChangesEvent(event)) this.burst.poke()
  }

  private async read(): Promise<void> {
    const { owner, boardId, task } = this.options
    this.clearRefresh()
    try {
      this.changes = await owner.rpc('task.changes', { board: boardId, task })
    } catch (failure) {
      this.publish(words(failure))
      this.watchLive()
      return
    }
    this.publish(null)
    this.watchLive()
  }

  /** While a run is live, looks again in a few seconds. */
  private watchLive(): void {
    if (this.stopped || this.changes === null || !hasLiveRun(this.changes)) return
    this.refresh = this.clock.setTimeout(() => {
      this.refresh = null
      this.burst.poke()
    }, this.options.refreshMs ?? LIVE_REFRESH_MS)
  }

  private clearRefresh(): void {
    if (this.refresh !== null) this.clock.clearTimeout(this.refresh)
    this.refresh = null
  }

  private publish(problem: string | null): void {
    if (this.stopped) return
    this.options.onRead({ changes: this.changes, problem })
  }
}
