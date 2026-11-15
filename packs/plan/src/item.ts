import type { LinkTypeDeclInput } from '@collide/pack-sdk'

export const ID_PATTERN = '^[a-z]{2}-[0-9]{3,}-[a-z]{2}$'
/** What a `link`'s `to` may be: an item's id, or a feature of a proposal, `pr-0003-km#F2`. */
export const LINK_TO_PATTERN = `${ID_PATTERN.slice(0, -1)}(#F[1-9][0-9]*)?$`
const idRegex = new RegExp(ID_PATTERN)

/** A task's statuses are five of the categories themselves (0.10); an epic uses the same five. */
export const CATEGORIES = ['ready', 'active', 'review', 'done', 'dropped'] as const

/** The category a status maps to. Only a proposal's `deferred` is `backlog`. */
export type Category = 'backlog' | (typeof CATEGORIES)[number]

/** A proposal's six statuses, each with its category (0.10, kinds). The same list is the kind's declaration. */
export const PROPOSAL_STATUSES: ReadonlyArray<{ name: string; category: Category }> = [
  { name: 'draft', category: 'active' },
  { name: 'proposed', category: 'review' },
  { name: 'accepted', category: 'done' },
  { name: 'deferred', category: 'backlog' },
  { name: 'rejected', category: 'dropped' },
  { name: 'superseded', category: 'dropped' },
]

export const PROPOSAL = 'proposal'

export const BLOCKED_BY = 'blocked-by'
export const IMPLEMENTS = 'implements'

/**
 * The link types the provider holds. `blocked-by` is the one that blocks. `implements` is a task's link to the feature it
 * builds, so its target is `feature` (revision 14); the others name an item, which is what a declaration with no `target`
 * means.
 */
export const LINK_TYPES: ReadonlyArray<LinkTypeDeclInput> = [
  { name: BLOCKED_BY, dir: 'out', blocking: true },
  { name: 'part-of', dir: 'out', blocking: false },
  { name: IMPLEMENTS, dir: 'out', blocking: false, target: 'feature' },
]

/** What a link of `type` points at: a feature when the type is declared so, otherwise an item. */
export const targetOf = (type: string): 'item' | 'feature' => LINK_TYPES.find((declared) => declared.name === type)?.target ?? 'item'

export const KNOWN_KINDS = ['task', 'epic', PROPOSAL] as const

/** The statuses an item of `kind` may have: a proposal's six, and the five categories for any other kind. */
export const statusesOf = (kind: string): ReadonlyArray<{ name: string; category: Category }> =>
  kind === PROPOSAL ? PROPOSAL_STATUSES : CATEGORIES.map((name) => ({ name, category: name }))

/** The state an item of `kind` is in when its status is `status`: the status word and its category. */
export const stateOf = (kind: string, status: string): { status: string; category: Category } => {
  const found = statusesOf(kind).find((candidate) => candidate.name === status)
  if (found === undefined) throw new Error(`"${status}" is not a status of kind ${kind}`)
  return { status, category: found.category }
}

/** An extra frontmatter field: text, a flag or a list of text. Numbers and null are refused, see `checkField`. */
export type FieldValue = string | boolean | string[]

export interface ItemLink {
  type: string
  to: string
}

/** What the frontmatter of one item says, every field's type checked. */
export interface Item {
  id: string
  alias: string | null
  kind: string
  /** The status word as written: one of the kind's statuses (a proposal's own six, any other kind's five categories). */
  status: string
  title: string
  owners: string[]
  links: ItemLink[]
  fields: Record<string, FieldValue>
}

export type ItemResult = { ok: true; item: Item } | { ok: false; reason: string }

const KEYS = new Set(['id', 'alias', 'kind', 'status', 'title', 'owners', 'links'])

const describe = (value: unknown): string => {
  if (value === null) return 'null'
  if (Array.isArray(value)) return 'a list'
  if (typeof value === 'object') return 'a mapping'
  return `a ${typeof value}`
}

