// What the thin-loop page does (ts-0081-wm): it calls the owner and dispatches the reducer's actions (`state.ts`). It
// is plain code over an `OwnerClient`, so a test drives it with a `FakeOwner` and no browser, and the React page
// (`useLoop.ts`) only reads its state and calls its methods.
//
// The page follows one run, the run of the task it started or the run a Queue item names. It reloads what it shows
// from the daemon, never from the events themselves: an event only says that it is time to look. So a reload after the
// event stream came back (`shell.connection`, `back`) is the same code as a reload after any event, and a gap in the
// daemon's log never leaves the page behind.
import { type ChannelEvent, CONNECTION_EVENT, type OwnerClient, toOwnerError } from '../owner/client.ts'
import { type LoopAction, type LoopState, initialLoopState, loopReducer } from './state.ts'
import type {
  AgentVersions,
  BoardSummary,
  DiffResult,
  Followed,
  ItemAnswer,
  QueueItem,
  RunSummary,
} from './types.ts'
import {
  ACCEPTED_WORDS,
  DISCARDED_WORDS,
  DISMISSED_WORDS,
  INTEGRATING_WORDS,
  NO_BOARD_WORDS,
  QUEUED_WORDS,
  SENT_BACK_WORDS,
} from './words.ts'

/** The workflow of a prompt: `collide/quick` (Stage 3c). */
export const QUICK_WORKFLOW = 'collide/quick'

/** The daemon events that say a run or the Queue may have changed. The rest are not worth a reload. */
const RELOAD_EVENTS: ReadonlySet<string> = new Set([
  'run.transition',
  'phase.transition',
  'permission.requested',
  'permission.answered',
  'workspace.transition',
  'task.changed',
])

/** The Queue kinds the page can answer. An item of another kind is listed, and the page says it cannot answer. */
const FOLLOWED_KINDS: ReadonlySet<QueueItem['kind']> = new Set(['permission', 'review'])

export interface Loop {
  getState(): LoopState
  /** Calls `listener` after each change of the state. Gives the function that stops it. */
  subscribe(listener: () => void): () => void
  /** Listens to the event channel and loads the page. It finishes when the first load has. */
  open(): Promise<void>
  /** Stops listening. */
  close(): void
  /** Finishes when no reload is waiting or running. */
  idle(): Promise<void>
  chooseBoard(id: string): void
  openFolder(path: string): Promise<void>
  /** Makes the task and starts the run. True when the run started. */
  startRun(prompt: string): Promise<boolean>
  allowOnce(item: string): Promise<void>
  deny(item: string): Promise<void>
  accept(item: string): Promise<void>
  /** Drops a confirmation the CLI asked for, which writes nothing. */
  dismiss(item: string): Promise<void>
  reprompt(item: string, words: string): Promise<void>
  /** The first press asks again; the second sends the discard. */
  discard(item: string): Promise<void>
  /** Keeps the work after the first press of discard. */
  keep(): void
  /** Shows one file's patch, or no patch for `null`. */
  pickFile(path: string | null): Promise<void>
  /** Follows the run of another item of the Queue. */
  openItem(item: string): Promise<void>
}

export function createLoop(owner: OwnerClient): Loop {
  return new LoopController(owner)
}

class LoopController implements Loop {
  private state = initialLoopState()
  private readonly listeners = new Set<() => void>()
  private stopListening: (() => void) | null = null
  private closed = false
  /** Counts opens and closes, so a subscription that finishes after a close (or a newer open) is dropped. */
  private generation = 0
  private wanted: 'light' | 'full' | null = null
  private draining: Promise<void> | null = null
  /** A task made for a prompt whose run was refused, so the same prompt again makes no second task. */
  private unstarted: { board: string; prompt: string; key: string } | null = null

