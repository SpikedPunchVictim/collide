import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { beforeEach, expect, test, vi } from 'vitest'

const invoke = vi.hoisted(() => vi.fn())
const listen = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke }))
vi.mock('@tauri-apps/api/event', () => ({ listen }))

import { CONNECTION_EVENT, OwnerError } from '../src/owner/client.ts'
import { EVENT_CHANNEL, RPC_COMMAND, createTauriOwner } from '../src/owner/tauri.ts'

beforeEach(() => {
  invoke.mockReset()
  listen.mockReset()
})

test('rpc invokes the shell\'s command with the method and the params and nothing else', async () => {
  invoke.mockResolvedValue({ board: 'b' })
  const result = await createTauriOwner().rpc('board.open', { path: '/work/repo' } as never)
  expect(result).toEqual({ board: 'b' })
  expect(invoke).toHaveBeenCalledWith(RPC_COMMAND, { method: 'board.open', params: { path: '/work/repo' } })
  expect(Object.keys(invoke.mock.calls[0]![1] as object)).toEqual(['method', 'params'])
})

test('a failure the shell rejects with becomes an OwnerError with its code', async () => {
  invoke.mockRejectedValue({ code: -32601, message: 'method not found: `x`' })
  const failure = await createTauriOwner().rpc('queue.list').catch((error: unknown) => error)
  expect(failure).toBeInstanceOf(OwnerError)
  expect(failure).toMatchObject({ code: -32601 })
})

test('subscribe listens on the one channel and hands the listener the payload', async () => {
  const stop = vi.fn()
  listen.mockResolvedValue(stop)
  const seen: unknown[] = []
  const unsubscribe = await createTauriOwner().subscribe((event) => seen.push(event))
  expect(listen.mock.calls[0]![0]).toBe(EVENT_CHANNEL)
  const payload = { method: 'queue.changed', params: {} }
  listen.mock.calls[0]![1]({ payload })
  expect(seen).toEqual([payload])
  expect(unsubscribe).toBe(stop)
})

test('the channel name is the one the shell emits on', () => {
  const lib = readFileSync(join(import.meta.dirname, '..', 'src-tauri', 'src', 'lib.rs'), 'utf8')
  expect(lib).toContain(`pub const EVENT_CHANNEL: &str = "${EVENT_CHANNEL}";`)
  expect(lib).toContain(`pub const RPC_COMMAND: &str = "${RPC_COMMAND}";`)
})

test('the shell\'s notice of the connection has the name the shell emits', () => {
  const shell = readFileSync(join(import.meta.dirname, '..', 'src-tauri', 'src', 'shell.rs'), 'utf8')
  expect(shell).toContain(`pub const CONNECTION_EVENT: &str = "${CONNECTION_EVENT}";`)
})