/** The hint every wrong-type message ends with: an unquoted `1e10` or `3e10` is a number to YAML. */
const QUOTE_HINT = 'quote a value that looks like a number'

const isString = (value: unknown): value is string => typeof value === 'string'

const checkField = (key: string, value: unknown): string | null => {
  if (isString(value) || typeof value === 'boolean') return null
  if (Array.isArray(value) && value.every(isString)) return null
  return `field "${key}" must be a string, a boolean or a list of strings, not ${describe(value)}; ${QUOTE_HINT}`
}

const checkLinks = (value: unknown): ItemLink[] | string => {
  if (!Array.isArray(value)) return `links must be a list, not ${describe(value)}`
  const links: ItemLink[] = []
  for (const [index, link] of value.entries()) {
    if (typeof link !== 'object' || link === null || Array.isArray(link)) return `links[${index}] must be a mapping with type and to`
    const entry = link as Record<string, unknown>
    const extra = Object.keys(entry).find((key) => key !== 'type' && key !== 'to')
    if (extra !== undefined) return `links[${index}] has the key "${extra}"; only type and to are allowed`
    if (!isString(entry.type) || entry.type === '') return `links[${index}].type must be a non-empty string, not ${describe(entry.type)}`
    if (!isString(entry.to)) return `links[${index}].to must be a string, not ${describe(entry.to)}; ${QUOTE_HINT}`
    if (entry.type === 'part-of') return `links[${index}] is part-of: the folder gives the epic, so frontmatter has no part-of`
    links.push({ type: entry.type, to: entry.to })
  }
  return links
}

/** Checks a parsed frontmatter mapping field by field. Nothing is coerced: a number where a string belongs is refused. */
export function readItem(frontmatter: Record<string, unknown>): ItemResult {
  const bad = (reason: string): ItemResult => ({ ok: false, reason })
  const keys = Object.keys(frontmatter)
  if (keys.includes('part-of')) return bad('part-of is not frontmatter: the folder gives the epic')
  if (keys.includes('__proto__')) return bad('the key "__proto__" is reserved')

  const { id, alias, kind, status, title, owners, links } = frontmatter
  if (!isString(id)) return bad(`id must be a string, not ${describe(id)}; ids look like ts-0012-ua (${QUOTE_HINT})`)
  if (!idRegex.test(id)) return bad(`id "${id}" does not match ${ID_PATTERN}`)
  if (alias !== undefined && !isString(alias)) return bad(`alias must be a string, not ${describe(alias)}; ${QUOTE_HINT}`)
  if (!isString(kind) || kind === '') return bad(`kind must be a non-empty string, not ${describe(kind)}`)
  const allowed = statusesOf(kind).map((candidate) => candidate.name)
  if (!isString(status) || !allowed.includes(status)) {
    return bad(`status must be one of ${allowed.join(', ')}${kind === PROPOSAL ? ' for a proposal' : ''}, not ${isString(status) ? `"${status}"` : describe(status)}`)
  }
  if (!isString(title) || title.trim() === '') return bad(`title must be a non-empty string, not ${describe(title)}; ${QUOTE_HINT}`)
  if (owners !== undefined && !(Array.isArray(owners) && owners.every(isString))) return bad(`owners must be a list of strings, not ${describe(owners)}`)

  const checkedLinks = links === undefined ? [] : checkLinks(links)
  if (isString(checkedLinks)) return bad(checkedLinks)

  const fields: Record<string, FieldValue> = {}
  for (const key of keys.filter((name) => !KEYS.has(name))) {
    const problem = checkField(key, frontmatter[key])
    if (problem !== null) return bad(problem)
    fields[key] = frontmatter[key] as FieldValue
  }

  return {
    ok: true,
    item: { id, alias: alias ?? null, kind, status, title, owners: owners ?? [], links: checkedLinks, fields },
  }
}
