// A scripted daemon for the thin-loop page's tests (ts-0081-wm): a `FakeOwner` whose handlers read and change one plain
// `World`, so a test says what the daemon holds, pushes the event that tells the page, and reads what the page does.
// There is no daemon, no socket and no timer here.
import type {
  BoardListResultOutput,
  DaemonAgentVersionsResultOutput,
  QueueAnswerParamsInput,
  QueueAnswerResultOutput,
  QueueListResultOutput,
  RunDiffResultOutput,
  RunGetResultOutput,
  TaskOutput,
} from '../../src/owner/generated.ts'
import { FakeOwner } from '../../src/owner/index.ts'

export type Board = BoardListResultOutput['boards'][number]
export type Agent = DaemonAgentVersionsResultOutput['agents'][number]
export type Item = QueueListResultOutput['items'][number]
export type Run = RunGetResultOutput['summary']
export type Diff = RunDiffResultOutput

export function board(id: string, root: string): Board {
  return { id, root, lines: [{ ref_name: 'main', root, trust: { state: 'trusted', hash: 'h' } }] }
}

export function claude(over: Partial<Agent> = {}): Agent {
  return {
    agent: 'claude',
    in_use: '2.1.325',
    in_tested_range: true,
    tested: { from: '2.1.300', through: '2.1.330' },
    published: { latest: '2.1.330', stable: '2.1.311', fetched_ms: 1_000 },
    ...over,
  }
}

export function run(id: string, task: string, over: Partial<Run> = {}): Run {
  return {
    id,
    tasks: [{ key: task, status: 'active' }],
    line: 'main',
    workflow: 'collide/quick',
    state: { state: 'running' },
    started_ms: 1_000,
    workspace: `/tmp/work/${id}`,
    warnings: [],
    phases: [
      {
        name: 'agent',
        kind: 'agent',
        round: 1,
        instances: [{ id: `${id}-i`, session_id: `session-${id}`, state: { state: 'running' } }],
        gate_results: {},
      },
    ],
    ...over,
  }
}

export function reviewItem(
  id: string,
  runId: string,
  task: string,
  answers: Item['answers'] = ['accept', 'reprompt', 'discard'],
): Item {
  return {
    id,
    kind: 'review',
    run: runId,
    phase: null,
    task,
    since_ms: 2_000,
    title: 'Ready to review',
    untrusted_input: false,
    show: [],
    answers,
  }
}

/** A person-only write the CLI asked for (ts-0128-bb): no run, and the title is Collide's own words. */
export function confirmationItem(id: string, task: string, title = 'Confirm: accept proposal plan:pr-0003-km'): Item {
  return {
    id,
    kind: 'confirmation',
    run: null,
    phase: null,
    task,
    since_ms: 2_000,
    title,
    untrusted_input: false,
    show: [],
    answers: ['accept', 'dismiss'],
  }
}

export function permissionItem(id: string, runId: string, task: string, input?: unknown): Item {
  return {
    id,
    kind: 'permission',
    run: runId,
    phase: 'agent',
    task,
    since_ms: 2_000,
    title: 'The agent asks to use a tool',
    untrusted_input: false,
    show: [],
    answers: ['permission'],
    permission: {
      id: `${id}-request`,
      kind: 'execute',
      tool: 'Bash',
      tool_call_id: 'call-1',
      reason: 'To run the tests',
      options: [
        { id: 'o1', kind: 'allow_once', name: 'Allow once' },
        { id: 'o2', kind: 'allow_always', name: 'Always allow' },
        { id: 'o3', kind: 'reject_once', name: 'Reject' },
      ],
      ...(input === undefined ? {} : { input }),
    },
  }
}

export function diff(
  files: Diff['files'] = [{ path: 'src/a.ts', status: 'modified', additions: 3, deletions: 1 }],
): Diff {
  return { base: 'base0', head: 'head1', files, collapsed: [], more: 0 }
}

interface StoredTask {
  key: string
  prompt: string
  /** Newest first, as `task.get` lists them. */
  runs: string[]
}

export class World {
  readonly owner = new FakeOwner()
  boards: Board[] = []
  /** The folders `board.open` knows, by the path a person types. */
  folders = new Map<string, Board | { checks: { message: string }[] }>()
  versions: DaemonAgentVersionsResultOutput | 'fails' = { agents: [claude()] }
  tasks = new Map<string, Map<string, StoredTask>>()
  runs = new Map<string, Run>()
  items: Item[] = []
  diffs = new Map<string, Diff>()
  patches = new Map<string, string>()
  /** What `queue.answer` says. A test may change it. */
  answerResult: (params: QueueAnswerParamsInput) => QueueAnswerResultOutput = () => ({ outcome: 'done' })
  /** What changes in the world when an answer is taken. */
  onAnswer: (params: QueueAnswerParamsInput) => void = () => {}
  /** Set to make `task.apply` or `run.start` refuse. */
  refuseCreate: string | null = null
  refuseStart: string | null = null
  private nextTask = 1
  private nextRun = 1

