import { PassThrough } from 'node:stream'
import { expect, test } from 'vitest'
import { defineProvider } from '../src/index.ts'
import type { ProviderHandlers } from '../src/index.ts'

const task = { id: 'ts-0001-ab', key: 'ts-0001-ab', title: 'One', version: 'v1', state: { category: 'ready' as const, status: 'ready' } }

const handlers: ProviderHandlers = {
  initialize: ({ config }) => ({
    scope: 'ref',
    idPattern: '^ts-\\d{4}-[a-z]{2}$',
    shortIdPattern: '^\\d{4}$',
    statuses: [{ name: 'ready', category: 'ready' }],
    capabilities: { watch: false, create: config.create === true },
  }),
  list: async ({ checkout }) => ({ tasks: [{ ...task, checkout }] }),
  get: ({ id }) => ({ task: { ...task, id } }),
  apply: ({ op, version }) =>
    version === 'stale' ? { outcome: 'conflict', current: task } : { outcome: 'ok', task, version: 'v2', files: [`${op}.md`], items: [] },
}

type Message = Record<string, any>

/** A fake client: writes lines to the provider's stdin and reads the lines it writes to stdout. */
function connect(provider: ProviderHandlers, log: (message: string) => void = () => {}) {
  const input = new PassThrough()
  const output = new PassThrough()
  output.setEncoding('utf8')
  const lines: string[] = []
  const waiting: Array<() => void> = []
  let buffer = ''
  output.on('data', (chunk: string) => {
    buffer += chunk
    const parts = buffer.split('\n')
    buffer = parts.pop() ?? ''
    lines.push(...parts)
    waiting.splice(0).forEach((wake) => wake())
  })
  const done = defineProvider(provider, { input, output, log })
  const nextLine = async (): Promise<string> => {
    while (lines.length === 0) await new Promise<void>((resolve) => waiting.push(resolve))
    return lines.shift() as string
  }
  return {
    done,
    sendRaw: (line: string) => input.write(`${line}\n`),
    send: (message: Message) => input.write(`${JSON.stringify(message)}\n`),
    nextLine,
    next: async (): Promise<Message> => JSON.parse(await nextLine()),
    /** Ends stdin, waits for the provider to finish, and returns every line it had not yet been read. */
    close: async () => {
      input.end()
      await done
      return [...lines, ...(buffer === '' ? [] : [buffer])]
    },
  }
}

const request = (id: number | string | null, method: string, params?: unknown): Message => ({ jsonrpc: '2.0', id, method, ...(params === undefined ? {} : { params }) })

test('initialize passes the pack settings to the handler and answers with its result', async () => {
  const client = connect(handlers)
  client.send(request(1, 'initialize', { config: { create: true }, root: '/work/repo' }))
  const reply = await client.next()
  expect(reply).toMatchObject({ jsonrpc: '2.0', id: 1, result: { scope: 'ref', capabilities: { create: true } } })
  expect(await client.close()).toEqual([])
})

test('a field that declares a label keeps it through initialize, and one without has none', async () => {
  const fields = [
    { id: 'story_points', type: 'number', filterable: true, label: 'Points' },
    { id: 'epic', type: 'string', filterable: true },
  ]
  const client = connect({
    ...handlers,
    initialize: (params) => ({ ...(handlers.initialize(params) as object), fields }) as never,
  })
  client.send(request(1, 'initialize', { config: {}, root: '/work/repo' }))
  const reply = await client.next()
  expect(reply.result.fields).toEqual(fields)
  expect(reply.result.fields[1]).not.toHaveProperty('label')
  await client.close()
})

test('list answers with the handler result', async () => {
  const client = connect(handlers)
  client.send(request(2, 'list', { checkout: 'main' }))
  expect(await client.next()).toEqual({ jsonrpc: '2.0', id: 2, result: { tasks: [{ ...task, checkout: 'main' }] } })
  await client.close()
})

