import type { KindDeclInput, LinkTypeDeclInput, TaskInput } from '@collide/pack-sdk'
import { adrRef, obligationRef, spikeRef } from './ids.ts'
import type { NameIds } from './ids.ts'

/**
 * The kinds the provider reads besides tasks (0.10, Planning sources): an ADR, a spike and a question, each with the
 * statuses the plan CLI gives it, and the category each status is in. Statuses of one kind are not those of another:
 * a spike is `running` where a task is `doing`, and an ADR is `proposed` where a task is `in_review`.
 */
export const ADR = 'adr'
export const SPIKE = 'spike'
export const QUESTION = 'question'

export type KindName = typeof ADR | typeof SPIKE | typeof QUESTION

type Statuses = KindDeclInput['statuses']

const ADR_STATUSES: Statuses = [
  { name: 'proposed', category: 'review' },
  { name: 'accepted', category: 'done' },
  { name: 'rejected', category: 'dropped' },
  { name: 'deprecated', category: 'dropped' },
  { name: 'superseded', category: 'dropped' },
]

const SPIKE_STATUSES: Statuses = [
  { name: 'open', category: 'ready' },
  { name: 'running', category: 'active' },
  { name: 'concluded', category: 'done' },
  { name: 'abandoned', category: 'dropped' },
  { name: 'superseded', category: 'dropped' },
]

const QUESTION_STATUSES: Statuses = [
  { name: 'open', category: 'ready' },
  { name: 'resolved', category: 'done' },
  { name: 'dropped', category: 'dropped' },
]

/**
 * One kind's declaration. It has no transitions, ops or roles of its own, so each is empty: a kind with no ops has the
 * source's (`setStatus` and `create`), which is why a write on it is refused by the provider and not by an argument
 * schema.
 */
const declare = (name: string, statuses: Statuses): KindDeclInput => ({
  name,
  statuses,
  transitions: [],
  ops: [],
  roles: [],
})

/** What `initialize` declares as `kinds`. A task is not here: it is what the source's own statuses are for. */
export const KINDS: KindDeclInput[] = [
  declare(ADR, ADR_STATUSES),
  declare(SPIKE, SPIKE_STATUSES),
  declare(QUESTION, QUESTION_STATUSES),
]

const STATUSES_OF: Record<KindName, Statuses> = {
  adr: ADR_STATUSES,
  spike: SPIKE_STATUSES,
  question: QUESTION_STATUSES,
}

export const EVIDENCE = 'evidence'
export const SUPERSEDES = 'supersedes'
export const BLOCKS = 'blocks'

/**
 * The link types of the three kinds. None of them blocks: `evidence` is an ADR's link to the spikes that support it,
 * `supersedes` an ADR's link to the ADR it replaces (or a spike's to a spike), and `blocks` a question's link to the
 * obligations that wait for its answer. An obligation is not an item here, so that link has nothing at its far end.
 * `inverse` is what the other end calls the link, which Collide shows from there.
 */
export const KIND_LINK_TYPES: LinkTypeDeclInput[] = [
  { name: EVIDENCE, dir: 'out', blocking: false, inverse: 'evidence for' },
  { name: SUPERSEDES, dir: 'out', blocking: false, inverse: 'superseded by' },
  { name: BLOCKS, dir: 'out', blocking: false, inverse: 'blocked by' },
]

/** An ADR, a spike or a question as one file read it. */
export interface KindSource {
  kind: KindName
  file: NameIds
  /** The file's path from the checkout, with `/` separators. */
  path: string
  /** The checkout's absolute path. */
  checkout: string
  frontmatter: Record<string, unknown>
  version: string
  /** The two digits of the proposal whose folder holds a question. */
  proposal?: string
}

export type KindResult = { ok: true; task: TaskInput } | { ok: false; reason: string }

const show = (value: unknown): string => JSON.stringify(value) ?? String(value)

const fail = (reason: string): KindResult => ({ ok: false, reason })

type Refs = { ok: true; value: string[] } | { ok: false; reason: string }

/**
 * A frontmatter list of ids as short ids, in order and once each, or the reason it cannot be read. `ref` gives the
 * short id an entry names, or null when the entry is not shaped like one; `what` says which kind of id the field holds.
 */
function refs(
  frontmatter: Record<string, unknown>,
  field: string,
  what: string,
  ref: (text: string) => string | null,
): Refs {
  const value = frontmatter[field]
  if (value === undefined || value === null) return { ok: true, value: [] }
  if (!Array.isArray(value)) return { ok: false, reason: `${field} must be a list` }
  const shortIds: string[] = []
  for (const entry of value) {
    if (typeof entry !== 'string') return { ok: false, reason: `${field} entry ${show(entry)} is not a string` }
    const shortId = ref(entry)
    if (shortId === null) return { ok: false, reason: `${field} entry "${entry}" is not ${what}` }
    if (!shortIds.includes(shortId)) shortIds.push(shortId)
  }
  return { ok: true, value: shortIds }
}

/**
 * One ADR, spike or question file as an item. Anything the provider cannot place is a reason, so the file goes to
 * `invalid[]`: a status the kind does not have cannot be given a category, and an id that is not shaped like an id
 * cannot be linked to. An id that is well formed and names nothing is a link to nothing, and not a reason.
 * `spikes` is the short ids of the spike folders of the checkout, which tell a hyphen of a spike's suffix from one of
 * its slug.
 */
export function toKindTask(source: KindSource, spikes: ReadonlySet<string>): KindResult {
  const { kind, file, frontmatter } = source
  const { title, status } = frontmatter
  if (typeof title !== 'string' || title.trim() === '') {
    return fail(`title must be a non-empty string, got ${show(title)}`)
  }
  const statuses = STATUSES_OF[kind]
  const names = statuses.map((candidate) => candidate.name).join(', ')
  if (status === undefined) return fail(`status is missing; it must be one of ${names}`)
  const declared = statuses.find((candidate) => candidate.name === status)
  if (declared === undefined) return fail(`status ${show(status)} is not declared; it must be one of ${names}`)

  const links: TaskInput['links'] = []
  const link = (type: string, found: Refs): string | null => {
    if (!found.ok) return found.reason
    links.push(...found.value.map((to) => ({ type, to })))
    return null
  }
  const spike = (text: string): string | null => spikeRef(text, spikes)
  const problems =
    kind === ADR
      ? [
          link(EVIDENCE, refs(frontmatter, 'evidence', 'a spike id', spike)),
          link(SUPERSEDES, refs(frontmatter, 'supersedes', 'an ADR id', adrRef)),
        ]
      : kind === SPIKE
        ? [link(SUPERSEDES, refs(frontmatter, 'supersedes', 'a spike id', spike))]
        : [link(BLOCKS, refs(frontmatter, 'blocks', 'an obligation id', obligationRef))]
  const problem = problems.find((reason) => reason !== null)
  if (problem !== undefined) return fail(problem as string)

  return {
    ok: true,
    task: {
      key: `plan:${file.shortId}`,
      id: file.id,
      kind,
      title,
      state: { status: declared.name, category: declared.category },
      version: source.version,
      checkout: source.checkout,
      path: source.path,
      links,
      fields: source.proposal === undefined ? {} : { proposal: source.proposal },
    },
  }
}
