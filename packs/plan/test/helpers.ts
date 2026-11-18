import { execFileSync, spawn } from 'node:child_process'
import { createHash } from 'node:crypto'
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { PassThrough } from 'node:stream'
import { afterAll } from 'vitest'
import { servePlan } from '../src/provider.ts'

type Message = Record<string, any>

export const fixture = (name: string): string => join(import.meta.dirname, 'fixtures', name)

const temps: string[] = []
afterAll(() => temps.forEach((dir) => rmSync(dir, { recursive: true, force: true })))

/** A new temp directory that is removed when the test file ends. */
export function tempDir(): string {
  const dir = mkdtempSync(join(tmpdir(), 'plan-pack-'))
  temps.push(dir)
  return dir
}

/** A copy of a fixture tree that a test may change. */
export function copyFixture(name: string): string {
  const dir = tempDir()
  cpSync(fixture(name), dir, { recursive: true })
  return dir
}

/** Writes `files` (paths relative to `<root>/.collide/planning/epics/`) into a fresh temp repository. */
export function makeRepo(files: Record<string, string>): string {
  const dir = tempDir()
  for (const [path, text] of Object.entries(files)) {
    const full = join(dir, '.collide/planning/epics', path)
    mkdirSync(dirname(full), { recursive: true })
    writeFileSync(full, text)
  }
  return dir
}

export const frontmatter = (id: string, extra = ''): string =>
  `---\nid: ${id}\nkind: task\nstatus: ready\ntitle: "Made-up item"\nowners: []\nlinks: []\n${extra}---\n\n`

export const EPIC_FILE = '---\nid: ep-001-aa\nkind: epic\nstatus: active\ntitle: "Made-up epic"\nowners: []\nlinks: []\n---\n'

export interface Reply {
  result?: Message
  error?: Message
  /** The `invalid` list of a `list` reply (empty for any other reply). */
  invalid: Message[]
  logs: string[]
}

/**
 * A fake client for the provider: one request in, its reply out. `packData` is the pack's data folder, which the daemon
 * gives as `COLLIDE_PACK_DATA`: none when it is left out, whatever this process's environment says.
 */
export function connect({ packData = null }: { packData?: string | null } = {}) {
  const input = new PassThrough()
  const output = new PassThrough()
  output.setEncoding('utf8')
  const lines: string[] = []
  const waiting: Array<() => void> = []
  const logs: string[] = []
  let buffer = ''
  output.on('data', (chunk: string) => {
    buffer += chunk
    const parts = buffer.split('\n')
    buffer = parts.pop() ?? ''
    lines.push(...parts)
    waiting.splice(0).forEach((wake) => wake())
  })
  const done = servePlan({ input, output, log: (message) => logs.push(message), packData })
  let nextId = 1

  const request = async (method: string, params: unknown = {}): Promise<Reply> => {
    const id = nextId++
    input.write(`${JSON.stringify({ jsonrpc: '2.0', id, method, params })}\n`)
    while (lines.length === 0) await new Promise<void>((resolve) => waiting.push(resolve))
    const message: Message = JSON.parse(lines.shift() as string)
    if (message.id !== id) throw new Error(`unexpected message: ${JSON.stringify(message)}`)
    return { result: message.result, error: message.error, invalid: message.result?.invalid ?? [], logs: logs.splice(0) }
  }
  const close = async (): Promise<void> => {
    input.end()
    await done
  }
  return { request, close }
}

/** Runs one request against a provider that `initialize` has given `root`, and returns its reply. */
export async function call(root: string, method: string, params: unknown = {}): Promise<Reply> {
  const client = connect()
  try {
    if (method !== 'initialize') await client.request('initialize', { config: {}, root })
    else params = { config: {}, root, ...(params as object) }
    return await client.request(method, params)
  } finally {
    await client.close()
  }
}

/** Runs one request against a provider that has not been initialized. */
export async function callUninitialized(method: string, params: unknown = {}): Promise<Reply> {
  const client = connect()
  try {
    return await client.request(method, params)
  } finally {
    await client.close()
  }
}

/** What `version` is for a file: the sha256 of its content. */
export const fileVersion = (path: string): string => createHash('sha256').update(readFileSync(path)).digest('hex')

const main = join(import.meta.dirname, '..', 'src', 'main.ts')

/**
 * The provider as its own process (`node src/main.ts`), initialized for `root`. `env` is added to this process's
 * environment. A request whose process exits before it answers rejects, so a killed writer is a value to test.
 */
export async function spawnProvider(root: string, env: Record<string, string> = {}) {
  const child = spawn(process.execPath, [main], { cwd: tmpdir(), env: { ...process.env, ...env }, stdio: ['pipe', 'pipe', 'pipe'] })
  const pending = new Map<number, { resolve: (message: Message) => void; reject: (error: Error) => void }>()
  let buffer = ''
  let nextId = 1
  child.stdout.setEncoding('utf8').on('data', (chunk: string) => {
    buffer += chunk
    const parts = buffer.split('\n')
    buffer = parts.pop() ?? ''
    for (const part of parts) {
      const message: Message = JSON.parse(part)
      pending.get(message.id)?.resolve(message)
      pending.delete(message.id)
    }
  })
  child.stderr.resume()
  const exited = new Promise<{ code: number | null; signal: string | null }>((resolve) => {
    child.on('close', (code, signal) => {
      pending.forEach(({ reject }) => reject(new Error(`the provider exited (code ${code}, signal ${signal}) before it answered`)))
      resolve({ code, signal })
    })
  })
  const request = async (method: string, params: unknown = {}): Promise<Message> => {
    const id = nextId++
    const reply = new Promise<Message>((resolve, reject) => pending.set(id, { resolve, reject }))
    child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', id, method, params })}\n`)
    const message = await reply
    if (message.error !== undefined) throw new Error(`${method} failed: ${JSON.stringify(message.error)}`)
    return message.result as Message
  }
  await request('initialize', { config: {}, root })
  return {
    request,
    exited,
    close: async () => {
      child.stdin.end()
      return exited
    },
  }
}

/** Runs git in `cwd` with no user or system configuration, a made-up identity, and no signing. */
export const git = (cwd: string, ...args: string[]): string =>
  execFileSync('git', ['-c', 'user.name=Test', '-c', 'user.email=test@example.invalid', '-c', 'commit.gpgsign=false', '-c', 'init.defaultBranch=main', ...args], {
    cwd,
    encoding: 'utf8',
    env: { ...process.env, GIT_CONFIG_GLOBAL: '/dev/null', GIT_CONFIG_NOSYSTEM: '1' },
  })

/** Commits everything in `cwd`. */
export function commitAll(cwd: string, message: string): void {
  git(cwd, 'add', '-A')
  git(cwd, 'commit', '--quiet', '-m', message)
}