test('get answers with the handler result', async () => {
  const client = connect(handlers)
  client.send(request('g-1', 'get', { id: 'ts-0002-cd' }))
  expect(await client.next()).toEqual({ jsonrpc: '2.0', id: 'g-1', result: { task: { ...task, id: 'ts-0002-cd' } } })
  await client.close()
})

test('apply answers ok and conflict as the handler returns them', async () => {
  const client = connect(handlers)
  client.send(request(3, 'apply', { op: 'setStatus', args: {}, version: 'v1' }))
  client.send(request(4, 'apply', { op: 'setStatus', args: {}, version: 'stale' }))
  expect(await client.next()).toEqual({ jsonrpc: '2.0', id: 3, result: { outcome: 'ok', task, version: 'v2', files: ['setStatus.md'], items: [] } })
  expect(await client.next()).toEqual({ jsonrpc: '2.0', id: 4, result: { outcome: 'conflict', current: task } })
  await client.close()
})

test('preview takes apply params and answers with the handler result when the pack has one', async () => {
  const client = connect({ ...handlers, preview: ({ op }) => ({ outcome: 'ok', items: [], files: [`${op}.md`] }) })
  client.send(request(5, 'preview', { op: 'claim', args: {} }))
  expect(await client.next()).toEqual({ jsonrpc: '2.0', id: 5, result: { outcome: 'ok', items: [], files: ['claim.md'] } })
  await client.close()
})

test('watch answers, and its notify writes a notification line with no id', async () => {
  const client = connect({
    ...handlers,
    watch: ({ checkouts }, notify) => {
      notify('source.stale', { reason: `watching ${checkouts.join(',')}` })
      return {}
    },
  })
  client.send(request(6, 'watch', { checkouts: ['main'] }))
  expect(await client.next()).toEqual({ jsonrpc: '2.0', method: 'source.stale', params: { reason: 'watching main' } })
  expect(await client.next()).toEqual({ jsonrpc: '2.0', id: 6, result: {} })
  await client.close()
})

test('notify still writes after the watch handler has returned', async () => {
  let later: (() => void) | undefined
  const client = connect({
    ...handlers,
    watch: (_params, notify) => {
      later = () => notify('task.invalid', { path: 'a.md', checkout: 'main', reason: 'no title' })
      return {}
    },
  })
  client.send(request(7, 'watch', { checkouts: ['main'] }))
  expect(await client.next()).toMatchObject({ id: 7, result: {} })
  later?.()
  expect(await client.next()).toEqual({ jsonrpc: '2.0', method: 'task.invalid', params: { path: 'a.md', checkout: 'main', reason: 'no title' } })
  await client.close()
})

test('an unknown method gets -32601 and the server keeps serving', async () => {
  const client = connect(handlers)
  client.send(request(1, 'frobnicate', {}))
  expect(await client.next()).toMatchObject({ jsonrpc: '2.0', id: 1, error: { code: -32601 } })
  client.send(request(2, 'get', { id: 'x' }))
  expect(await client.next()).toMatchObject({ id: 2, result: { task: { id: 'x' } } })
  await client.close()
})

test.each(['watch', 'preview'])('%s is method not found when the pack does not define it', async (method) => {
  const client = connect(handlers)
  client.send(request(1, method, { op: 'x', args: {}, checkouts: [] }))
  expect(await client.next()).toMatchObject({ id: 1, error: { code: -32601 } })
  await client.close()
})

test('a name on Object.prototype is method not found, not a call', async () => {
  const client = connect(handlers)
  client.send(request(1, 'toString'))
  client.send(request(2, '__proto__'))
  expect(await client.next()).toMatchObject({ id: 1, error: { code: -32601 } })
  expect(await client.next()).toMatchObject({ id: 2, error: { code: -32601 } })
  await client.close()
})

