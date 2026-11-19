// A provider of a repo pack for the person-only tests (ts-0128-bb): protocol v0 over stdio, run as
// `node providers/fake.mjs`. A test copies this file into a fixture repo's pack.
//
// It lists four items, one of each kind the person-only rule looks at and one it does not:
// `p1` a proposal (proposed), `p2` a proposal (deferred), `q1` a question (open) and `t1` a task (todo).
//
// Every `apply` appends the whole of its params, as one JSON line, to `applies` in the pack's data folder
// (`COLLIDE_PACK_DATA`), so a test reads what the provider was asked and whether it was asked at all. It also writes
// the file `written-<id>` in the checkout it was given, holding the status it was asked for, so a test sees on disk
// whether a write happened.
import { appendFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { createInterface } from 'node:readline'

const SOURCE = 'fake'

const ITEMS = {
  p1: { kind: 'proposal', status: 'proposed', category: 'backlog' },
  p2: { kind: 'proposal', status: 'deferred', category: 'backlog' },
  q1: { kind: 'question', status: 'open', category: 'ready' },
  t1: { kind: 'task', status: 'todo', category: 'ready' },
}

let init = null

const say = (message) => process.stdout.write(`${JSON.stringify(message)}\n`)
const state = (status, category) => ({ status, category })

const task = (id) => ({
  key: `${SOURCE}:${id}`,
  id,
  title: `A ${ITEMS[id].kind}`,
  kind: ITEMS[id].kind,
  state: state(ITEMS[id].status, ITEMS[id].category),
  version: 'v1',
  fields: {},
})

const initializeResult = () => ({
  scope: 'ref',
  idPattern: '^[a-z0-9-]+$',
  shortIdPattern: '^[a-z0-9]+$',
  statuses: [
    { name: 'proposed', category: 'backlog' },
    { name: 'accepted', category: 'ready' },
    { name: 'deferred', category: 'backlog' },
    { name: 'rejected', category: 'dropped' },
    { name: 'open', category: 'ready' },
    { name: 'todo', category: 'ready' },
    { name: 'doing', category: 'active' },
  ],
  ops: [{ name: 'setStatus' }],
  capabilities: { watch: false, preview: true },
})

const apply = (params) => {
  appendFileSync(join(process.env.COLLIDE_PACK_DATA, 'applies'), `${JSON.stringify(params)}\n`)
  const checkout = params.checkout ?? init.root
  const status = params.args?.status ?? 'doing'
  writeFileSync(join(checkout, `written-${params.id}`), `${status}\n`)
  const from = ITEMS[params.id]
  const written = { ...task(params.id), state: state(status, 'ready'), version: 'v2', checkout }
  const effect = {
    key: `${SOURCE}:${params.id}`,
    version: 'v2',
    from: state(from.status, from.category),
    to: state(status, 'ready'),
  }
  return { outcome: 'ok', task: written, version: 'v2', files: [`written-${params.id}`], items: [effect] }
}

// Every `preview` appends the whole of its params to `previews`, so a test reads the actor the provider was told.
const preview = (params) => {
  appendFileSync(join(process.env.COLLIDE_PACK_DATA, 'previews'), `${JSON.stringify(params)}\n`)
  return { outcome: 'ok', items: [], files: [] }
}

const answer = (method, params) => {
  switch (method) {
    case 'initialize':
      init = params
      return [initializeResult(), null]
    case 'list':
      return [{ tasks: Object.keys(ITEMS).map(task), collections: [], invalid: [] }, null]
    case 'get':
      return [ITEMS[params.id] ? { task: task(params.id) } : {}, null]
    case 'apply':
      return [apply(params), null]
    case 'preview':
      return [preview(params), null]
    default:
      return [null, { code: -32601, message: `Method not found: ${method}` }]
  }
}

createInterface({ input: process.stdin }).on('line', (line) => {
  if (!line.trim()) return
  const { id, method, params = {} } = JSON.parse(line)
  const [result, error] = answer(method, params)
  say(error ? { jsonrpc: '2.0', id, error } : { jsonrpc: '2.0', id, result })
})
