import { isAbsolute, resolve } from 'node:path'
import type { Readable, Writable } from 'node:stream'
import { defineProvider } from '@collide/pack-sdk'
import type { GetResultInput, ListResultInput, ProviderHandlers } from '@collide/pack-sdk'
import { INITIALIZE_RESULT } from './declaration.ts'
import { findTask, scan } from './scan.ts'
import type { Scan } from './scan.ts'
import { applyOp, previewOp } from './write.ts'
import type { WriteContext } from './write.ts'

/** A page of `list` is this many tasks. The corpus has about 1,600, so a full list is a few pages. */
export const PAGE_SIZE = 500

/** The plan CLI may take this long to write before it is killed. The Orchard tree has about 1,600 files. */
export const WRITE_TIMEOUT_MS = 120_000

const CURSOR = /^p(\d+)$/

export interface ServeOptions {
  input?: Readable
  output?: Writable
  log?: (message: string) => void
  /** Tests make it small to page over a small tree. */
  pageSize?: number
  /** The environment of the plan CLI that a write runs. The default is this process's. */
  env?: NodeJS.ProcessEnv
  /** How long the plan CLI may run before a write fails. */
  timeoutMs?: number
}

/**
 * `initialize`, `list`, `get`, `apply` and `preview` for one checkout's `arch/` tree. The root comes from `initialize`
 * and from nowhere else; until it has run, every other call fails. The tree is read again on each call, so a reply is
 * never older than the request. A write runs the checkout's own `./plan`; the SDK answers one request at a time, in
 * the order they came, so two writes to the checkout never overlap in this process.
 */
export function planHandlers(options: {
  log: (message: string) => void
  pageSize?: number
  env?: NodeJS.ProcessEnv
  timeoutMs?: number
}): ProviderHandlers {
  const { log, pageSize = PAGE_SIZE, env = process.env, timeoutMs = WRITE_TIMEOUT_MS } = options
  let root: string | null = null

  /** The root a call is for: the one `initialize` gave, which is the only checkout this provider reads or writes. */
  const rootFor = (checkout: string | undefined): string => {
    if (root === null) {
      throw new Error("the provider has no root yet: send initialize, with the checkout's path as root, first")
    }
    if (checkout !== undefined && resolve(checkout) !== root) {
      throw new Error(`checkout ${JSON.stringify(checkout)} is not the checkout this provider was given (${root})`)
    }
    return root
  }
  const read = async (checkout: string | undefined): Promise<Scan> => scan(rootFor(checkout), log)
  const writing = (checkout: string | undefined): WriteContext => ({ root: rootFor(checkout), env, timeoutMs, log })

  return {
    initialize: ({ root: given }) => {
      if (typeof given !== 'string' || !isAbsolute(given)) {
        throw new Error(`initialize needs root, the checkout's absolute path; got ${JSON.stringify(given)}`)
      }
      root = resolve(given)
      return INITIALIZE_RESULT
    },
    list: async ({ checkout, cursor }): Promise<ListResultInput> => {
      let from = 0
      if (cursor !== undefined && cursor !== null) {
        const match = CURSOR.exec(cursor)
        if (match === null) throw new Error(`cursor ${JSON.stringify(cursor)} is not one this provider gave`)
        from = Number(match[1])
      }
      const found = await read(checkout ?? undefined)
      const page = found.tasks.slice(from, from + pageSize).map(({ task }) => task)
      const next = from + pageSize < found.tasks.length ? `p${from + pageSize}` : undefined
      // The collections and the invalid files come with the first page, so walking every page gives each once.
      const first = from === 0
      return {
        tasks: page,
        collections: first ? found.collections : [],
        invalid: first ? found.invalid : [],
        ...(next === undefined ? {} : { nextCursor: next }),
      }
    },
    get: async ({ id, checkout }): Promise<GetResultInput> => {
      const found = findTask((await read(checkout ?? undefined)).tasks, id)
      return found === undefined ? {} : { task: { ...found.task, body: found.body } }
    },
    apply: (params) => applyOp(writing(params.checkout ?? undefined), params),
    preview: (params) => previewOp(writing(params.checkout ?? undefined), params),
  }
}

/** Serves the provider over `input` and `output` (stdin and stdout by default) until the input ends. */
export function servePlan({
  input,
  output,
  log = (message) => void process.stderr.write(`${message}\n`),
  pageSize,
  env,
  timeoutMs,
}: ServeOptions = {}): Promise<void> {
  return defineProvider(planHandlers({ log, pageSize, env, timeoutMs }), { input, output, log })
}
