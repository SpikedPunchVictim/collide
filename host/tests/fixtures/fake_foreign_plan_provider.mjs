// A planning source whose plan is not Collide's (ts-0149-kq): protocol v0 over stdio, run as `node providers/plan.mjs`
// by a repo pack that names it `plan`, so it takes the place of the built-in plan source. A test copies this file into
// a fixture repo's pack.
//
// Its ids are not Collide's: the key `plan:WORK-12` has the id `WORK-12-mend-the-door`. It lists no item of kind
// `epic`. Its own groupings are collections, `AREA-1` to `AREA-25`. Its `create` declares arguments that Collide's plan
// does not have: `proposal`, `tags` (an array of strings), `points` (an integer) and `flag` (a boolean).
//
// Every `create` appends the `args` it was given, as one JSON line, to `created.jsonl` in the checkout it was given,
// so a test sees what the source was asked and whether it was asked at all.
import { appendFileSync } from 'node:fs'
import { join } from 'node:path'
import { createInterface } from 'node:readline'

const SOURCE = 'plan'
const AREAS = 25

let init = null

const say = (message) => process.stdout.write(`${JSON.stringify(message)}\n`)

const task = (short, slug) => ({
  key: `${SOURCE}:${short}`,
  id: `${short}-${slug}`,
  title: slug.replaceAll('-', ' '),
  state: { status: 'ready', category: 'ready' },
  version: 'v1',
  fields: {},
})

const TASKS = [task('WORK-12', 'mend-the-door'), task('WORK-13', 'paint-it')]

const collections = () =>
  Array.from({ length: AREAS }, (_, index) => ({ id: `${SOURCE}:AREA-${index + 1}`, title: `Area ${index + 1}` }))

const initializeResult = () => ({
  scope: 'ref',
  idPattern: '^[A-Z]+-[0-9]+(-[a-z-]+)?$',
  shortIdPattern: '^[A-Z]+-[0-9]+$',
  statuses: [{ name: 'ready', category: 'ready' }],
  ops: [
    {
      name: 'create',
      args: {
        type: 'object',
        properties: {
          proposal: { type: 'string' },
          title: { type: 'string', minLength: 1 },
          epic: { type: 'string' },
          tags: { type: 'array', items: { type: 'string' } },
          points: { type: 'integer' },
          flag: { type: 'boolean' },
        },
        required: ['proposal', 'title', 'epic'],
        additionalProperties: false,
      },
    },
  ],
  capabilities: { watch: false, create: true, collections: true },
})

const create = (params) => {
  const checkout = params.checkout ?? init.root
  appendFileSync(join(checkout, 'created.jsonl'), `${JSON.stringify(params.args)}\n`)
  const made = task('WORK-14', 'new-one')
  const effect = { key: made.key, version: 'v1', from: null, to: made.state }
  return { outcome: 'ok', task: { ...made, checkout }, version: 'v1', files: ['created.jsonl'], items: [effect] }
}

const answer = (method, params) => {
  switch (method) {
    case 'initialize':
      init = params
      return [initializeResult(), null]
    case 'list':
      return [{ tasks: TASKS, collections: collections(), invalid: [] }, null]
    case 'get':
      return [{ task: TASKS.find((item) => item.id === params.id) }, null]
    case 'apply':
      return [create(params), null]
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
