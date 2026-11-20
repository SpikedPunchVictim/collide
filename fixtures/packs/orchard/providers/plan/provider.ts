import { isAbsolute, resolve } from 'node:path'
import type { Readable, Writable } from 'node:stream'
import { defineProvider } from '@collide/pack-sdk'
import type { GetResultInput, ListResultInput, ProviderHandlers } from '@collide/pack-sdk'
import { INITIALIZE_RESULT } from './declaration.ts'
import { isFolder } from './files.ts'
import { findTask, scan } from './scan.ts'
import type { Scan } from './scan.ts'
import { applyOp, previewOp } from './write.ts'
import { createWatcher } from './watcher.ts'
import type { Watcher, WatchOptions } from './watcher.ts'
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
  /** The debounce, the recheck, the clock and the file watcher of `watch`. Tests replace them. */
  watch?: WatchOptions
}

/** The handlers, and the one call that ends them: `stop` closes every watcher `watch` opened. */
export interface PlanHandlers extends ProviderHandlers {
  stop(): void
}

/**
 * `initialize`, `list`, `get`, `apply`, `preview` and `watch` for the checkouts of one repository. The root comes from
 * `initialize`: it is the checkout that a call without `checkout` means, and nothing is read from the working
 * directory. Until `initialize` has run, every other call fails. A call that names a checkout means that folder, which
 * must exist. The tree is read again on each call, so a reply is never older than the request. A write runs the
 * checkout's own `./plan`; the SDK answers one request at a time, in the order they came, so two writes in this
 * process never overlap. `list` and `get` run `./plan status` too when a question is open, for the human queue.
 */
export function planHandlers(options: {
  log: (message: string) => void
  pageSize?: number
  env?: NodeJS.ProcessEnv
  timeoutMs?: number
  watch?: WatchOptions
}): PlanHandlers {
  const { log, pageSize = PAGE_SIZE, env = process.env, timeoutMs = WRITE_TIMEOUT_MS } = options
  let root: string | null = null
  let watcher: Watcher | null = null

  /** The folder a call is for: the one it names, else the root `initialize` gave. */
  const checkoutFor = async (checkout: string | undefined): Promise<string> => {
    if (root === null) {
      throw new Error("the provider has no root yet: send initialize, with the checkout's path as root, first")
    }
    if (checkout === undefined) return root
    if (!isAbsolute(checkout)) throw new Error(`checkout ${JSON.stringify(checkout)} is not an absolute path`)
    if (!(await isFolder(checkout))) throw new Error(`checkout ${JSON.stringify(checkout)} is not a folder`)
    return resolve(checkout)
  }
  /** The checkout as `list` and `get` show it: with the human queue, read by the plan CLI when a question is open. */
  const read = async (checkout: string | undefined): Promise<Scan> =>
    scan(await checkoutFor(checkout), log, { env, timeoutMs })
  const writing = async (checkout: string | undefined): Promise<WriteContext> => ({
    root: await checkoutFor(checkout),
    env,
    timeoutMs,
    log,
  })

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
    apply: async (params) => applyOp(await writing(params.checkout ?? undefined), params),
    preview: async (params) => previewOp(await writing(params.checkout ?? undefined), params),
    watch: async ({ checkouts }, notify) => {
      await checkoutFor(undefined)
      if (!Array.isArray(checkouts) || checkouts.some((one) => typeof one !== 'string' || !isAbsolute(one))) {
        throw new Error(`watch needs checkouts, a list of absolute paths; got ${JSON.stringify(checkouts)}`)
      }
      watcher ??= createWatcher({ env, log, notify, timeoutMs, options: options.watch })
      await watcher.set(checkouts)
      return {}
    },
    stop: () => watcher?.stop(),
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
  watch,
}: ServeOptions = {}): Promise<void> {
  const handlers = planHandlers({ log, pageSize, env, timeoutMs, watch })
  // The watchers keep the process alive, so they end with the input.
  return defineProvider(handlers, { input, output, log }).finally(handlers.stop)
}
