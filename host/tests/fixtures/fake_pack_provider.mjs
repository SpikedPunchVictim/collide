// A provider of a repo pack, for the daemon's source tests (ts-0113-ck): protocol v0 over stdio, run as
// `node providers/<source>.mjs`. A test copies this file into a fixture repo's pack and fills in the two words below.
// It answers `initialize`, `list`, `get`, `apply`, `preview` and `watch` from fixed data, so a test can tell the
// answers apart by what it asks.
//
// On `initialize` it appends a line to `started` in its data folder (`COLLIDE_PACK_DATA`): a test reads that file to
// tell whether, and how many times, a provider started.
//
// `get` answers by its `id`: `boom` is a JSON-RPC error, `missing` is no task, `die` ends the process with no reply,
// `linger` answers like any task and then ignores the end of its input, so only a signal ends it,
// `echo` is a task whose `fields` hold what the process saw (what `initialize` gave it, its working folder, its pid,
// the names of its environment variables and the version below), and any other id is a plain task.
// `apply` and `preview` answer by their `op`: `ok`, `conflict` or `refused`.
import { appendFileSync } from 'node:fs'
import { createInterface } from 'node:readline'

const SOURCE = '__SOURCE__'
const VERSION = '__VERSION__'

let init = null

const say = (message) => process.stdout.write(`${JSON.stringify(message)}\n`)
const state = (status = 'todo', category = 'ready') => ({ status, category })
const task = (id = '1', fields = {}) => ({
  key: `${SOURCE}:${id.split('-').pop()}`,
  id,
  title: 'A fake task',
  state: state(),
  version: 'v1',
  fields,
})
const effect = () => ({ key: `${SOURCE}:1`, version: 'v2', from: state(), to: state('doing', 'active') })

const initializeResult = () => ({
  scope: 'ref',
  idPattern: '^[a-z0-9-]+$',
  shortIdPattern: '^[0-9]+$',
  statuses: [
    { name: 'todo', category: 'ready' },
    { name: 'doing', category: 'active' },
  ],
  ops: [{ name: 'setStatus' }],
  capabilities: { watch: true, preview: true },
})

const echoFields = () => ({
  init,
  cwd: process.cwd(),
  pid: process.pid,
  env: Object.keys(process.env).sort(),
  version: VERSION,
})

const get = (params) => {
  if (params.id === 'boom') return [null, { code: -32000, message: 'disk on fire' }]
  if (params.id === 'missing') return [{}, null]
  if (params.id === 'die') process.exit(4)
  if (params.id === 'linger') setInterval(() => {}, 1000)
  if (params.id === 'echo') return [{ task: task('1', echoFields()) }, null]
  return [{ task: task(params.id) }, null]
}

const apply = (params) => {
  if (params.op === 'conflict') return { outcome: 'conflict', current: task() }
  if (params.op === 'refused') return { outcome: 'refused', message: 'illegal transition' }
  const written = { ...task(), checkout: params.checkout ?? init.root }
  return { outcome: 'ok', task: written, version: 'v2', files: ['a.md'], items: [effect()] }
}

const preview = (params) => {
  if (params.op === 'conflict') return { outcome: 'conflict', current: task() }
  if (params.op === 'refused') return { outcome: 'refused', message: 'illegal transition' }
  return { outcome: 'ok', items: [effect()], files: ['a.md'] }
}

const answer = (method, params) => {
  switch (method) {
    case 'initialize':
      init = params
      appendFileSync(`${process.env.COLLIDE_PACK_DATA}/started`, `${process.pid}\n`)
      return [initializeResult(), null]
    case 'list':
      return [{ tasks: [task()], collections: [], invalid: [] }, null]
    case 'get':
      return get(params)
    case 'apply':
      return [apply(params), null]
    case 'preview':
      return [preview(params), null]
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
  say(error ? { jsonrpc: '2.0', id, error } : { jsonrpc: '2.0', id, result })
})
