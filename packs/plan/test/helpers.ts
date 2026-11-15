import { cpSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs'
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

/** A fake client for the provider: one request in, its reply out. `root` is what `initialize` will send. */
export function connect() {
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
  const done = servePlan({ input, output, log: (message) => logs.push(message) })
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
