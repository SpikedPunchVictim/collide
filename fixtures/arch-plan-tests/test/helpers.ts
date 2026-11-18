import { cpSync, mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { PassThrough } from 'node:stream'
import { afterAll } from 'vitest'
import { servePlan } from '../../../fixtures/packs/orchard/providers/plan/provider.ts'

type Message = Record<string, any>

/** What a test may give the provider besides its streams. `env` is the environment of the plan CLI it runs. */
export interface ProviderOptions {
  pageSize?: number
  env?: NodeJS.ProcessEnv
  timeoutMs?: number
}

export const REPO = join(import.meta.dirname, '..', '..', '..')
export const PACK = join(REPO, 'fixtures', 'packs', 'orchard')

/** A fixture tree: a checkout root that holds `arch/`. Tests read it and never write to it. */
export const tree = (name: string): string => join(REPO, 'fixtures', 'orchard-arch', name)

const temps: string[] = []
afterAll(() => temps.forEach((dir) => rmSync(dir, { recursive: true, force: true })))

/** A copy of a fixture tree that a test may change. It is removed when the test file ends. */
export function copyTree(name: string): string {
  const dir = mkdtempSync(join(tmpdir(), 'arch-plan-'))
  temps.push(dir)
  cpSync(tree(name), dir, { recursive: true })
  return dir
}

export interface Reply {
  result?: Message
  error?: Message
  logs: string[]
}

/** A fake client for the provider, in this process: one request in, its reply out. */
export function connect(options: ProviderOptions = {}) {
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
  const done = servePlan({ input, output, log: (message) => logs.push(message), ...options })
  let nextId = 1

  const request = async (method: string, params: unknown = {}): Promise<Reply> => {
    const id = nextId++
    input.write(`${JSON.stringify({ jsonrpc: '2.0', id, method, params })}\n`)
    while (lines.length === 0) await new Promise<void>((resolve) => waiting.push(resolve))
    const message: Message = JSON.parse(lines.shift() as string)
    if (message.id !== id) throw new Error(`unexpected message: ${JSON.stringify(message)}`)
    return { result: message.result, error: message.error, logs: logs.splice(0) }
  }
  /** Writes every request before it reads any reply, so the provider has them all at once. Replies come in order. */
  const requestMany = async (method: string, list: unknown[]): Promise<Reply[]> => {
    const ids = list.map(() => nextId++)
    list.forEach((params, at) => input.write(`${JSON.stringify({ jsonrpc: '2.0', id: ids[at], method, params })}\n`))
    const replies: Reply[] = []
    for (const id of ids) {
      while (lines.length === 0) await new Promise<void>((resolve) => waiting.push(resolve))
      const message: Message = JSON.parse(lines.shift() as string)
      if (message.id !== id) throw new Error(`unexpected message: ${JSON.stringify(message)}`)
      replies.push({ result: message.result, error: message.error, logs: logs.splice(0) })
    }
    return replies
  }
  const close = async (): Promise<void> => {
    input.end()
    await done
  }
  return { request, requestMany, close }
}

/** A provider that `initialize` has given `root`. `close` ends it. */
export async function start(root: string, options: ProviderOptions = {}) {
  const client = connect(options)
  const init = await client.request('initialize', { config: {}, root })
  if (init.error !== undefined) throw new Error(`initialize failed: ${JSON.stringify(init.error)}`)
  return client
}

/** One request against a provider that `initialize` has given `root`. */
export async function call(root: string, method: string, params: unknown = {}): Promise<Reply> {
  const client = await start(root)
  try {
    return await client.request(method, params)
  } finally {
    await client.close()
  }
}

/** Every task and the lists of the first page, for a provider that is not paged by the test. */
export async function listAll(root: string): Promise<Message> {
  const { result } = await call(root, 'list')
  return result as Message
}

export const taskOf = (result: Message, shortId: string): Message =>
  (result.tasks as Message[]).find((task) => task.key === `plan:${shortId}`) as Message

export const keysOf = (result: Message): string[] => (result.tasks as Message[]).map((task) => task.key as string)
