import { isAbsolute, resolve } from 'node:path'
import type { Readable, Writable } from 'node:stream'
import { defineProvider } from '@collide/pack-sdk'
import type { GetResultInput, ListResultInput, ProviderHandlers } from '@collide/pack-sdk'
import { INITIALIZE_RESULT } from './declaration.ts'
import { scan } from './scan.ts'
import type { Scan } from './scan.ts'

/** A page of `list` is this many tasks. The corpus has about 1,600, so a full list is a few pages. */
export const PAGE_SIZE = 500

const CURSOR = /^p(\d+)$/

export interface ServeOptions {
  input?: Readable
  output?: Writable
  log?: (message: string) => void
  /** Tests make it small to page over a small tree. */
  pageSize?: number
}

/** The long id, or the short id, or the key, that a `get` may name a task by. */
const stripSource = (id: string): string => (id.startsWith('plan:') ? id.slice('plan:'.length) : id)

/**
 * `initialize`, `list` and `get` for one checkout's `arch/` tree. The root comes from `initialize` and from nowhere
 * else; until it has run, `list` and `get` fail. The tree is read again on each call, so a reply is never older than
 * the request.
 */
export function planHandlers(options: { log: (message: string) => void; pageSize?: number }): ProviderHandlers {
  const { log, pageSize = PAGE_SIZE } = options
  let root: string | null = null

  const read = async (checkout: string | undefined): Promise<Scan> => {
    if (root === null) {
      throw new Error("the provider has no root yet: send initialize, with the checkout's path as root, first")
    }
    if (checkout !== undefined && resolve(checkout) !== root) {
      throw new Error(`checkout ${JSON.stringify(checkout)} is not the checkout this provider was given (${root})`)
    }
    return scan(root, log)
  }

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
      const wanted = stripSource(id)
      const { tasks } = await read(checkout ?? undefined)
      const found = tasks.find(({ task }) => task.id === wanted || task.key === `plan:${wanted}`)
      return found === undefined ? {} : { task: { ...found.task, body: found.body } }
    },
    apply: () => {
      throw new Error('apply is not built yet: this provider reads only')
    },
  }
}

/** Serves the provider over `input` and `output` (stdin and stdout by default) until the input ends. */
export function servePlan({
  input,
  output,
  log = (message) => void process.stderr.write(`${message}\n`),
  pageSize,
}: ServeOptions = {}): Promise<void> {
  return defineProvider(planHandlers({ log, pageSize }), { input, output, log })
}
