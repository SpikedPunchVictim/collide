import { expect, test } from 'vitest'
import { type ChannelEvent, FakeOwner, METHOD_NOT_FOUND, type OwnerClient, OwnerError } from '../src/owner/index.ts'

// The fake owner stands in for the daemon behind the same interface as the shell's client, so these run in a browser
// (or node) with no daemon and no shell.

const status = {
  version: '0.0.0',
  protocol: 1,
  uptime_s: 12,
  live_runs: 0,
  store: { state: 'ok' },
  warnings: [],
} as never

test('a call to the fake owner gets the typed result of its handler', async () => {
  const fake = new FakeOwner().handle('daemon.status', () => status)
  const owner: OwnerClient = fake
  const result = await owner.rpc('daemon.status')
  expect(result).toEqual(status)
})

test('the fake owner hands its handler the params the caller sent', async () => {
  const fake = new FakeOwner().handle('board.open', (params) => {
    expect(params).toEqual({ path: '/work/repo' })
    return {} as never
  })
  await fake.rpc('board.open', { path: '/work/repo' } as never)
  expect(fake.calls).toEqual([{ method: 'board.open', params: { path: '/work/repo' } }])
})

test('a method with no handler fails as an unknown method does', async () => {
  const failure = await new FakeOwner().rpc('queue.list').catch((error: unknown) => error)
  expect(failure).toBeInstanceOf(OwnerError)
  expect((failure as OwnerError).code).toBe(METHOD_NOT_FOUND)
})

test('an OwnerError thrown by a handler reaches the caller with its code', async () => {
  const fake = new FakeOwner().handle('run.stop', () => {
    throw new OwnerError(-32010, 'no such run')
  })
  const failure = await fake.rpc('run.stop', { run: 'r-1' } as never).catch((error: unknown) => error)
  expect(failure).toMatchObject({ code: -32010, message: 'no such run' })
})

const queueChanged: ChannelEvent = { method: 'queue.changed', params: { count: 1 } as never }

test('an event pushed by the fake owner reaches a subscriber', async () => {
  const fake = new FakeOwner()
  const seen: ChannelEvent[] = []
  await fake.subscribe((event) => seen.push(event))
  fake.push(queueChanged)
  expect(seen).toEqual([queueChanged])
})

test('an event reaches every subscriber, and none after it unsubscribes', async () => {
  const fake = new FakeOwner()
  const first: ChannelEvent[] = []
  const second: ChannelEvent[] = []
  const stopFirst = await fake.subscribe((event) => first.push(event))
  await fake.subscribe((event) => second.push(event))
  fake.push(queueChanged)
  stopFirst()
  fake.push(queueChanged)
  expect([first.length, second.length]).toEqual([1, 2])
})

test('the shell\'s ui_tools.request event is a channel event with its own payload', async () => {
  const fake = new FakeOwner()
  const seen: ChannelEvent[] = []
  await fake.subscribe((event) => seen.push(event))
  fake.push({
    method: 'ui_tools.request',
    params: { requestId: 'q-1', caller: 'external', deadline: 1_700_000_010_000, message: { method: 'tools/call' } },
  })
  expect(seen[0]?.method).toBe('ui_tools.request')
})
