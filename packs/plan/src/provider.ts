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
import { ROOT_NAMES, RUN_MAX, TITLE_MAX } from './create.ts'
import { MOVE } from './move.ts'
import { ADD_FEATURE, FEATURE_PATTERN, MOVE_FEATURE } from './feature-ops.ts'
import { SECTION_MAX } from './features.ts'
import { DEFAULT_LAYOUT, resolveLayout } from './layout.ts'
import type { Layout } from './layout.ts'
import {
  BLOCKED_BY,
  CATEGORIES,
  ID_PATTERN,
  IMPLEMENTS,
  INCLUDES,
  INFORMS,
  LINK_TO_PATTERN,
  LINK_TYPES,
  PROPOSAL,
  PROPOSAL_STATUSES,
  QUESTION,
  QUESTION_STATUSES,
  SPIKE,
  SPIKE_STATUSES,
  SUPERSEDES,
} from './item.ts'
import type { Category } from './item.ts'
import { scanEpics, toTask } from './scan.ts'
import type { Scan } from './scan.ts'
import { ACCEPT, DECISION_MAX, DEFER, NOTE_MAX, REJECT, RESOLVE } from './person-ops.ts'
import { RENAME_SECTION } from './rename-section.ts'
import { applyOp } from './write.ts'

const noExtraArgs = { additionalProperties: false }
type OpDecls = NonNullable<KindDeclInput['ops']>

const statuses = CATEGORIES.map((name) => ({ name, category: name }))
const kind = (name: string): KindDeclInput => ({ name, statuses })

/**
 * A kind with statuses of its own declares its own `setStatus`, which takes only those words, because a category does
 * not name one status: a proposal's `rejected` and `superseded` are both `dropped`. A spike's five words each have a
 * category to themselves, but it declares its own too, as a proposal does, so the one rule holds for all of them.
 * `settable` is the words the op takes, all of them unless a kind says fewer.
 */
const ownStatusKind = (
  name: string,
  own: ReadonlyArray<{ name: string; category: Category }>,
  settable: readonly string[] = own.map((status) => status.name),
  more: OpDecls = [],
): KindDeclInput => ({
  name,
  statuses: [...own],
  ops: [
    {
      name: 'setStatus',
      args: {
        type: 'object',
        properties: { status: { enum: [...settable] } },
        required: ['status'],
        additionalProperties: false,
      },
    },
    ...more,
  ],
})

/** The ops that only a person makes (0.10): each is declared for the kind it is for. */
const noArgs = { type: 'object', properties: {}, ...noExtraArgs }
const note = { type: 'string', minLength: 1, maxLength: NOTE_MAX }
const PROPOSAL_OPS: OpDecls = [
  { name: ACCEPT, args: noArgs },
  {
    name: DEFER,
    args: { type: 'object', properties: { trigger: note }, required: ['trigger'], ...noExtraArgs },
  },
  { name: REJECT, args: { type: 'object', properties: { reason: note }, ...noExtraArgs } },
]
/**
 * The ops that write a proposal's Features part. `section` is a section name of at most `SECTION_MAX` characters,
 * matched after trimming and in any case, and may be empty, for the Unsorted part. A new feature's number is one past
 * the highest in the file, so a number that was removed from the file can be given again: the description says so.
 */
const sectionArg = (description: string) => ({ type: 'string', maxLength: SECTION_MAX, description })
const FEATURE_OPS: OpDecls = [
  {
    name: ADD_FEATURE,
    args: {
      type: 'object',
      description:
        'Adds a feature at the end of a section. It takes the next number, one past the highest F<n> in the file, ' +
        'so a number that was removed from the file can be given again.',
      properties: {
        section: sectionArg('The section to add it to, made when it is missing. Empty means Unsorted.'),
        title: { type: 'string', minLength: 1, maxLength: TITLE_MAX, description: 'One line, with no **.' },
        summary: { type: 'string', minLength: 1, description: 'One line, or several that continue it.' },
      },
      required: ['section', 'title', 'summary'],
      ...noExtraArgs,
    },
  },
  {
    name: MOVE_FEATURE,
    args: {
      type: 'object',
      description: 'Moves a feature, with its number and its continuation lines, to the end of a section.',
      properties: {
        feature: { type: 'string', pattern: FEATURE_PATTERN, description: 'The feature to move, such as F2.' },
        section: sectionArg('The section to move it to, made when it is missing. Empty means Unsorted.'),
      },
      required: ['feature', 'section'],
      ...noExtraArgs,
    },
  },
]
const QUESTION_OPS: OpDecls = [
  {
    name: RESOLVE,
    args: {
      type: 'object',
      properties: {
        by: { type: 'string', pattern: ID_PATTERN },
        text: { type: 'string', minLength: 1, maxLength: DECISION_MAX },
      },
      required: ['text'],
      ...noExtraArgs,
    },
  },
]

