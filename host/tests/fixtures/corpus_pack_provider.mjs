// A provider of a repo pack, for the corpus tests (ts-0116-wh): protocol v0 over stdio, run as
// `node providers/<source>.mjs`. A test copies this file into a fixture repo's pack and fills in the words below.
// It is a ref-scoped source over files: each `tasks/**/*.json` of a checkout is one task, so a test makes, edits and
// removes tasks with plain file writes, in as many checkouts as it likes.
//
//   __SOURCE__  the source's name
//   __WATCH__   `true` or `false`: whether `initialize` declares the `watch` capability
//   __PAGE__    how many files one page of `list` reads
//
// A task file is `{ "short": "12", "title": "...", "status": "todo", "version": "v1", "kind": "task" }`, and its key is
// `<source>:<short>`. A file that is not JSON, or has no `short`, is an invalid item in `list`. It may also have
// `labels` (strings), `collections` (ids) and `links` (`[{ "type": "blocked-by", "to": "41" }]`), which a view reads,
// and `features` (`[{ "id": "F1", "title": "...", "section": "", "summary": "" }]`) on a proposal, and
// `partProblems` (`[{ "part": "claims", "line": 12, "reason": "..." }]`), which a view counts. `initialize`
// declares `blocked-by` and `covers`, a link to a feature (`"to": "10#F1"`), each with the label of its other end.
//
// The data folder (`COLLIDE_PACK_DATA`) is the test's way in and out:
//   started       a line with the pid for each `initialize`
//   init.json     the params of the last `initialize` (the settings the provider was started with)
//   calls.log     a line for each `list` (`list <checkout> <cursor>`) and `watch` (`watch <json>`)
//   notify.jsonl  each line a test appends is sent to the daemon as a notification, `{ "method", "params" }`
//   fail-list     when it exists, `list` answers a JSON-RPC error
//   die-on-list   when it exists, `list` ends the process with no reply
import { appendFileSync, existsSync, readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs'
import { join, relative } from 'node:path'
import { createInterface } from 'node:readline'

const SOURCE = '__SOURCE__'
const WATCH = __WATCH__
const PAGE = __PAGE__
const DATA = process.env.COLLIDE_PACK_DATA

let init = null

const say = (message) => process.stdout.write(`${JSON.stringify(message)}\n`)
const log = (line) => appendFileSync(join(DATA, 'calls.log'), `${line}\n`)

const CATEGORY = { todo: 'ready', doing: 'active', done: 'done' }
const state = (status) => ({ status, category: CATEGORY[status] ?? 'backlog' })

const files = (checkout) => {
  const out = []
  const walk = (dir) => {
    let names = []
    try {
      names = readdirSync(dir).sort()
    } catch {
      return
    }
    for (const name of names) {
      const path = join(dir, name)
      if (statSync(path).isDirectory()) walk(path)
      else if (name.endsWith('.json')) out.push(path)
    }
  }
  walk(join(checkout, 'tasks'))
  return out
}

const read = (checkout, file) => {
  const path = relative(checkout, file)
  try {
    const data = JSON.parse(readFileSync(file, 'utf8'))
    if (typeof data.short !== 'string') return { invalid: { path, checkout, reason: 'no short id' } }
    const status = data.status ?? 'todo'
    const task = {
      key: `${SOURCE}:${data.short}`,
      id: data.short,
      title: data.title ?? `Task ${data.short}`,
      state: state(status),
      version: data.version ?? 'v1',
      checkout,
      path,
      fields: {},
    }
    if (data.kind) task.kind = data.kind
    if (data.labels) task.labels = data.labels
    if (data.collections) task.collections = data.collections
    if (data.links) task.links = data.links
    if (data.features) task.features = data.features
    if (data.partProblems) task.partProblems = data.partProblems
    return { task, file }
  } catch (error) {
    return { invalid: { path, checkout, reason: `not JSON: ${error.message}` } }
  }
}

const find = (checkout, id) => files(checkout).map((file) => read(checkout, file)).find((got) => got.task?.id === id)

const list = (params) => {
  const checkout = params.checkout ?? init.root
  log(`list ${checkout} ${params.cursor ?? ''}`)
  if (existsSync(join(DATA, 'die-on-list'))) process.exit(3)
  if (existsSync(join(DATA, 'fail-list'))) return [null, { code: -32000, message: 'the list was refused' }]
  const all = files(checkout)
  const from = params.cursor ? Number(params.cursor) : 0
  const page = all.slice(from, from + PAGE).map((file) => read(checkout, file))
  const result = {
    tasks: page.filter((got) => got.task).map((got) => got.task),
    collections: [],
    invalid: page.filter((got) => got.invalid).map((got) => got.invalid),
  }
  if (from + PAGE < all.length) result.nextCursor = String(from + PAGE)
  return [result, null]
}

const get = (params) => {
  const found = find(params.checkout ?? init.root, params.id)
  if (!found) return [{}, null]
  return [{ task: { ...found.task, body: `body of ${params.id}` } }, null]
}

const apply = (params) => {
  const checkout = params.checkout ?? init.root
  const found = find(checkout, params.id)
  if (!found) return [{ outcome: 'refused', message: `no task ${params.id}` }, null]
  if (params.version && params.version !== found.task.version) {
    return [{ outcome: 'conflict', current: found.task }, null]
  }
  const data = JSON.parse(readFileSync(found.file, 'utf8'))
  const next = `v${Number(found.task.version.slice(1)) + 1}`
  const from = found.task.state
  data.status = params.args.status
  data.version = next
  writeFileSync(found.file, `${JSON.stringify(data)}\n`)
  const written = read(checkout, found.file).task
  const effect = { key: written.key, version: next, from, to: written.state }
  return [{ outcome: 'ok', task: written, version: next, files: [written.path], items: [effect] }, null]
}

const answer = (method, params) => {
  switch (method) {
    case 'initialize':
      init = params
      // init.json first: a test that sees the start in `started` finds the settings of that start.
      writeFileSync(join(DATA, 'init.json'), JSON.stringify(params))
      appendFileSync(join(DATA, 'started'), `${process.pid}\n`)
      return [
        {
          scope: 'ref',
          idPattern: '^[a-z0-9-]+$',
          shortIdPattern: '^[0-9]+$',
          statuses: [
            { name: 'todo', category: 'ready' },
            { name: 'doing', category: 'active' },
            { name: 'done', category: 'done' },
          ],
          ops: [{ name: 'setStatus' }],
          links: [
            { name: 'blocked-by', dir: 'out', blocking: true, inverse: 'blocks' },
            { name: 'covers', dir: 'out', blocking: false, target: 'feature', inverse: 'covered by' },
          ],
          capabilities: { watch: WATCH, preview: false },
        },
        null,
      ]
    case 'list':
      return list(params)
    case 'get':
      return get(params)
    case 'apply':
      return apply(params)
    case 'watch':
      log(`watch ${JSON.stringify(params.checkouts)}`)
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

// What a test appends to `notify.jsonl` after this process started is sent on, line by line.
const NOTIFY = join(DATA, 'notify.jsonl')
let offset = existsSync(NOTIFY) ? readFileSync(NOTIFY).lastIndexOf(10) + 1 : 0
setInterval(() => {
  if (!existsSync(NOTIFY)) return
  const bytes = readFileSync(NOTIFY)
  // Only whole lines: a line a test is still writing is read at the next tick.
  const end = bytes.lastIndexOf(10) + 1
  if (end <= offset) return
  const fresh = bytes.subarray(offset, end).toString('utf8')
  offset = end
  for (const line of fresh.split('\n')) {
    if (!line.trim()) continue
    const { method, params } = JSON.parse(line)
    say({ jsonrpc: '2.0', method, params })
  }
}, 25)
