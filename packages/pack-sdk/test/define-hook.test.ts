import { spawn, type ChildProcess } from 'node:child_process'
import {
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, beforeEach, expect, test } from 'vitest'
import { createHookContext } from '../src/context.ts'
import type { HookInputOutput } from '../src/index.ts'

const script = join(import.meta.dirname, 'hook-script.ts')

// Every test runs the hook as a real child with a cleared environment: only the variables a test names, and a temp
// `HOME`. The child ends on its own when stdin has ended; the test's own timeout is the deadline, and `afterEach`
// kills a child that is still there.
let root: string
const children: ChildProcess[] = []
beforeEach(() => {
  root = mkdtempSync(join(tmpdir(), 'pack-sdk-hook-'))
  mkdirSync(join(root, 'out'))
})
afterEach(() => {
  for (const child of children.splice(0)) child.kill('SIGKILL')
  rmSync(root, { recursive: true, force: true })
})

const input: HookInputOutput = {
  protocol: 1,
  hook: 'gate',
  pack: 'fixture',
  workflow: null,
  board: 'board',
  line: 'main',
  experiment: null,
  run: null,
  launch: {},
  config: { threshold: 3, names: ['a', 'b'] },
  tasks: [],
  phase: null,
  workspace: null,
  history: [],
  state: { run: { branch: 'x' }, workspace: { head: 'abc' }, point: {} },
}

const resultPath = () => join(root, 'out', 'result.json')
const dataDir = () => join(root, 'data')

interface Ran {
  code: number | null
  stdout: string
  stderr: string
}

/** Runs `hook-script.ts` with `behavior`, writing `stdin` to it, and waits for it to exit. */
async function runHook(behavior: string, stdin: string, env: Record<string, string>): Promise<Ran> {
  const child = spawn(process.execPath, [script, behavior], {
    env: { HOME: root, ...env },
    stdio: ['pipe', 'pipe', 'pipe'],
  })
  children.push(child)
  let stdout = ''
  let stderr = ''
  child.stdout.setEncoding('utf8').on('data', (chunk: string) => (stdout += chunk))
  child.stderr.setEncoding('utf8').on('data', (chunk: string) => (stderr += chunk))
  child.stdin.on('error', () => {})
  const closed = new Promise<number | null>((resolve) => child.on('close', resolve))
  child.stdin.end(stdin)
  return { code: await closed, stdout, stderr }
}

const withPaths = () => ({ COLLIDE_RESULT: resultPath(), COLLIDE_PACK_DATA: dataDir() })
const outFolder = () => readdirSync(join(root, 'out'))
const resultFile = () => JSON.parse(readFileSync(resultPath(), 'utf8'))

test('a hook that returns ok leaves exactly its result in the file and exits 0', async () => {
  const ran = await runHook('ok', JSON.stringify(input), withPaths())
  expect(ran.code).toBe(0)
  expect(resultFile()).toEqual({ status: 'ok', outputs: { a: 1 } })
})

test('the result is written whole: one file in the folder, and nothing left of the temp file', async () => {
  await runHook('ok', JSON.stringify(input), withPaths())
  expect(outFolder()).toEqual(['result.json'])
})

test('the result replaces what stood at the path: a symlink is replaced, not written through', async () => {
  const elsewhere = join(root, 'elsewhere.txt')
  writeFileSync(elsewhere, 'old')
  symlinkSync(elsewhere, resultPath())
  const ran = await runHook('ok', JSON.stringify(input), withPaths())
  expect(ran.code).toBe(0)
  expect(lstatSync(resultPath()).isFile()).toBe(true)
  expect(resultFile()).toEqual({ status: 'ok', outputs: { a: 1 } })
  expect(readFileSync(elsewhere, 'utf8')).toBe('old')
  expect(outFolder()).toEqual(['result.json'])
})

test('a result that cannot be put in place exits non-zero with the reason and leaves no temp file', async () => {
  mkdirSync(resultPath())
  writeFileSync(join(resultPath(), 'inside'), 'x')
  const ran = await runHook('ok', JSON.stringify(input), withPaths())
  expect(ran.code).not.toBe(0)
  expect(ran.stderr).not.toBe('')
  expect(outFolder()).toEqual(['result.json'])
  expect(lstatSync(resultPath()).isDirectory()).toBe(true)
})

test('a pending result keeps its retryAfter and its message', async () => {
  const ran = await runHook('pending', JSON.stringify(input), withPaths())
  expect(ran.code).toBe(0)
  expect(resultFile()).toEqual({ status: 'pending', message: 'not yet', retryAfter: 30 })
})

test('an unverified result is written as it is', async () => {
  const ran = await runHook('unverified', JSON.stringify(input), withPaths())
  expect(ran.code).toBe(0)
  expect(resultFile()).toEqual({ status: 'unverified', message: 'no network here' })
})