const proposalKind = ownStatusKind(PROPOSAL, PROPOSAL_STATUSES, undefined, [...PROPOSAL_OPS, ...FEATURE_OPS])
const spikeKind = ownStatusKind(SPIKE, SPIKE_STATUSES)
/** A question is resolved by the `resolve` op, which records who and what, so `setStatus` takes the other two. */
const questionKind = ownStatusKind(QUESTION, QUESTION_STATUSES, ['open', 'dropped'], QUESTION_OPS)

/**
 * The write ops, for every kind. `create` makes an item, so it is sent with no `id` and no `version`; `reid` is sent
 * with an `id` and no `version`; `renameSection` renames a section in every proposal that has it, so it is sent with
 * neither, and may carry `versions`, the version of each file the caller read; the others change one item and need
 * both. None has `movesTo`: a status write moves the item to the category its argument names, which one fixed category
 * cannot say, and `claim` records an owner without changing the status. `checkouts` is left out (any): `apply` writes
 * in the main checkout, or in the one that `checkout` names.
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
    args: {
      type: 'object',
      properties: {
        to: { type: 'string', pattern: LINK_TO_PATTERN },
        type: { enum: [BLOCKED_BY, IMPLEMENTS, INFORMS, INCLUDES, SUPERSEDES] },
      },
      required: ['to'],
      ...noExtraArgs,
    },
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
        kind: { enum: ['task', 'epic', PROPOSAL, SPIKE, QUESTION] },
        epic: { type: 'string', pattern: '^ep-[0-9]{3,}-[a-z]{2}$' },
        title: { type: 'string', minLength: 1, maxLength: TITLE_MAX },
        seed: { type: 'integer' },
        root: { enum: [...ROOT_NAMES] },
        raisedBy: { type: 'string', pattern: ID_PATTERN, description: 'The item that raised it: a raised-by link.' },
        run: { type: 'string', minLength: 1, maxLength: RUN_MAX, description: 'The run that raised it: a run: line.' },
      },
      required: ['title'],
      ...noExtraArgs,
    },
  },
  {
    name: MOVE,
    args: {
      type: 'object',
      description: 'Keeps one private item: moves it to the shared root, in the folder of its epic, under its id.',
      properties: {},
      ...noExtraArgs,
    },
  },
  { name: 'reid', args: { type: 'object', properties: { seed: { type: 'integer' } }, ...noExtraArgs } },
  {
    name: RENAME_SECTION,
    args: {
      type: 'object',
      description:
        'Renames a section in every proposal that has it, whatever its status. Where `to` is already a section, ' +
        'the entries are merged into it.',
      properties: {
        from: { type: 'string', minLength: 1, description: 'The section to rename. It cannot be Unsorted.' },
        to: sectionArg('The new name, 1 to 40 characters.'),
        versions: {
          type: 'object',
          additionalProperties: { type: 'string' },
          description: 'The version of each proposal you read, by id. A file that has moved on is a conflict.',
        },
      },
      required: ['from', 'to'],
      ...noExtraArgs,
    },
  },
]

/**
 * `initialize`'s answer: kinds `task`, `epic`, `proposal`, `spike` and `question`, the five categories as a task's
 * statuses, `blocked-by` as the one blocking link, `implements`, whose target is a feature, and `informs`, `includes`
 * and `supersedes`.
 */
export const INITIALIZE_RESULT: InitializeResultInput = {
  scope: 'ref',
  idPattern: ID_PATTERN,
  shortIdPattern: '^[0-9]+(-[a-z]{2})?$',
  statuses,
  categoryStatus: Object.fromEntries(CATEGORIES.map((name) => [name, name])),
  kinds: [kind('task'), kind('epic'), proposalKind, spikeKind, questionKind],
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
      return { tasks: items.map((found) => toTask(found, false, items)), invalid }
    },
    get: async ({ id, checkout }): Promise<GetResultInput> => {
      const { items } = await scan(checkout ?? null)
      const found = items.find((candidate) => candidate.item.id === id)
      return found === undefined ? {} : { task: toTask(found, true, items) }
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
