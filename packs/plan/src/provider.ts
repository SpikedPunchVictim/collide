import { isAbsolute } from 'node:path'
import type { Readable, Writable } from 'node:stream'
import { defineProvider } from '@collide/pack-sdk'
import type {
  GetResultInput,
  InitializeResultInput,
  KindDeclInput,
  ListResultInput,
  ProviderHandlers,
} from '@collide/pack-sdk'
import { CATEGORIES, ID_PATTERN } from './item.ts'
import { scanEpics, toTask } from './scan.ts'
import type { Scan } from './scan.ts'

const statuses = CATEGORIES.map((name) => ({ name, category: name }))
const kind = (name: string): KindDeclInput => ({ name, statuses })

/** `initialize`'s answer: kinds `task` and `epic`, the five categories as statuses, `blocked-by` as the one blocking link. */
export const INITIALIZE_RESULT: InitializeResultInput = {
  scope: 'ref',
  idPattern: ID_PATTERN,
  shortIdPattern: '^[0-9]+(-[a-z]{2})?$',
  statuses,
  categoryStatus: Object.fromEntries(CATEGORIES.map((name) => [name, name])),
  kinds: [kind('task'), kind('epic')],
  links: [
    { name: 'blocked-by', dir: 'out', blocking: true },
    { name: 'part-of', dir: 'out', blocking: false },
  ],
  capabilities: { watch: false, create: false, collections: false, preview: false },
}

export interface PlanProviderOptions {
  log: (message: string) => void
}

/**
 * Protocol v0's `initialize`, `list` and `get` for the markdown provider. The repository root comes from
 * `initialize`'s `root` and from nowhere else; until it has run, `list` and `get` fail.
 */
export function planHandlers({ log }: PlanProviderOptions): ProviderHandlers {
  let root: string | null = null

  const scan = async (checkout: string | null): Promise<Scan> => {
    if (root === null) throw new Error('the provider has no root yet: send initialize, with the main checkout\'s path as root, first')
    const result = await scanEpics(root, checkout)
    if (result.invalid.length + result.unknownKind + result.brokenLink > 0) {
      log(`plan: ${result.items.length} items, ${result.invalid.length} invalid, ${result.unknownKind} with an unknown kind, ${result.brokenLink} with a broken link`)
    }
    return result
  }

  return {
    initialize: ({ root: given }) => {
      if (typeof given !== 'string' || !isAbsolute(given)) throw new Error(`initialize needs root, the main checkout's absolute path; got ${JSON.stringify(given)}`)
      root = given
      return INITIALIZE_RESULT
    },
    list: async ({ checkout }): Promise<ListResultInput> => {
      const { items, invalid } = await scan(checkout ?? null)
      return { tasks: items.map((found) => toTask(found, false)), invalid }
    },
    get: async ({ id, checkout }): Promise<GetResultInput> => {
      const found = (await scan(checkout ?? null)).items.find((candidate) => candidate.item.id === id)
      return found === undefined ? {} : { task: toTask(found, true) }
    },
    apply: () => ({ outcome: 'refused', message: 'the markdown provider is read-only until its write ops land' }),
  }
}

export interface ServeOptions {
  input?: Readable
  output?: Writable
  log?: (message: string) => void
}

/** Serves the provider over `input` and `output` (stdin and stdout by default) until the input ends. */
export function servePlan({ input, output, log = (message) => void process.stderr.write(`${message}\n`) }: ServeOptions = {}): Promise<void> {
  return defineProvider(planHandlers({ log }), { input, output, log })
}