test('a handler that throws exits non-zero with its message on stderr and writes no file', async () => {
  const ran = await runHook('throws', JSON.stringify(input), withPaths())
  expect(ran.code).not.toBe(0)
  expect(ran.stderr).toContain('the handler blew up')
  expect(outFolder()).toEqual([])
})

test('a handler that returns nothing, with no result file written, exits non-zero and says so', async () => {
  const ran = await runHook('nothing', JSON.stringify(input), withPaths())
  expect(ran.code).not.toBe(0)
  expect(ran.stderr).toContain('no result')
  expect(outFolder()).toEqual([])
})

test.each([
  ['a status outside the six', 'bad-status', /status/],
  ['a result that is not an object', 'not-an-object', /not an object/],
])('%s is not written: the hook exits non-zero and says why', async (_name, behavior, why) => {
  const ran = await runHook(behavior, JSON.stringify(input), withPaths())
  expect(ran.code).not.toBe(0)
  expect(ran.stderr).toMatch(why)
  expect(outFolder()).toEqual([])
})

test.each([
  ['text that is not JSON', 'not json'],
  ['an empty stdin', ''],
  ['a JSON array', '[1]'],
  ['JSON null', 'null'],
  ['a JSON string', '"x"'],
  ['two JSON objects', '{}{}'],
])('input that is %s exits non-zero with the reason and leaves no file', async (_name, stdin) => {
  const ran = await runHook('ok', stdin, withPaths())
  expect(ran.code).not.toBe(0)
  expect(ran.stderr).toContain('hook input')
  expect(outFolder()).toEqual([])
})

test('an unset COLLIDE_RESULT exits non-zero with a message that names it and runs no handler', async () => {
  const ran = await runHook('throws', JSON.stringify(input), { COLLIDE_PACK_DATA: dataDir() })
  expect(ran.code).not.toBe(0)
  expect(ran.stderr).toContain('COLLIDE_RESULT')
  expect(ran.stderr).not.toContain('blew up')
})

test('state, config and dataDir are the input and the environment, as the handler reads them', async () => {
  const ran = await runHook('context', JSON.stringify(input), withPaths())
  expect(ran.code).toBe(0)
  expect(resultFile().outputs).toEqual({
    hook: 'gate',
    state: input.state,
    config: input.config,
    dataDir: dataDir(),
  })
})

test('reading dataDir with COLLIDE_PACK_DATA unset fails the hook with a message that names the variable', async () => {
  const ran = await runHook('data-dir-only', JSON.stringify(input), { COLLIDE_RESULT: resultPath() })
  expect(ran.code).not.toBe(0)
  expect(ran.stderr).toContain('COLLIDE_PACK_DATA')
  expect(outFolder()).toEqual([])
})

test('a hook that never reads dataDir does not need COLLIDE_PACK_DATA', async () => {
  const ran = await runHook('ok', JSON.stringify(input), { COLLIDE_RESULT: resultPath() })
  expect(ran.code).toBe(0)
})

// The context itself, built in this process from a sample input.

const withEnv = (name: string, value: string | undefined, body: () => void) => {
  const before = process.env[name]
  if (value === undefined) delete process.env[name]
  else process.env[name] = value
  try {
    body()
  } finally {
    if (before === undefined) delete process.env[name]
    else process.env[name] = before
  }
}

test('createHookContext gives the input state and config, and the environment data folder', () => {
  withEnv('COLLIDE_PACK_DATA', '/data/pack', () => {
    const ctx = createHookContext(input)
    expect(ctx.state).toEqual(input.state)
    expect(ctx.config).toEqual(input.config)
    expect(ctx.dataDir).toBe('/data/pack')
  })
})

test('createHookContext reads dataDir when asked: building it with the variable unset does not throw', () => {
  withEnv('COLLIDE_PACK_DATA', undefined, () => {
    const ctx = createHookContext(input)
    expect(() => ctx.dataDir).toThrow('COLLIDE_PACK_DATA')
  })
})

test('an empty COLLIDE_PACK_DATA is as good as unset', () => {
  withEnv('COLLIDE_PACK_DATA', '', () => {
    expect(() => createHookContext(input).dataDir).toThrow('COLLIDE_PACK_DATA')
  })
})

test('ctx.ok, ctx.fail and ctx.refuse still throw the stage 7 tag', () => {
  const ctx = createHookContext(input)
  expect(() => ctx.ok({ a: 1 })).toThrow('unimplemented: sdk: ctx.result')
  expect(() => ctx.fail('no')).toThrow('unimplemented: sdk: ctx.result')
  expect(() => ctx.refuse('no')).toThrow('unimplemented: sdk: ctx.result')
})

test('ctx.report.span, metric and progress still throw the stage 7 tag', () => {
  const ctx = createHookContext(input)
  expect(() => ctx.report.metric('tests.passed', 1)).toThrow('unimplemented: sdk: ctx.report')
  expect(() => ctx.report.progress(1, 2)).toThrow('unimplemented: sdk: ctx.report')
  expect(() => ctx.report.span('build', () => 1)).toThrow('unimplemented: sdk: ctx.report')
})