test('a throwing handler gets -32603 with its message, the stack goes to the log, and the server keeps serving', async () => {
  const logged: string[] = []
  const client = connect(
    {
      ...handlers,
      get: () => {
        throw new Error('disk on fire')
      },
      list: async () => {
        throw 'plain string'
      },
    },
    (message) => logged.push(message),
  )
  client.send(request(1, 'get', { id: 'x' }))
  expect(await client.next()).toMatchObject({ id: 1, error: { code: -32603, message: 'disk on fire' } })
  client.send(request(2, 'list', {}))
  expect(await client.next()).toMatchObject({ id: 2, error: { code: -32603, message: 'plain string' } })
  client.send(request(3, 'initialize', { config: {}, root: '/work/repo' }))
  expect(await client.next()).toMatchObject({ id: 3, result: { scope: 'ref' } })
  await client.close()
  expect(logged.join('\n')).toContain('disk on fire')
})

test('a handler that returns nothing, or a value JSON cannot hold, gets -32603', async () => {
  const circular: Record<string, unknown> = {}
  circular.self = circular
  const client = connect({
    ...handlers,
    get: (() => undefined) as unknown as ProviderHandlers['get'],
    list: (() => circular) as unknown as ProviderHandlers['list'],
  })
  client.send(request(1, 'get', { id: 'x' }))
  client.send(request(2, 'list', {}))
  client.send(request(3, 'initialize', { config: {}, root: '/work/repo' }))
  expect(await client.next()).toMatchObject({ id: 1, error: { code: -32603 } })
  expect(await client.next()).toMatchObject({ id: 2, error: { code: -32603 } })
  expect(await client.next()).toMatchObject({ id: 3, result: { scope: 'ref' } })
  await client.close()
})

test('a malformed line gets -32700 with a null id and the server keeps serving', async () => {
  const client = connect(handlers)
  client.sendRaw('{"jsonrpc": "2.0", "id": 1, "method": ')
  expect(await client.next()).toMatchObject({ jsonrpc: '2.0', id: null, error: { code: -32700 } })
  client.send(request(2, 'get', { id: 'x' }))
  expect(await client.next()).toMatchObject({ id: 2, result: { task: { id: 'x' } } })
  await client.close()
})

test('blank lines are skipped, and a CRLF line ending is accepted', async () => {
  const client = connect(handlers)
  client.sendRaw('')
  client.sendRaw('   ')
  client.sendRaw(`${JSON.stringify(request(1, 'get', { id: 'x' }))}\r`)
  expect(await client.next()).toMatchObject({ id: 1, result: { task: { id: 'x' } } })
  expect(await client.close()).toEqual([])
})

test('a final line with no newline is still served', async () => {
  const input = new PassThrough()
  const output = new PassThrough()
  output.setEncoding('utf8')
  const done = defineProvider(handlers, { input, output })
  input.end(JSON.stringify(request(1, 'get', { id: 'x' })))
  await done
  expect(JSON.parse(output.read() as string)).toMatchObject({ id: 1, result: { task: { id: 'x' } } })
})

test.each([
  ['a JSON value that is not an object', '5'],
  ['a batch array', '[]'],
  ['a missing jsonrpc version', '{"id": 1, "method": "get"}'],
  ['a method that is not a string', '{"jsonrpc": "2.0", "id": 1, "method": 7}'],
])('%s gets -32600 and the server keeps serving', async (_name, line) => {
  const client = connect(handlers)
  client.sendRaw(line)
  expect(await client.next()).toMatchObject({ jsonrpc: '2.0', error: { code: -32600 } })
  client.send(request(2, 'get', { id: 'x' }))
  expect(await client.next()).toMatchObject({ id: 2, result: {} })
  await client.close()
})

test('an invalid request that carries an id answers with that id', async () => {
  const client = connect(handlers)
  client.sendRaw('{"jsonrpc": "2.0", "id": 9, "method": 7}')
  expect(await client.next()).toMatchObject({ id: 9, error: { code: -32600 } })
  await client.close()
})