  constructor(private readonly owner: OwnerClient) {
    // The page hands these to components as callbacks (`LoopActions`), so each keeps its `this`.
    this.chooseBoard = this.chooseBoard.bind(this)
    this.openFolder = this.openFolder.bind(this)
    this.startRun = this.startRun.bind(this)
    this.allowOnce = this.allowOnce.bind(this)
    this.deny = this.deny.bind(this)
    this.accept = this.accept.bind(this)
    this.dismiss = this.dismiss.bind(this)
    this.reprompt = this.reprompt.bind(this)
    this.discard = this.discard.bind(this)
    this.keep = this.keep.bind(this)
    this.pickFile = this.pickFile.bind(this)
    this.openItem = this.openItem.bind(this)
  }

  getState(): LoopState {
    return this.state
  }

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener)
    return () => {
      this.listeners.delete(listener)
    }
  }

  private dispatch(action: LoopAction): void {
    this.state = loopReducer(this.state, action)
    for (const listener of [...this.listeners]) listener()
  }

  // ---- the channel and the reloads ------------------------------------------------------------------------------

  async open(): Promise<void> {
    this.closed = false
    const generation = ++this.generation
    // Listen first, so an event between the subscription and the load is not missed.
    const stop = await this.owner.subscribe((event) => this.heard(event))
    if (this.generation === generation) this.stopListening = stop
    else stop()
    await this.reload(true)
  }

  close(): void {
    this.closed = true
    this.generation++
    this.stopListening?.()
    this.stopListening = null
  }

  async idle(): Promise<void> {
    while (this.draining !== null) await this.draining
  }

  private heard(event: ChannelEvent): void {
    if (event.method === CONNECTION_EVENT) {
      this.dispatch({ type: 'connection', state: event.params.state === 'lost' ? 'lost' : 'live' })
      if (event.params.state === 'back') void this.reload(true)
    } else if (event.method === 'queue.changed') {
      void this.reload(false)
    } else if (event.method === 'daemon.event' && RELOAD_EVENTS.has(event.params.body.event)) {
      void this.reload(false)
    }
  }

  /** Asks for a reload. Asks that come while one runs are met with one more reload after it. */
  private reload(full: boolean): Promise<void> {
    this.wanted = full || this.wanted === 'full' ? 'full' : 'light'
    this.draining ??= this.drain()
    return this.draining
  }

  private async drain(): Promise<void> {
    try {
      while (this.wanted !== null && !this.closed) {
        const full = this.wanted === 'full'
        this.wanted = null
        try {
          await this.sync(full)
        } catch (failure) {
          this.dispatch({ type: 'sync_failed', problem: `Could not reach Collide: ${toOwnerError(failure).message}` })
        }
      }
    } finally {
      this.draining = null
    }
  }

  /**
   * One reload: the runs, the Queue, the run that is followed and its diff. `full` also reads the boards and the
   * versions of `claude`.
   */
  private async sync(full: boolean): Promise<void> {
    const { followed } = this.state
    const listParams =
      followed?.board != null && followed.task != null
        ? { board: followed.board, task: followed.task, limit: 1 }
        : { limit: 1 }
    const [boards, versions, queue, listed] = await Promise.all([
      full ? this.owner.rpc('board.list').then((r) => r.boards) : undefined,
      full ? this.readVersions() : undefined,
      this.owner.rpc('queue.list', {}),
      this.owner.rpc('run.list', listParams),
    ])
    const items = queue.items
    const runId =
      followed?.run !== undefined
        ? (listed.runs[0]?.id ?? followed.run)
        : (items.find((i) => i.run != null && FOLLOWED_KINDS.has(i.kind))?.run ?? listed.runs[0]?.id ?? null)
    const target = runId === null ? null : await this.targetOf(runId, boards ?? this.state.boards, followed)
    const diff = target === null ? null : await this.diffOf(target.run.id, items)
    this.dispatch({
      type: 'synced',
      ...(boards === undefined ? {} : { boards }),
      ...(versions === undefined ? {} : { versions }),
      items,
      followed: target?.followed ?? null,
      run: target?.run ?? null,
      diff,
    })
  }

  private async readVersions(): Promise<AgentVersions | null> {
    try {
      const { agents } = await this.owner.rpc('daemon.agent_versions')
      return agents.find((a) => a.agent === 'claude') ?? agents[0] ?? null
    } catch {
      // The chips are an extra: a daemon that cannot read the versions leaves the page working.
      return null
    }
  }

  /** The run, and what the page says about it. The words of the task are read once, when the run first shows. */
  private async targetOf(
    runId: string,
    boards: readonly BoardSummary[],
    followed: Followed | null,
  ): Promise<{ run: RunSummary; followed: Followed }> {
    const { summary } = await this.owner.rpc('run.get', { run: runId })
    if (followed?.run === summary.id) return { run: summary, followed }
    const key = summary.tasks[0]?.key ?? null
    if (followed !== null && key !== null && followed.task === key) {
      return { run: summary, followed: { ...followed, run: summary.id } }
    }
    return { run: summary, followed: { ...(await this.findTask(boards, key, summary.id)), run: summary.id } }
  }

  /** The board that holds the task, and its words: a task key is only known on its own board. */
  private async findTask(
    boards: readonly BoardSummary[],
    key: string | null,
    runId: string,
  ): Promise<Omit<Followed, 'run'>> {
    if (key !== null) {
      for (const board of boards) {
        const { task, runs } = await this.owner.rpc('task.get', { board: board.id, id: key })
        if (task != null && runs.includes(runId)) {
          return { board: board.id, task: key, prompt: task.body ?? task.title }
        }
      }
    }
    return { board: null, task: key, prompt: null }
  }

  private async diffOf(runId: string, items: readonly QueueItem[]): Promise<DiffResult | null> {
    const waiting = items.some((i) => i.run === runId && i.kind === 'review')
    return waiting ? await this.owner.rpc('run.diff', { run: runId }) : null
  }

  // ---- what the person does -------------------------------------------------------------------------------------

  chooseBoard(id: string): void {
    this.dispatch({ type: 'board_chosen', id })
  }

  async openFolder(path: string): Promise<void> {
    const root = path.trim()
    if (root === '') return
    try {
      const { board, checks } = await this.owner.rpc('board.open', { root })
      if (board != null) {
        this.dispatch({ type: 'board_opened', board })
        return
      }
      const failed = checks.filter((c) => !c.ok).map((c) => c.message)
      this.dispatch({
        type: 'folder_refused',
        problems: failed.length > 0 ? failed : ['Collide could not open this folder.'],
      })
    } catch (failure) {
      this.dispatch({ type: 'folder_refused', problems: [toOwnerError(failure).message] })
    }
  }

  async startRun(prompt: string): Promise<boolean> {
    if (this.state.sending) return false
    const board = this.state.boardId
    if (board === null) {
      this.dispatch({ type: 'refused', message: NO_BOARD_WORDS })
      return false
    }
    if (prompt.trim() === '') return false
    this.dispatch({ type: 'sending' })
    try {
      const key = await this.taskFor(board, prompt)
      if (key === null) return false
      const started = await this.owner.rpc('run.start', { board, tasks: [key], workflow: QUICK_WORKFLOW })
      if (started.outcome === 'refused') {
        this.dispatch({ type: 'refused', message: started.problems.map((p) => p.message).join(' ') })
        return false
      }
      this.unstarted = null
      this.dispatch({ type: 'run_started', followed: { board, task: key, prompt, run: started.run } })
    } catch (failure) {
      this.dispatch({ type: 'refused', message: toOwnerError(failure).message })
      return false
    }
    await this.reload(false)
    return true
  }

  /** The scratch task for a prompt: the one a refused start left, or a new one. Null when the daemon refused it. */
  private async taskFor(board: string, prompt: string): Promise<string | null> {
    if (this.unstarted?.board === board && this.unstarted.prompt === prompt) return this.unstarted.key
    const { result, queued } = await this.owner.rpc('task.apply', {
      board,
      source: 'scratch',
      apply: { op: 'create', args: { prompt } },
    })
    if (result?.outcome !== 'ok') {
      // A scratch write is never queued, so `queued` is for the type's sake: it says why if one ever is.
      const fallback = queued?.reason ?? 'Collide could not make the task.'
      const message = result?.outcome === 'refused' ? result.message : fallback
      this.dispatch({ type: 'refused', message })
      return null
    }
    this.unstarted = { board, prompt, key: result.task.key }
    return result.task.key
  }

  allowOnce(item: string): Promise<void> {
    return this.answer(item, { type: 'permission', reply: { type: 'allow' } })
  }

  deny(item: string): Promise<void> {
    return this.answer(item, { type: 'permission', reply: { type: 'deny' } })
  }

  accept(item: string): Promise<void> {
    return this.answer(item, { type: 'accept' })
  }

  dismiss(item: string): Promise<void> {
    return this.answer(item, { type: 'dismiss' })
  }

  async reprompt(item: string, words: string): Promise<void> {
    if (words.trim() === '') return
    await this.answer(item, { type: 'reprompt', prompt: words })
  }

  async discard(item: string): Promise<void> {
    if (!this.state.confirmingDiscard) {
      this.dispatch({ type: 'discard_asked' })
      return
    }
    await this.answer(item, { type: 'discard' })
  }

  keep(): void {
    this.dispatch({ type: 'discard_kept' })
  }

  private async answer(item: string, answer: ItemAnswer): Promise<void> {
    if (this.state.sending) return
    this.dispatch({ type: 'sending' })
    try {
      const result = await this.owner.rpc('queue.answer', { item, answer })
      if (result.outcome === 'refused') this.dispatch({ type: 'refused', message: result.message })
      else
        this.dispatch({
          type: 'answered',
          notice: noticeFor(answer, result.outcome),
          sentBack: answer.type === 'reprompt' && result.outcome === 'done',
        })
    } catch (failure) {
      // A failed call may have taken effect, and the shell's message says so; the reload below shows what is true.
      this.dispatch({ type: 'refused', message: toOwnerError(failure).message })
    }
    await this.reload(false)
  }

  async pickFile(path: string | null): Promise<void> {
    this.dispatch({ type: 'file_picked', path })
    const run = this.state.run?.id
    if (path === null || run === undefined) return
    try {
      const { patch } = await this.owner.rpc('run.diff', { run, path })
      if (patch != null) this.dispatch({ type: 'patch_loaded', patch })
    } catch (failure) {
      this.dispatch({ type: 'refused', message: toOwnerError(failure).message })
    }
  }

  async openItem(item: string): Promise<void> {
    const run = this.state.items.find((i) => i.id === item)?.run
    if (run == null) return
    const boards = this.state.boards
    try {
      const { summary } = await this.owner.rpc('run.get', { run })
      const found = await this.findTask(boards, summary.tasks[0]?.key ?? null, run)
      this.dispatch({ type: 'run_started', followed: { ...found, run } })
    } catch (failure) {
      this.dispatch({ type: 'refused', message: toOwnerError(failure).message })
      return
    }
    await this.reload(false)
  }
}

function noticeFor(answer: ItemAnswer, outcome: 'done' | 'merged' | 'integrating' | 'queued'): string | null {
  switch (outcome) {
    case 'merged':
      return ACCEPTED_WORDS
    case 'integrating':
      return INTEGRATING_WORDS
    case 'queued':
      return QUEUED_WORDS
    case 'done':
      return answer.type === 'accept'
        ? 'Accepted.'
        : answer.type === 'reprompt'
          ? SENT_BACK_WORDS
          : answer.type === 'discard'
            ? DISCARDED_WORDS
            : answer.type === 'dismiss'
              ? DISMISSED_WORDS
              : null
  }
}

