// The provider of the `s6-board` pack (ts-0196-uw): protocol v0 over stdio, run as `node providers/s6.mjs`. It lists
// the tasks and epics of S6's start state for scenario 2 (`sp-0009`, `src/corpus.ts`: the planted tasks and the decoys
// around them), all fixed. It reads and writes nothing.
//
// The scenario's target: the tasks of epic E-07 that are ready and carry the label perf, exactly these five.
//   T-07-001  T-07-004  T-07-007  T-07-010  T-07-013
// The decoys each miss one of the three: another status, another epic, or no `perf` label.
import { createInterface } from 'node:readline'

const SOURCE = 's6'

const EPICS = [
  { id: 'E-04', title: 'Cache layer' },
  { id: 'E-06', title: 'Config and CLI' },
  { id: 'E-07', title: 'Performance' },
]

// [id, title, status, labels]
const TASKS = [
  ['T-04-006', 'Shard map lookups are slow', 'ready', ['perf']],
  ['T-04-009', 'Choose an eviction order', 'ready', ['cache']],
  ['T-06-003', 'Flag parser is slow on long input', 'ready', ['perf']],
  ['T-06-005', 'Env overrides ignore empty values', 'ready', ['build']],
  ['T-07-001', 'Speed up index scan', 'ready', ['perf']],
  ['T-07-002', 'Document the profiling steps', 'ready', ['docs']],
  ['T-07-003', 'Cut the hot loop allocation', 'ready', ['api']],
  ['T-07-004', 'Trim allocation churn', 'ready', ['perf', 'build']],
  ['T-07-005', 'Measure the cold start', 'backlog', ['perf']],
  ['T-07-006', 'Profile the batch reader', 'active', ['perf']],
  ['T-07-007', 'Speed up startup path', 'ready', ['perf', 'cache']],
  ['T-07-008', 'Cache the parsed schema', 'done', ['perf']],
  ['T-07-009', 'Add a benchmark harness', 'backlog', ['build']],
  ['T-07-010', 'Rework batch size', 'ready', ['perf']],
  ['T-07-011', 'Expose timing in the API', 'ready', ['api']],
  ['T-07-012', 'Retire the old timer', 'done', ['api']],
  ['T-07-013', 'Fix lock contention', 'ready', ['perf', 'flaky']],
]

const CATEGORY = { backlog: 'backlog', ready: 'ready', active: 'active', done: 'done' }

const taskOf = (checkout, [id, title, status, labels]) => ({
  key: `${SOURCE}:${id}`,
  id,
  title,
  state: { status, category: CATEGORY[status] },
  version: 'v1',
  checkout,
  path: `tasks/${id}.json`,
  fields: {},
  labels,
  collections: [`${SOURCE}:E-${id.slice(2, 4)}`],
})

const INITIALIZE = {
  scope: 'ref',
  idPattern: '^[A-Z]-[0-9]{2}-[0-9]{3}$',
  shortIdPattern: '^[A-Z]-[0-9]{2}-[0-9]{3}$',
  statuses: Object.keys(CATEGORY).map((name) => ({ name, category: CATEGORY[name] })),
  ops: [],
  links: [],
  capabilities: { watch: false, preview: false },
}

let root = ''

const answer = (method, params) => {
  const checkout = params.checkout ?? root
  switch (method) {
    case 'initialize':
      root = params.root
      return [INITIALIZE, null]
    case 'list':
      return [
        {
          tasks: TASKS.map((row) => taskOf(checkout, row)),
          collections: EPICS.map(({ id, title }) => ({ id: `${SOURCE}:${id}`, title })),
          invalid: [],
        },
        null,
      ]
    case 'get': {
      const row = TASKS.find(([id]) => id === params.id)
      return [row === undefined ? {} : { task: { ...taskOf(checkout, row), body: `${row[1]}.` } }, null]
    }
    case 'watch':
      return [{}, null]
    default:
      return [null, { code: -32601, message: `Method not found: ${method}` }]
  }
}

createInterface({ input: process.stdin }).on('line', (line) => {
  if (!line.trim()) return
  const { id, method, params = {} } = JSON.parse(line)
  const [result, error] = answer(method, params)
  process.stdout.write(`${JSON.stringify(error ? { jsonrpc: '2.0', id, error } : { jsonrpc: '2.0', id, result })}\n`)
})
