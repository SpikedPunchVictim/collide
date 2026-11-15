// Placeholder props: invented data of the generated shapes, so each surface renders before the window has an owner to
// ask (Stage 3c and Stage 6 replace it with the real client). Ids and names are made up and match no real task.
import type { ContextRecord, OutputEntry, PhaseSummary, QueueItem, RunSummary, Task } from './types.ts'

/** "Now" for the placeholder data, so the waiting times it shows are the same on every render. */
export const PLACEHOLDER_NOW_MS = 1_800_000_000_000
const MINUTE = 60_000

function task(key: string, title: string, category: Task['state']['category'], labels: string[] = []): Task {
  return {
    id: `plan:${key}`,
    key,
    title,
    aliases: [],
    collections: [],
    fields: {},
    labels,
    links: [],
    owners: [],
    state: { category, status: category },
    templateFields: {},
    version: '1',
  }
}

export const PLACEHOLDER_TASKS: readonly Task[] = [
  task('T-03-011', 'Remember the task list between launches', 'ready', ['speed']),
  task('T-03-012', 'Explain why the agent asked for permission', 'active', ['clarity']),
  task('T-05-009', 'Offer a retry when a check fails', 'review'),
  task('T-01-002', 'Write the first-run checklist', 'backlog'),
  task('T-01-001', 'Open any git folder without setup', 'done'),
]

function phase(
  name: string,
  kind: PhaseSummary['kind'],
  state: PhaseSummary['instances'][number]['state'],
  times: { started_ms?: number; ended_ms?: number } = {},
): PhaseSummary {
  return {
    name,
    kind,
    round: 1,
    gate_results: {},
    instances: [{ id: `${name}-1`, state }],
    ...times,
  }
}

const START = PLACEHOLDER_NOW_MS - 40 * MINUTE

export const PLACEHOLDER_RUNS: readonly RunSummary[] = [
  {
    id: 'R-118',
    line: 'main',
    workflow: 'collide/quick',
    started_ms: START,
    state: { state: 'running' },
    tasks: [{ key: 'T-03-012', status: 'active' }],
    phases: [
      phase('provision', 'script', { state: 'ended' }, { started_ms: START, ended_ms: START + 5 * MINUTE }),
      phase('build', 'agent', { state: 'ended' }, { started_ms: START + 5 * MINUTE, ended_ms: START + 25 * MINUTE }),
      phase('review', 'fanout', { state: 'running' }, { started_ms: START + 25 * MINUTE }),
      phase('close', 'script', { state: 'pending' }),
    ],
  },
  {
    id: 'R-119',
    line: 'main',
    workflow: 'collide/quick',
    started_ms: START + 10 * MINUTE,
    state: { state: 'paused', cause: 'ask_human' },
    tasks: [{ key: 'T-05-009', status: 'review' }],
    phases: [
      phase('provision', 'script', { state: 'ended' }, { started_ms: START + 10 * MINUTE, ended_ms: START + 14 * MINUTE }),
      phase('build', 'agent', { state: 'paused', cause: 'ask_human' }, { started_ms: START + 14 * MINUTE }),
      phase('review', 'fanout', { state: 'pending' }),
    ],
  },
  {
    id: 'R-115',
    line: 'main',
    workflow: 'collide/quick',
    started_ms: START - 3 * 60 * MINUTE,
    ended_ms: START - 2 * 60 * MINUTE,
    state: { state: 'ended', outcome: 'accepted' },
    tasks: [{ key: 'T-01-001', status: 'done' }],
    phases: [
      phase('build', 'agent', { state: 'ended' }, { started_ms: START - 180 * MINUTE, ended_ms: START - 150 * MINUTE }),
      phase('review', 'fanout', { state: 'ended' }, { started_ms: START - 150 * MINUTE, ended_ms: START - 135 * MINUTE }),
      phase('fix', 'agent', { state: 'ended' }, { started_ms: START - 135 * MINUTE, ended_ms: START - 125 * MINUTE }),
    ],
  },
  {
    id: 'R-116',
    line: 'main',
    workflow: 'collide/quick',
    started_ms: START - 2 * 60 * MINUTE,
    ended_ms: START - 30 * MINUTE,
    state: { state: 'ended', outcome: 'accepted' },
    tasks: [{ key: 'T-01-002', status: 'done' }],
    phases: [
      phase('build', 'agent', { state: 'ended' }, { started_ms: START - 120 * MINUTE, ended_ms: START - 95 * MINUTE }),
      phase('review', 'fanout', { state: 'ended' }, { started_ms: START - 95 * MINUTE, ended_ms: START - 80 * MINUTE }),
      phase('fix', 'agent', { state: 'ended' }, { started_ms: START - 80 * MINUTE, ended_ms: START - 30 * MINUTE }),
    ],
  },
]

export const PLACEHOLDER_QUEUE: readonly QueueItem[] = [
  {
    id: 'Q-1',
    kind: 'permission',
    title: 'The agent wants to run a command',
    run: 'R-119',
    task: 'T-05-009',
    phase: 'build',
    since_ms: PLACEHOLDER_NOW_MS - 14 * MINUTE,
    untrusted_input: true,
    show: [],
    answers: ['permission', 'stop'],
    cache: { warm_secs_left: 12 * 60, resume_cost: { amount: 1.3, currency: 'USD' } },
    permission: {
      id: 'perm-1',
      tool_call_id: 'call-1',
      kind: 'execute',
      tool: 'Bash',
      input: { command: 'pnpm install' },
      options: [{ id: 'o1', kind: 'allow_once', name: 'Allow once' }],
    },
  },
  {
    id: 'Q-2',
    kind: 'failed_gate',
    title: 'The tests did not pass',
    run: 'R-118',
    since_ms: PLACEHOLDER_NOW_MS - 6 * MINUTE,
    untrusted_input: false,
    show: [],
    answers: ['proceed', 'stop', 'retry'],
  },
  {
    id: 'Q-3',
    kind: 'review',
    title: 'A finished run is ready to review',
    run: 'R-116',
    since_ms: PLACEHOLDER_NOW_MS - 1 * MINUTE,
    untrusted_input: false,
    show: [],
    answers: ['accept', 'discard', 'reprompt'],
  },
]

/** A context record with two sections the data lacks, as scenario 9 of S6 left two empty on purpose. */
export const PLACEHOLDER_CONTEXT: ContextRecord = {
  why_it_asks: { status: 'filled', text: 'The rule "ask before installing" matched this command.' },
  what_it_would_do: { status: 'filled', text: 'Runs `pnpm install` in the run\'s own copy of the repository.' },
  task: { status: 'filled', text: 'Offer a retry when a check fails.' },
  what_led_here: { status: 'not_available' },
  history: { status: 'not_available' },
  agents_own_words: { status: 'filled', text: 'I need the packages installed before I can run the tests.' },
}

export const PLACEHOLDER_OUTPUT: readonly OutputEntry[] = [
  { index: 0, wall_ms: START, event: { type: 'message', text: 'I will start by reading the failing test.' } },
  { index: 1, wall_ms: START + 2000, event: { type: 'tool_call', id: 'c1', kind: 'read', tool: 'Read', input: { path: 'app/test/queue.test.ts' } } },
  { index: 2, wall_ms: START + 9000, event: { type: 'tool_call', id: 'c2', kind: 'execute', tool: 'Bash', input: { command: 'pnpm test' } } },
  { index: 3, wall_ms: START + 31000, event: { type: 'message', text: 'Two tests fail. The queue sorts newest first.' } },
]
