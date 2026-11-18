import { isAbsolute, join } from 'node:path'
import type { Readable, Writable } from 'node:stream'
import { defineProvider } from '@collide/pack-sdk'
import type {
  GetResultInput,
  InitializeResultInput,
  KindDeclInput,
  ListResultInput,
  ProviderHandlers,
} from '@collide/pack-sdk'
import { isMainCheckout, resolveCheckout } from './checkout.ts'
import { ROOT_NAMES, TITLE_MAX } from './create.ts'
import { DEFAULT_LAYOUT, resolveLayout } from './layout.ts'
import type { Layout } from './layout.ts'
import { BLOCKED_BY, CATEGORIES, ID_PATTERN, IMPLEMENTS, LINK_TO_PATTERN, LINK_TYPES, PROPOSAL, PROPOSAL_STATUSES } from './item.ts'
import { scanEpics, toTask } from './scan.ts'
import type { Scan } from './scan.ts'
import { applyOp } from './write.ts'

const statuses = CATEGORIES.map((name) => ({ name, category: name }))
const kind = (name: string): KindDeclInput => ({ name, statuses })

/**
 * A proposal's statuses are its own six words, each with a category (0.10). It declares its own `setStatus`, which takes
 * only those words, because a category does not name one status: `rejected` and `superseded` are both `dropped`.
 */
const proposalKind: KindDeclInput = {
  name: PROPOSAL,
  statuses: [...PROPOSAL_STATUSES],
  ops: [
    {
      name: 'setStatus',
      args: { type: 'object', properties: { status: { enum: PROPOSAL_STATUSES.map((status) => status.name) } }, required: ['status'], additionalProperties: false },
    },
  ],
}

const noExtraArgs = { additionalProperties: false }

/**
 * The write ops, for both kinds. `create` makes an item, so it is sent with no `id` and no `version`; `reid` is sent with
 * an `id` and no `version`; the others change one item and need both. None has `movesTo`: a status write moves the item to the category its argument names,
 * which one fixed category cannot say, and `claim` records an owner without changing the status. `checkouts` is left
 * out (any): `apply` writes in the main checkout, or in the one that `checkout` names.
 */
const OPS: NonNullable<InitializeResultInput['ops']> = [
  {
    name: 'setStatus',
    args: {
      type: 'object',
      properties: { status: { enum: [...CATEGORIES] }, category: { enum: [...CATEGORIES] } },
      oneOf: [{ required: ['status'] }, { required: ['category'] }],
      ...noExtraArgs,
    },
  },
  {
    name: 'link',
    args: { type: 'object', properties: { to: { type: 'string', pattern: LINK_TO_PATTERN }, type: { enum: [BLOCKED_BY, IMPLEMENTS] } }, required: ['to'], ...noExtraArgs },
  },
  {
    name: 'claim',
    args: { type: 'object', properties: { claimant: { type: 'string', minLength: 1, maxLength: 200 } }, required: ['claimant'], ...noExtraArgs },
  },
  { name: 'unclaim', args: { type: 'object', properties: {}, ...noExtraArgs } },
  {
    name: 'create',
    args: {
      type: 'object',
      properties: {
        kind: { enum: ['task', 'epic', PROPOSAL] },
        epic: { type: 'string', pattern: '^ep-[0-9]{3,}-[a-z]{2}$' },
        title: { type: 'string', minLength: 1, maxLength: TITLE_MAX },
        seed: { type: 'integer' },
        root: { enum: [...ROOT_NAMES] },
      },
      required: ['title'],
      ...noExtraArgs,
    },
  },
  { name: 'reid', args: { type: 'object', properties: { seed: { type: 'integer' } }, ...noExtraArgs } },
]

/** `initialize`'s answer: kinds `task`, `epic` and `proposal`, the five categories as a task's statuses, `blocked-by` as the one blocking link, and `implements`, whose target is a feature. */
export const INITIALIZE_RESULT: InitializeResultInput = {
  scope: 'ref',
  idPattern: ID_PATTERN,
  shortIdPattern: '^[0-9]+(-[a-z]{2})?$',
  statuses,
  categoryStatus: Object.fromEntries(CATEGORIES.map((name) => [name, name])),
  kinds: [kind('task'), kind('epic'), proposalKind],
  ops: OPS,
  links: [...LINK_TYPES],
  capabilities: { watch: false, create: true, collections: false, preview: false },
}

export interface PlanProviderOptions {
  log: (message: string) => void
  /**
   * The pack's data folder, which the daemon gives as `COLLIDE_PACK_DATA`: the private root is `planning/` in it. A
   * value that is not an absolute path, or none, means there is no private root.
   */
  packData: string | null
}

/** The private root, `<pack data>/planning`, or null when there is no usable pack data folder. */
const privateRootOf = (packData: string | null): string | null =>
  packData !== null && isAbsolute(packData) ? join(packData, 'planning') : null

/**
 * Protocol v0's `initialize`, `list`, `get` and `apply` for the markdown provider. The repository root comes from
 * `initialize`'s `root` and from nowhere else, and the shared folders from its `config.layout` (checked there: an
 * `initialize` with a layout that may not be used fails with the reason); until it has run, `list` and `get` fail.
 */
export function planHandlers({ log, packData }: PlanProviderOptions): ProviderHandlers {
  let root: string | null = null
  let layout: Layout = DEFAULT_LAYOUT
  const privateRoot = privateRootOf(packData)

  const scan = async (checkout: string | null): Promise<Scan> => {
    if (root === null) throw new Error('the provider has no root yet: send initialize, with the main checkout\'s path as root, first')
    // A named checkout is read in place of the main checkout, so an item is listed with the version that a write
    // there will check; one that is not a checkout of the repository is an error.
    // The private root is the main checkout's: a call that names another checkout lists only the shared root.
    const where = checkout === null ? root : await resolveCheckout(root, checkout)
    const withPrivate = privateRoot !== null && (checkout === null || (await isMainCheckout(root, where)))
    const result = await scanEpics(where, checkout, layout, withPrivate ? privateRoot : null)
    if (result.invalid.length + result.unknownKind + result.brokenLink > 0) {
      log(`plan: ${result.items.length} items, ${result.invalid.length} invalid, ${result.unknownKind} with an unknown kind, ${result.brokenLink} with a broken link`)
    }
    return result
  }

  return {
    initialize: async ({ root: given, config }) => {
      if (typeof given !== 'string' || !isAbsolute(given)) throw new Error(`initialize needs root, the main checkout's absolute path; got ${JSON.stringify(given)}`)
      // Checked before anything is kept, so a refused layout leaves the provider as it was.
      layout = await resolveLayout(given, config?.layout)
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
    apply: (params) => {
      if (root === null) throw new Error('the provider has no root yet: send initialize, with the main checkout\'s path as root, first')
      return applyOp(root, params, {}, {}, layout, privateRoot)
    },
  }
}

export interface ServeOptions {
  input?: Readable
  output?: Writable
  log?: (message: string) => void
  /** The pack's data folder. `COLLIDE_PACK_DATA` when it is left out; null for none, whatever the environment says. */
  packData?: string | null
}

/** Serves the provider over `input` and `output` (stdin and stdout by default) until the input ends. */
export function servePlan({
  input,
  output,
  log = (message) => void process.stderr.write(`${message}\n`),
  packData = process.env.COLLIDE_PACK_DATA ?? null,
}: ServeOptions = {}): Promise<void> {
  return defineProvider(planHandlers({ log, packData }), { input, output, log })
}
