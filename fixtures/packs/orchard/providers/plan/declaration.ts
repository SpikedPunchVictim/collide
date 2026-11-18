import type { InitializeResultInput } from '@collide/pack-sdk'
import { ID_PATTERN, LABEL_ID_PATTERN, SHORT_ID_PATTERN } from './ids.ts'

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

/**
 * What `initialize` answers. No `ops`, `create`, `preview` or `watch` yet: this is the read side. A category is
 * written as the one status that has it, which is what `categoryStatus` says.
 */
export const INITIALIZE_RESULT: InitializeResultInput = {
  scope: 'ref',
  idPattern: ID_PATTERN,
  shortIdPattern: SHORT_ID_PATTERN,
  templateFields: [{ name: 'labelId', pattern: LABEL_ID_PATTERN }],
  statuses: STATUSES.map((status) => ({ ...status })),
  categoryStatus: Object.fromEntries(STATUSES.map((status) => [status.category, status.name])),
  fields: [
    { id: 'proposal', type: 'string', filterable: true },
    { id: 'epic', type: 'string', filterable: true },
  ],
  ops: [],
  links: [{ name: BLOCKED_BY, dir: 'out', blocking: true }],
  capabilities: { watch: false, create: false, collections: true, preview: false },
}
