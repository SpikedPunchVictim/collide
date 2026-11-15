// A stand-in provider for the CLI's tests: answers from fixed data, and misbehaves as FAKE_MODE says.
//   conflict    apply answers conflict, with the item as it is now
//   rpc-error   list answers with a JSON-RPC error
//   exit        list writes to stderr and exits
//   notify      sends a notification before each reply
//   garbage     list answers with something that is not a list result
import readline from 'node:readline'

const mode = process.env.FAKE_MODE ?? ''
const statuses = ['ready', 'active', 'review', 'done', 'dropped']
const task = (status, version) => ({
  key: 'plan:ts-0001-aa',
  id: 'ts-0001-aa',
  title: 'The only item',
  state: { status, category: status },
  version,
  path: '.collide/planning/epics/ep-001-aa-alpha/tasks/ts-0001-aa-only.md',
  kind: 'task',
  links: [{ type: 'part-of', to: 'ep-001-aa' }],
  owners: [],
})

const results = {
  initialize: {
    scope: 'ref',
    idPattern: '^[a-z]{2}-[0-9]{3,}-[a-z]{2}$',
    shortIdPattern: '^[0-9]+(-[a-z]{2})?$',
    statuses: statuses.map((name) => ({ name, category: name })),
    kinds: [{ name: 'task', statuses: statuses.map((name) => ({ name, category: name })) }],
    ops: [
      {
        name: 'setStatus',
        args: { type: 'object', properties: { status: { enum: statuses } }, required: ['status'], additionalProperties: false },
      },
    ],
    capabilities: { watch: false, create: false, collections: false, preview: false },
  },
  list: { tasks: [task('ready', 'v1')], invalid: [] },
  get: { task: task('ready', 'v1') },
  apply: { outcome: 'conflict', current: task('active', 'v2') },
}

const send = (message) => process.stdout.write(`${JSON.stringify({ jsonrpc: '2.0', ...message })}\n`)

readline.createInterface({ input: process.stdin }).on('line', (line) => {
  const { id, method } = JSON.parse(line)
  if (mode === 'notify') send({ method: 'source.stale', params: { reason: 'just a notification' } })
  if (mode === 'rpc-error' && method === 'list') return send({ id, error: { code: -32000, message: 'the fake source is on fire' } })
  if (mode === 'exit' && method === 'list') {
    process.stderr.write('fake provider: giving up\n')
    process.exit(3)
  }
  if (mode === 'garbage' && method === 'list') return send({ id, result: { tasks: 'none' } })
  send({ id, result: results[method] })
})
