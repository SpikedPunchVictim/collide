// Synthetic Orchard-shaped corpus. Invented names only.
import { mulberry32, pick, chance } from './rng'
import type { Branch, Epic, Task, TaskStatus } from './types'

export const EPICS: Epic[] = [
  { id: 'E-01', title: 'Ingest pipeline' },
  { id: 'E-02', title: 'Scheduler' },
  { id: 'E-03', title: 'Importers' },
  { id: 'E-04', title: 'Cache layer' },
  { id: 'E-05', title: 'Exports and reports' },
  { id: 'E-06', title: 'Config and CLI' },
  { id: 'E-07', title: 'Performance' },
  { id: 'E-08', title: 'Docs and tooling' },
]

const NOUNS: Record<string, string[]> = {
  'E-01': ['record parser', 'batch reader', 'dedupe pass', 'schema check', 'intake queue', 'checksum step'],
  'E-02': ['job queue', 'retry timer', 'priority lanes', 'lease renewal', 'cron parser', 'backoff policy'],
  'E-03': ['CSV importer', 'XML importer', 'delta import', 'import preview', 'field mapper', 'import log'],
  'E-04': ['entry TTL', 'eviction order', 'warmup job', 'cache keys', 'shard map', 'stale reads'],
  'E-05': ['report builder', 'export job', 'column picker', 'PDF layout', 'summary table', 'download link'],
  'E-06': ['config loader', 'env overrides', 'flag parser', 'profile switch', 'help output', 'init command'],
  'E-07': ['hot loop', 'index scan', 'allocation churn', 'startup path', 'batch size', 'lock contention'],
  'E-08': ['API notes', 'lint rule', 'release script', 'changelog tool', 'example repo', 'dev container'],
}
const VERBS = ['Add', 'Fix', 'Speed up', 'Trim', 'Rework', 'Cover', 'Document', 'Split', 'Retry', 'Log']
const LABELS = ['auth', 'cache', 'api', 'ui', 'infra', 'docs', 'perf', 'flaky', 'build', 'security']
const COUNTS = [22, 18, 20, 17, 16, 15, 18, 14]

function body(title: string): string {
  return `${title}. Keep the change small and add a test. Update the notes if behaviour changes.`
}

function build(): Task[] {
  const r = mulberry32(7)
  const tasks: Task[] = []
  EPICS.forEach((e, ei) => {
    for (let n = 1; n <= COUNTS[ei]; n++) {
      const title = `${pick(r, VERBS)} ${pick(r, NOUNS[e.id])}`
      const x = r()
      const status: TaskStatus =
        x < 0.3 ? 'ready' : x < 0.58 ? 'done' : x < 0.72 ? 'backlog' : x < 0.84 ? 'blocked' : 'ready'
      const labels = [pick(r, LABELS)]
      if (chance(r, 0.35)) {
        const l = pick(r, LABELS)
        if (!labels.includes(l)) labels.push(l)
      }
      tasks.push({
        id: `T-${String(ei + 1).padStart(2, '0')}-${String(n).padStart(3, '0')}`,
        title,
        epic: e.id,
        status,
        labels,
        source: chance(r, 0.3) ? 'github' : 'tasks/',
        dependsOn: [],
        body: body(title),
        release4: pick(r, ['done', 'ready', 'ready', 'backlog', 'blocked'] as TaskStatus[]),
      })
    }
  })
  // blocked tasks depend on an earlier task that is not done
  const open = tasks.filter((t) => t.status === 'ready' || t.status === 'backlog')
  tasks.filter((t) => t.status === 'blocked').forEach((t) => {
    t.dependsOn = [pick(r, open).id]
  })
  return tasks
}

