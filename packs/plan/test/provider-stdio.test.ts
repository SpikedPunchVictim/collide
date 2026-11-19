import { spawn } from 'node:child_process'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { expect, test } from 'vitest'
import { fixture } from './helpers.ts'

const main = join(import.meta.dirname, '..', 'src', 'main.ts')

/** Runs `node src/main.ts` in a directory that is not the repository, writes the requests to its stdin, closes it, and returns every stdout message. */
async function run(requests: object[]): Promise<{ messages: Array<Record<string, any>>; stderr: string }> {
  const child = spawn(process.execPath, [main], { cwd: tmpdir(), stdio: ['pipe', 'pipe', 'pipe'] })
  let stdout = ''
  let stderr = ''
  child.stdout.setEncoding('utf8').on('data', (chunk: string) => (stdout += chunk))
  child.stderr.setEncoding('utf8').on('data', (chunk: string) => (stderr += chunk))
  const exited = new Promise<number | null>((resolve) => child.on('close', resolve))
  child.stdin.end(requests.map((request) => `${JSON.stringify(request)}\n`).join(''))
  expect(await exited).toBe(0)
  return { messages: stdout.trimEnd().split('\n').map((line) => JSON.parse(line)), stderr }
}

const request = (id: number, method: string, params: object = {}) => ({ jsonrpc: '2.0', id, method, params })

const initialize = (root: string) => request(1, 'initialize', { config: {}, root })

test('the provider answers initialize, list and get over real stdio, in the repository that initialize names', async () => {
  const { messages, stderr } = await run([initialize(fixture('basic')), request(2, 'list'), request(3, 'get', { id: 'ts-0001-aa' }), request(4, 'get', { id: 'ts-9999-zz' })])
  expect(messages).toHaveLength(4)
  const kinds = ['task', 'epic', 'proposal', 'spike', 'question'].map((name) => ({ name }))
  expect(messages[0]).toMatchObject({ id: 1, result: { scope: 'ref', kinds } })
  const ids = messages[1]?.result.tasks.map((task: { id: string }) => task.id).sort()
  expect(ids).toEqual([
    'ep-001-aa', 'ep-002-bb', 'pr-0001-aa', 'qu-0001-aa', 'sp-0001-aa', 'ts-0001-aa', 'ts-0002-bb', 'ts-0003-cc',
    'ts-0004-dd',
  ])
  expect(messages[2]).toMatchObject({ id: 3, result: { task: { id: 'ts-0001-aa', body: expect.stringContaining('Made-up content') } } })
  expect(messages[3]).toEqual({ jsonrpc: '2.0', id: 4, result: {} })
  expect(stderr).toContain('1 with an unknown kind, 1 with a broken link')
})

test('invalid items come back in the list reply, and the provider writes no other notification', async () => {
  const { messages } = await run([initialize(fixture('hazards')), request(2, 'list')])
  expect(messages).toHaveLength(2)
  expect(messages.every((message) => message.method === undefined)).toBe(true)
  expect(messages[1]?.result.invalid).toHaveLength(11)
  expect(messages[1]?.result.invalid).toContainEqual(expect.objectContaining({ path: expect.stringContaining('ts-0102-aa-two.md'), checkout: null }))
})

test('list before initialize is an error reply, and the provider keeps serving', async () => {
  const { messages } = await run([request(1, 'list'), { ...initialize(fixture('basic')), id: 2 }, request(3, 'list')])
  expect(messages[0]).toMatchObject({ id: 1, error: { code: -32603 } })
  expect(messages[2]?.result.tasks).toHaveLength(9)
})
