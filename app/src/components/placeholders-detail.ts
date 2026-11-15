// Placeholder props for the parts whose shape the generated types do not cover yet (the time breakdown, the review, an
// experiment), and the whole set the shell shows until the owner client supplies real data (Stage 3c and Stage 6).
import type { ExperimentView } from './experiments/ExperimentsSurface.tsx'
import {
  PLACEHOLDER_CONTEXT,
  PLACEHOLDER_NOW_MS,
  PLACEHOLDER_OUTPUT,
  PLACEHOLDER_QUEUE,
  PLACEHOLDER_RUNS,
  PLACEHOLDER_TASKS,
} from './placeholders.ts'
import type { ReviewData } from './run/ReviewPane.tsx'
import type { BreakdownRow } from './run/TimeBreakdown.tsx'
import type { ContextRecord, OutputEntry, QueueItem, RunSummary, Task } from './types.ts'

export const PLACEHOLDER_BREAKDOWN: readonly BreakdownRow[] = [
  { name: 'Model time', calls: 14, totalMs: 9 * 60_000, share: 0.45, errors: 0 },
  { name: 'Bash', calls: 9, totalMs: 6 * 60_000, share: 0.3, errors: 1 },
  { name: 'Read', calls: 22, totalMs: 2 * 60_000, share: 0.1, errors: 0 },
  { name: 'Waiting for you', calls: 2, totalMs: 2 * 60_000, share: 0.1, errors: 0 },
  { name: 'unattributed (hooks, start-up, waits)', calls: 0, totalMs: 60_000, share: 0.05, errors: 0 },
]

export const PLACEHOLDER_REVIEW: ReviewData = {
  report: 'Added the retry offer. Three files changed and the tests pass.',
  reviewerA: ['The retry count is not shown to the person.'],
  reviewerB: [],
  diffSummary: ' app/src/queue.ts | 12 ++++++++----\n 1 file changed, 8 insertions(+), 4 deletions(-)',
}

export const PLACEHOLDER_EXPERIMENT: ExperimentView = {
  preview: {
    task: 'Remember the task list between launches',
    armA: 'main',
    armB: 'try/ttl-cache',
    diffStat: ' core/src/cache.rs | 40 ++++++++++++++++++++++++++++++++++++++++\n 1 file changed, 40 insertions(+)',
    protectedPathsHit: [],
    mergeBaseWarning: false,
    primaryMetric: 'time from launch to an accepted run',
    otherRunsLive: 2,
    expectedDurationWords: null,
  },
  result: { verdict: 'Arm B was faster', criticalPathDelta: '4 min shorter', outcomesEqual: true },
}

/** What the shell shows. The shape is the one the owner client's results will fill. */
export interface ShellData {
  readonly tasks: readonly Task[]
  readonly runs: readonly RunSummary[]
  readonly queue: readonly QueueItem[]
  readonly context: Partial<ContextRecord>
  readonly output: readonly OutputEntry[]
  readonly breakdown: readonly BreakdownRow[]
  readonly review: ReviewData
  readonly experiment: ExperimentView
  readonly nowMs: number
}

export const PLACEHOLDER_DATA: ShellData = {
  tasks: PLACEHOLDER_TASKS,
  runs: PLACEHOLDER_RUNS,
  queue: PLACEHOLDER_QUEUE,
  context: PLACEHOLDER_CONTEXT,
  output: PLACEHOLDER_OUTPUT,
  breakdown: PLACEHOLDER_BREAKDOWN,
  review: PLACEHOLDER_REVIEW,
  experiment: PLACEHOLDER_EXPERIMENT,
  nowMs: PLACEHOLDER_NOW_MS,
}
