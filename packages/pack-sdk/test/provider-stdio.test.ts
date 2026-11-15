import { spawn } from 'node:child_process'
import { join } from 'node:path'
import { expect, test } from 'vitest'

const script = join(import.meta.dirname, 'provider-script.ts')

test('a provider written with the SDK answers over real stdin and stdout and exits when stdin closes', async () => {
  const child = spawn(process.execPath, [script], { stdio: ['pipe', 'pipe', 'pipe'] })
  let stdout = ''
  let stderr = ''
  child.stdout.setEncoding('utf8').on('data', (chunk: string) => (stdout += chunk))
  child.stderr.setEncoding('utf8').on('data', (chunk: string) => (stderr += chunk))
  const exited = new Promise<number | null>((resolve) => child.on('close', resolve))

  const requests = [
    { jsonrpc: '2.0', id: 1, method: 'initialize', params: { config: { create: true } } },
    { jsonrpc: '2.0', id: 2, method: 'get', params: { id: '#7' } },
    { jsonrpc: '2.0', id: 3, method: 'apply', params: { op: 'setStatus', args: {} } },
    { jsonrpc: '2.0', id: 4, method: 'watch', params: { checkouts: [] } },
    { jsonrpc: '2.0', id: 5, method: 'nope' },
  ]
  child.stdin.write(requests.map((r) => `${JSON.stringify(r)}\n`).join(''))
  child.stdin.write('not json\n')
  child.stdin.end()

  expect(await exited).toBe(0)
  const messages = stdout.trimEnd().split('\n').map((line) => JSON.parse(line))
  expect(messages).toEqual([
    { jsonrpc: '2.0', id: 1, result: expect.objectContaining({ scope: 'global', capabilities: { watch: true, create: true } }) },
    { jsonrpc: '2.0', id: 2, result: { task: expect.objectContaining({ id: '#7' }) } },
    { jsonrpc: '2.0', id: 3, result: { outcome: 'refused', message: 'read-only' } },
    { jsonrpc: '2.0', method: 'source.stale', params: { reason: 'no source' } },
    { jsonrpc: '2.0', id: 4, result: {} },
    { jsonrpc: '2.0', id: 5, error: expect.objectContaining({ code: -32601 }) },
    { jsonrpc: '2.0', id: null, error: expect.objectContaining({ code: -32700 }) },
  ])
  expect(stderr).toContain('provider script started')
})