  constructor() {
    const owner = this.owner
    owner.handle('board.list', () => ({ boards: this.boards }))
    owner.handle('board.open', ({ root }) => {
      const known = this.folders.get(root)
      if (known === undefined) return { checks: [failed('This folder is not a git repository.')] }
      if ('id' in known) {
        if (!this.boards.some((b) => b.id === known.id)) this.boards.push(known)
        return { board: known, checks: [{ id: 'git_repo', ok: true, required: true, message: 'ok' }] }
      }
      return { checks: known.checks.map((c) => failed(c.message)) }
    })
    owner.handle('daemon.agent_versions', () => {
      if (this.versions === 'fails') throw new Error('the daemon could not read the versions')
      return this.versions
    })
    owner.handle('task.apply', ({ board: boardId, apply }) => {
      if (this.refuseCreate !== null) return { result: { outcome: 'refused', message: this.refuseCreate } }
      const key = `scratch:${this.nextTask++}`
      const prompt = String((apply.args as { prompt?: string } | undefined)?.prompt ?? '')
      this.tasksOf(boardId).set(key, { key, prompt, runs: [] })
      return { result: { outcome: 'ok', files: [], items: [], task: taskOf(key, prompt), version: 'v1' } }
    })
    owner.handle('task.get', ({ board: boardId, id }) => {
      const stored = this.tasksOf(boardId).get(id)
      if (stored === undefined) return { ambiguous: [], copies: [], collisions: [], links: [], runs: [] }
      const task = taskOf(stored.key, stored.prompt)
      return { task, ambiguous: [], copies: [], collisions: [], links: [], runs: stored.runs }
    })
    owner.handle('run.start', ({ board: boardId, tasks }) => {
      if (this.refuseStart !== null) return { outcome: 'refused', problems: [{ message: this.refuseStart }] }
      const key = tasks[0]!
      const id = `r-${this.nextRun++}`
      this.addRun(boardId, run(id, key))
      return { outcome: 'started', run: id }
    })
    owner.handle('run.list', ({ task, board: boardId }) => {
      let runs = [...this.runs.values()].sort((a, b) => b.started_ms - a.started_ms)
      if (task !== undefined && task !== null) runs = runs.filter((r) => r.tasks.some((t) => t.key === task))
      if (boardId !== undefined && boardId !== null) {
        const ids = new Set([...this.tasksOf(boardId).values()].flatMap((t) => t.runs))
        runs = runs.filter((r) => ids.has(r.id))
      }
      return { runs }
    })
    owner.handle('run.get', ({ run: id }) => {
      const summary = this.runs.get(id)
      if (summary === undefined) throw new Error(`no run ${id}`)
      return {
        summary,
        snapshot_commit: 'abc',
        launch: {},
        implementations: {},
        rounds: {},
        fingerprint: {},
        annotations: [],
      }
    })
    owner.handle('run.diff', ({ run: id, path }) => {
      const found = this.diffs.get(id)
      if (found === undefined) throw new Error(`no diff for ${id}`)
      if (path === undefined || path === null) return found
      const text = this.patches.get(path) ?? ''
      return { ...found, patch: { path, text, bytes: text.length, truncated: false } }
    })
    owner.handle('queue.list', () => ({ items: this.items }))
    owner.handle('queue.answer', (params) => {
      const result = this.answerResult(params)
      if (result.outcome !== 'refused') this.onAnswer(params)
      return result
    })
  }

  tasksOf(boardId: string): Map<string, StoredTask> {
    let tasks = this.tasks.get(boardId)
    if (tasks === undefined) this.tasks.set(boardId, (tasks = new Map()))
    return tasks
  }

  /** A task and its run that were there before the page loaded. */
  seed(boardId: string, key: string, prompt: string, summary: Run): void {
    this.tasksOf(boardId).set(key, { key, prompt, runs: [] })
    this.addRun(boardId, summary)
  }

  addRun(boardId: string, summary: Run): void {
    this.runs.set(summary.id, summary)
    const task = this.tasksOf(boardId).get(summary.tasks[0]!.key)!
    task.runs.unshift(summary.id)
  }

  setRun(id: string, over: Partial<Run>): void {
    this.runs.set(id, { ...this.runs.get(id)!, ...over })
  }

  /** The run's agent finished: it ended, its review item waits and its diff is there. */
  finish(id: string, itemId = 'q-review'): void {
    const task = this.runs.get(id)!.tasks[0]!.key
    this.setRun(id, { state: { state: 'ended', outcome: 'done' }, ended_ms: 5_000 })
    this.items = [reviewItem(itemId, id, task)]
    this.diffs.set(id, diff())
    this.patches.set('src/a.ts', '@@ -1 +1 @@\n-old\n+new\n')
  }

  /** The Queue's change notice and the run's transition, as the daemon sends them. */
  announce(runId: string): void {
    this.owner.push({
      method: 'daemon.event',
      params: {
        seq: 1,
        wall_ms: 1,
        actor: { kind: 'workflow' },
        experiment: null,
        body: { event: 'run.transition', run: runId, from: null, to: { state: 'running' } },
      },
    })
  }

  callsTo(method: string): unknown[] {
    return this.owner.calls.filter((c) => c.method === method).map((c) => c.params)
  }
}

function failed(message: string) {
  return { id: 'git_repo' as const, ok: false, required: true, message }
}

function taskOf(key: string, prompt: string): TaskOutput {
  return {
    aliases: [],
    body: prompt,
    collections: [],
    fields: {},
    id: key,
    key,
    labels: [],
    links: [],
    owners: [],
    state: { category: 'ready', status: 'ready' },
    templateFields: {},
    title: prompt.split('\n')[0]!,
    version: 'v1',
  }
}
