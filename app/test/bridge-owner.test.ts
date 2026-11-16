import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'
import { afterEach, expect, test, vi } from 'vitest'
import { OwnerError } from '../src/owner/client.ts'
import { createBridgeOwner, readEvents } from '../src/owner/bridge.ts'

// The owner client of the browser tests (ts-0082-fp). The bridge it talks to is exercised by `browser/`; these check
// the client's own parts with a `fetch` that is a script.

const base = 'http://127.0.0.1:9'

afterEach(() => vi.unstubAllGlobals())

function answering(body: unknown, status = 200) {
  const fetchMock = vi.fn(async () => new Response(JSON.stringify(body), { status }))
  vi.stubGlobal('fetch', fetchMock)
  return fetchMock
}

test('a call posts the method and the params as JSON with the token as a bearer, and gives the result', async () => {
  const fetchMock = answering({ result: { boards: [] } })
  const result = await createBridgeOwner({ base, token: 't0k' }).rpc('board.list')
  expect(result).toEqual({ boards: [] })
  const [url, init] = fetchMock.mock.calls[0] as unknown as [string, RequestInit]
  expect(url).toBe(`${base}/rpc`)
  expect(init.method).toBe('POST')
  expect(init.headers).toEqual({ authorization: 'Bearer t0k', 'content-type': 'application/json' })
  expect(JSON.parse(init.body as string)).toEqual({ method: 'board.list' })
})

test('an error of the owner rejects with an OwnerError that has its code', async () => {
  answering({ error: { code: -32000, message: 'no such run' } })
  const failure = await createBridgeOwner({ base, token: 't' }).rpc('run.list').catch((error: unknown) => error)
  expect(failure).toBeInstanceOf(OwnerError)
  expect(failure).toMatchObject({ code: -32000, message: 'no such run' })
})

test('a refusal of the bridge itself is an OwnerError that says so', async () => {
  answering('no', 401)
  const failure = await createBridgeOwner({ base, token: 'x' }).rpc('run.list').catch((error: unknown) => error)
  expect(failure).toBeInstanceOf(OwnerError)
  expect((failure as OwnerError).message).toContain('401')
})

function streamOf(...chunks: string[]): ReadableStream<Uint8Array> {
  const encoder = new TextEncoder()
  return new ReadableStream({
    start(controller) {
      for (const chunk of chunks) controller.enqueue(encoder.encode(chunk))
      controller.close()
    },
  })
}

test('events are read from the stream even when a block is split across chunks, and comments are skipped', async () => {
  const seen: unknown[] = []
  await readEvents(
    streamOf(': open\n\ndata: {"method":"queue.changed","pa', 'rams":{}}\n\n: keep-alive\n\ndata: {"method":"b"}\n\n'),
    (event) => seen.push(event),
  )
  expect(seen).toEqual([{ method: 'queue.changed', params: {} }, { method: 'b' }])
})

test('subscribing sends the token, resolves once the stream is open and stops with the function it gives', async () => {
  const fetchMock = vi.fn(async (_url: string, init: RequestInit) => {
    expect(init.signal).toBeInstanceOf(AbortSignal)
    return new Response(streamOf(': open\n\n'), { status: 200 })
  })
  vi.stubGlobal('fetch', fetchMock)
  const stop = await createBridgeOwner({ base, token: 'abc' }).subscribe(() => undefined)
  const [url, init] = fetchMock.mock.calls[0] as unknown as [string, RequestInit]
  expect(url).toBe(`${base}/events`)
  expect(init.headers).toEqual({ authorization: 'Bearer abc' })
  const signal = init.signal as AbortSignal
  expect(signal.aborted).toBe(false)
  stop()
  expect(signal.aborted).toBe(true)
})

// ---- nothing of the bridge is in the app ------------------------------------------------------------------------

const SRC = join(import.meta.dirname, '..', 'src')

function sources(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name)
    return statSync(path).isDirectory() ? sources(path) : [path]
  })
}

test('only the bridge entry reads the bridge client, and index.html loads the app entry', () => {
  const readers = sources(SRC)
    .filter((file) => /\.(ts|tsx)$/.test(file))
    .filter((file) => /owner\/bridge|\.\/bridge/.test(readFileSync(file, 'utf8')))
    .map((file) => relative(SRC, file))
    .sort()
  expect(readers).toEqual(['bridge-main.tsx'])
  const index = readFileSync(join(SRC, '..', 'index.html'), 'utf8')
  expect(index).toContain('/src/main.tsx')
  expect(index).not.toContain('bridge')
})
