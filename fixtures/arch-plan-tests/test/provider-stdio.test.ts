import { spawn } from 'node:child_process'
import { cpSync, existsSync, mkdirSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'
import { afterAll, expect, test } from 'vitest'
import { PACK, REPO, tree } from './helpers.ts'

const temp = mkdtempSync(join(tmpdir(), 'arch-plan-stdio-'))
afterAll(() => rmSync(temp, { recursive: true, force: true }))

/**
 * What Collide's pack cache makes of a repo pack: the pack's files, and the SDK linked at
 * `node_modules/@collide/pack-sdk`.
 * The link names a small shim that re-exports the SDK's source, so this test does not depend on the SDK's `dist`,
 * which `pnpm sdk` in another package deletes and rebuilds while the suites run side by side. Node follows the link to
 * the shim, which is outside `node_modules`, and strips types from the source the shim names.
 */
function materialize(): string {
  const shim = join(temp, 'sdk')
  if (!existsSync(shim)) {
    mkdirSync(shim)
    const source = (file: string) => pathToFileURL(join(REPO, 'packages', 'pack-sdk', 'src', file)).href
    writeFileSync(
      join(shim, 'package.json'),
      JSON.stringify({
        name: '@collide/pack-sdk',
        type: 'module',
        exports: { '.': './index.js', './yaml': './yaml.js' },
      }),
    )
    writeFileSync(join(shim, 'index.js'), `export * from '${source('index.ts')}'\n`)
    writeFileSync(join(shim, 'yaml.js'), `export * from '${source('yaml.ts')}'\n`)
  }
  const dir = mkdtempSync(join(temp, 'pack-'))
  cpSync(PACK, dir, { recursive: true })
  mkdirSync(join(dir, 'node_modules', '@collide'), { recursive: true })
  symlinkSync(shim, join(dir, 'node_modules', '@collide', 'pack-sdk'))
  return dir
}

/** Runs the pack's `providers/plan.ts` with `node`, a clean environment and a temp home; returns its messages. */
async function run(requests: object[]): Promise<{ messages: Array<Record<string, any>>; stderr: string }> {
  const dir = materialize()
  const child = spawn(process.execPath, [join(dir, 'providers', 'plan.ts')], {
    cwd: temp,
    env: { PATH: process.env.PATH ?? '', HOME: temp, COLLIDE_DAEMON_ROOT: temp },
    stdio: ['pipe', 'pipe', 'pipe'],
  })
  let stdout = ''
  let stderr = ''
  child.stdout.setEncoding('utf8').on('data', (chunk: string) => (stdout += chunk))
  child.stderr.setEncoding('utf8').on('data', (chunk: string) => (stderr += chunk))
  const exited = new Promise<number | null>((resolve) => child.on('close', resolve))
  try {
    child.stdin.end(requests.map((request) => `${JSON.stringify(request)}\n`).join(''))
    expect(await exited, stderr).toBe(0)
  } finally {
    // The provider exits when its input ends; this only matters if the assertion above failed first.
    if (child.exitCode === null) {
      child.kill('SIGKILL')
      await exited
    }
  }
  return { messages: stdout.trimEnd().split('\n').map((line) => JSON.parse(line)), stderr }
}

const request = (id: number, method: string, params: object = {}) => ({ jsonrpc: '2.0', id, method, params })

test('a real node child answers initialize, list and get on stdio for the root initialize names', async () => {
  const root = tree('basic')
  const { messages } = await run([
    request(1, 'initialize', { config: {}, root }),
    request(2, 'list'),
    request(3, 'get', { id: 'T-03-001' }),
    request(4, 'get', { id: 'T-99-999' }),
  ])
  expect(messages).toHaveLength(4)
  expect(messages[0]).toMatchObject({ id: 1, result: { scope: 'ref', capabilities: { collections: true } } })
  expect(messages[1]?.result.tasks.map((task: { key: string }) => task.key)).toHaveLength(8)
  expect(messages[1]?.result.collections.map((c: { id: string }) => c.id)).toEqual(['plan:E-07', 'plan:E-08'])
  expect(messages[2]).toMatchObject({ id: 3, result: { task: { key: 'plan:T-03-001' } } })
  expect(messages[3]).toEqual({ jsonrpc: '2.0', id: 4, result: {} })
})

test('the provider writes nothing but replies to stdout, and logs to stderr', async () => {
  const root = tree('hazards')
  const { messages, stderr } = await run([request(1, 'initialize', { config: {}, root }), request(2, 'list')])
  expect(messages).toHaveLength(2)
  expect(messages.every((message) => message.method === undefined)).toBe(true)
  expect(stderr).toMatch(/E-10-broken-epic\.md/)
})