type Patch = Partial<Omit<Task, 'id'>>
const PLANTED: Record<string, Patch> = {
  // scenario 2: epic E-07, ready, label perf: exactly these five
  'T-07-001': { title: 'Speed up index scan', status: 'ready', labels: ['perf'] },
  'T-07-004': { title: 'Trim allocation churn', status: 'ready', labels: ['perf', 'build'] },
  'T-07-007': { title: 'Speed up startup path', status: 'ready', labels: ['perf', 'cache'] },
  'T-07-010': { title: 'Rework batch size', status: 'ready', labels: ['perf'] },
  'T-07-013': { title: 'Fix lock contention', status: 'ready', labels: ['perf', 'flaky'] },
  // decoys for scenario 2
  'T-07-002': { status: 'ready', labels: ['docs'] },
  'T-07-005': { status: 'backlog', labels: ['perf'] },
  'T-07-008': { status: 'done', labels: ['perf'] },
  'T-07-011': { status: 'ready', labels: ['api'] },
  'T-06-003': { status: 'ready', labels: ['perf'] },
  'T-04-006': { status: 'ready', labels: ['perf'] },
  // scenario 3: six ready tasks in E-03, three of them named on the card
  'T-03-011': { title: 'Retry budget for the CSV importer', status: 'ready', labels: ['api'] },
  'T-03-012': { title: 'Add import preview', status: 'ready', labels: ['ui'] },
  'T-03-014': { title: 'Log skipped rows in the import log', status: 'ready', labels: ['api'] },
  'T-03-015': { title: 'Fix field mapper defaults', status: 'ready', labels: ['api'] },
  'T-03-017': { title: 'Cover delta import with tests', status: 'ready', labels: ['build'] },
  'T-03-019': { title: 'Document the XML importer limits', status: 'ready', labels: ['docs'] },
  'T-03-016': { status: 'backlog' },
  'T-03-018': { status: 'done' },
  // scenario 5: the finished run's task
  'T-05-009': { title: 'Add column picker to the export job', status: 'ready', labels: ['ui'] },
  // scenario 7: the task for the experiment
  'T-04-012': { title: 'Choose an eviction order for the cache layer', status: 'ready', labels: ['cache', 'perf'] },
  // scenario 4 and 9 tasks
  'T-02-006': { title: 'Retry timer drifts after sleep', status: 'ready', labels: ['flaky'] },
  'T-02-009': { title: 'Add priority lanes to the job queue', status: 'ready', labels: ['api'] },
  'T-01-005': { title: 'Add a checksum step to intake', status: 'ready', labels: ['security'] },
  'T-01-008': { title: 'Fix dedupe pass on empty batches', status: 'ready', labels: ['api'] },
  'T-06-005': { title: 'Env overrides ignore empty values', status: 'ready', labels: ['build'] },
  'T-06-009': { title: 'Add an init command', status: 'ready', labels: ['ui'] },
  'T-05-004': { title: 'Summary table drops the last row', status: 'ready', labels: ['api'] },
  'T-08-003': { title: 'Add a lint rule for stale TODOs', status: 'ready', labels: ['build'] },
  'T-08-006': { title: 'Release script skips tags', status: 'ready', labels: ['build'] },
  'T-03-004': { title: 'Import log loses the first line', status: 'ready', labels: ['flaky'] },
  // scenario 6 and 8 runs
  'T-04-003': { title: 'Warmup job for the shard map', status: 'done', labels: ['cache'] },
  'T-04-004': { title: 'Stale reads after eviction', status: 'done', labels: ['cache'] },
  'T-07-003': { title: 'Cut the hot loop allocation', status: 'ready', labels: ['api'] },
}

export function makeTasks(): Task[] {
  const tasks = build()
  for (const t of tasks) {
    const p = PLANTED[t.id]
    if (p) Object.assign(t, p, { body: body(p.title ?? t.title) })
    if (t.status !== 'blocked') t.dependsOn = []
    if (!p) {
      // keep the scenario target sets exact: no stray ready perf tasks in E-07, no stray ready tasks in E-03
      if (t.epic === 'E-07' && t.status === 'ready') t.labels = t.labels.map((l) => (l === 'perf' ? 'api' : l))
      if (t.epic === 'E-03' && t.status === 'ready') t.status = 'backlog'
    }
  }
  return tasks
}

export const BRANCHES: Branch[] = [
  {
    name: 'main',
    mergeBase: 'main',
    stat: [],
  },
  {
    name: 'exp/ttl-cache',
    mergeBase: 'main',
    stat: [
      { path: 'src/cache/entry.rs', add: 41, del: 12 },
      { path: 'src/cache/ttl.rs', add: 88, del: 0 },
      { path: 'tests/cache_ttl.rs', add: 64, del: 0 },
    ],
  },
  {
    name: 'exp/lru-cache',
    mergeBase: 'main',
    stat: [
      { path: 'src/cache/entry.rs', add: 29, del: 18 },
      { path: 'src/cache/lru.rs', add: 102, del: 0 },
      { path: 'tests/cache_lru.rs', add: 71, del: 0 },
      { path: 'migrations/0042_cache_index.sql', add: 9, del: 0, protectedPath: true },
    ],
  },
  {
    name: 'exp/no-cache',
    mergeBase: 'release-3',
    stat: [
      { path: 'src/cache/mod.rs', add: 3, del: 140 },
      { path: '.github/workflows/ci.yml', add: 2, del: 2, protectedPath: true },
    ],
  },
]