test('params that are not an object get -32602 and the handler is not called', async () => {
  let calls = 0
  const client = connect({ ...handlers, get: () => (calls++, { task }) })
  client.send(request(1, 'get', [1, 2]))
  client.send(request(2, 'get', 'x'))
  expect(await client.next()).toMatchObject({ id: 1, error: { code: -32602 } })
  expect(await client.next()).toMatchObject({ id: 2, error: { code: -32602 } })
  await client.close()
  expect(calls).toBe(0)
})

test('absent params reach the handler as an empty object', async () => {
  const client = connect({ ...handlers, list: (params) => ({ tasks: [], nextCursor: JSON.stringify(params) }) })
  client.send(request(1, 'list'))
  expect(await client.next()).toMatchObject({ id: 1, result: { nextCursor: '{}' } })
  await client.close()
})

test('a client notification gets no response, even when the handler throws or the method is unknown', async () => {
  const seen: string[] = []
  const logged: string[] = []
  const client = connect(
    {
      ...handlers,
      get: ({ id }) => {
        seen.push(id)
        throw new Error('ignored')
      },
    },
    (message) => logged.push(message),
  )
  client.send({ jsonrpc: '2.0', method: 'get', params: { id: 'n-1' } })
  client.send({ jsonrpc: '2.0', method: 'frobnicate' })
  client.send(request(1, 'initialize', { config: {}, root: '/work/repo' }))
  expect(await client.next()).toMatchObject({ id: 1, result: { scope: 'ref' } })
  expect(await client.close()).toEqual([])
  expect(seen).toEqual(['n-1'])
  expect(logged.join('\n')).toContain('ignored')
})

test('an id of null is a request and is answered with a null id', async () => {
  const client = connect(handlers)
  client.send(request(null, 'get', { id: 'x' }))
  expect(await client.next()).toMatchObject({ id: null, result: { task: { id: 'x' } } })
  await client.close()
})

test('requests are handled one at a time, in order, and answered in order', async () => {
  const order: string[] = []
  const client = connect({
    ...handlers,
    list: async () => {
      order.push('list:start')
      await new Promise((resolve) => setTimeout(resolve, 20))
      order.push('list:end')
      return { tasks: [] }
    },
    get: ({ id }) => {
      order.push(`get:${id}`)
      return { task }
    },
  })
  client.send(request(1, 'list', {}))
  client.send(request(2, 'get', { id: 'x' }))
  expect((await client.next()).id).toBe(1)
  expect((await client.next()).id).toBe(2)
  expect(order).toEqual(['list:start', 'list:end', 'get:x'])
  await client.close()
})

test('the promise resolves when input ends, after the in-flight handler has answered', async () => {
  const client = connect({
    ...handlers,
    list: async () => {
      await new Promise((resolve) => setTimeout(resolve, 20))
      return { tasks: [] }
    },
  })
  client.send(request(1, 'list', {}))
  const lines = await client.close()
  expect(lines.map((line) => JSON.parse(line).id)).toEqual([1])
})

test('requests work before initialize: the SDK imposes no order', async () => {
  const client = connect(handlers)
  client.send(request(1, 'get', { id: 'x' }))
  expect(await client.next()).toMatchObject({ id: 1, result: {} })
  await client.close()
})

test('every line the provider writes is one JSON message, even when a string holds a newline', async () => {
  const client = connect({ ...handlers, get: () => ({ task: { ...task, title: 'two\nlines here' } }) })
  client.send(request(1, 'get', { id: 'x' }))
  const line = await client.nextLine()
  expect(line).not.toContain('\n')
  expect(JSON.parse(line).result.task.title).toBe('two\nlines here')
  await client.close()
})

test('input that errors rejects the promise', async () => {
  const input = new PassThrough()
  const done = defineProvider(handlers, { input, output: new PassThrough() })
  input.destroy(new Error('stdin broke'))
  await expect(done).rejects.toThrow('stdin broke')
})
