import type { InitializeResultInput } from '@collide/pack-sdk'
import { declareKind, KIND_LINK_TYPES, KINDS, TASK } from './kinds.ts'
import {
  ANY_ID_PATTERN,
  ANY_SHORT_ID_PATTERN,
  EPIC_ARG,
  LABEL_ID_PATTERN,
  PROPOSAL_ARG,
  SHORT_ID_PATTERN,
} from './ids.ts'

export const BLOCKED_BY = 'blocked-by'

/** The statuses a task may have, in order, each with its category. */
export const STATUSES = [
  { name: 'specified', category: 'backlog' },
  { name: 'ready', category: 'ready' },
  { name: 'doing', category: 'active' },
  { name: 'in_review', category: 'review' },
  { name: 'done', category: 'done' },
  { name: 'dropped', category: 'dropped' },
] as const

export type Status = (typeof STATUSES)[number]

/** The status that writing each category produces. Each category has one status in this source. */
export const CATEGORY_STATUS: Record<string, string> = Object.fromEntries(
  STATUSES.map((status) => [status.category, status.name]),
)

/** A title is one line, at most this long. The plan CLI makes the file name from it. */
export const TITLE_MAX = 200

const noExtraArgs = { additionalProperties: false }

/**
 * `setStatus` takes a status or a category (a category is written as the status in `categoryStatus`). `create` takes
 * what the plan CLI's `new task` takes: the proposal that specifies the task, a title, the owning epic and, if any,
 * the tasks it depends on. Neither declares `movesTo`: where a status write goes is the status it names. `checkouts`
 * is left out (any).
 */
const OPS: NonNullable<InitializeResultInput['ops']> = [
  {
    name: 'setStatus',
    args: {
      type: 'object',
      properties: {
        status: { enum: STATUSES.map((status) => status.name) },
        category: { enum: STATUSES.map((status) => status.category) },
      },
      oneOf: [{ required: ['status'] }, { required: ['category'] }],
      ...noExtraArgs,
    },
  },
  {
    name: 'create',
    args: {
      type: 'object',
      properties: {
        proposal: { type: 'string', pattern: PROPOSAL_ARG },
        title: { type: 'string', minLength: 1, maxLength: TITLE_MAX },
        epic: { type: 'string', pattern: EPIC_ARG },
        dependsOn: { type: 'array', items: { type: 'string', pattern: SHORT_ID_PATTERN }, uniqueItems: true },
      },
      required: ['proposal', 'title', 'epic'],
      ...noExtraArgs,
    },
  },
]

/**
 * What `initialize` answers. A category is written as the one status that has it, which is what `categoryStatus` says.
 * The statuses at the top are a task's, and the first kind is the task, with the same statuses (every item names its
 * kind, a task `task`). The others are an ADR, a spike and a question. The ops are a task's: `setStatus` and `create`
 * on another kind are refused (`write.ts`).
 */
export const INITIALIZE_RESULT: InitializeResultInput = {
  scope: 'ref',
  idPattern: ANY_ID_PATTERN,
  shortIdPattern: ANY_SHORT_ID_PATTERN,
  templateFields: [{ name: 'labelId', pattern: LABEL_ID_PATTERN }],
  statuses: STATUSES.map((status) => ({ ...status })),
  categoryStatus: CATEGORY_STATUS,
  fields: [
    { id: 'proposal', type: 'string', filterable: true },
    { id: 'epic', type: 'string', filterable: true },
  ],
  kinds: [declareKind(TASK, STATUSES.map((status) => ({ ...status }))), ...KINDS],
  ops: OPS,
  links: [{ name: BLOCKED_BY, dir: 'out', blocking: true }, ...KIND_LINK_TYPES],
  capabilities: { watch: true, create: true, collections: true, preview: true },
}
