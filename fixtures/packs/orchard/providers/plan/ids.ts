/**
 * The id rules of the corpus. A task is `T-<proposal>-<number>-<slug>`; its short form drops the slug. The other kinds
 * the provider reads have ids of the same shape: an ADR is `ADR-<number>-<slug>`, a spike `S<number><suffix>-<slug>`
 * (the suffix is letters, and may have a hyphen: `S4d-dock`) and a question `Q-<proposal>-<number>-<slug>`.
 */

export const ID_PATTERN = '^T-[0-9]{2}-[0-9]{3}-[a-z0-9]+(-[a-z0-9]+)*$'
export const SHORT_ID_PATTERN = '^T-[0-9]{2}-[0-9]{3}$'

const SHORT_FORMS = ['T-[0-9]{2}-[0-9]{3}', 'ADR-[0-9]{3,}', 'S[0-9]+(?:[a-z]+(?:-[a-z]+)*)?', 'Q-[0-9]{2}-[0-9]{2}']
const SLUG = '[a-z0-9]+(?:-[a-z0-9]+)*'

/** What `initialize` declares as the ids of the source: those of every kind, since a task is only one of them. */
export const ANY_ID_PATTERN = `^(?:${SHORT_FORMS.join('|')})-${SLUG}$`
export const ANY_SHORT_ID_PATTERN = `^(?:${SHORT_FORMS.join('|')})$`
export const LABEL_ID_PATTERN = '^t\\d{2}-\\d{3}$'

/** What `create` takes for the proposal and the epic: the forms the plan CLI's `new task` is given. */
export const PROPOSAL_ARG = '^P[0-9]{2}$'
export const EPIC_ARG = '^E-[0-9]{2}(-[a-z0-9]+)*$'

const TASK_FILE = /^(T-(\d{2})-\d{3})-[a-z0-9]+(?:-[a-z0-9]+)*\.md$/
const EPIC_FILE = /^(E-\d{2})-[a-z0-9]+(?:-[a-z0-9]+)*\.md$/
const PROPOSAL_DIR = /^(\d{2})-[a-z0-9]+(?:-[a-z0-9]+)*$/
const TASK_REF = /^(T-\d{2}-\d{3})(?:-[a-z0-9]+(?:-[a-z0-9]+)*)?$/
const EPIC_REF = /^(E-\d{2})(?:-[a-z0-9]+(?:-[a-z0-9]+)*)?$/

const ADR_FILE = /^(\d{3,})-\d{4}-\d{2}-\d{2}-([a-z0-9]+(?:-[a-z0-9]+)*)\.md$/
const SPIKE_DIR = /^s(\d+)((?:[a-z]+(?:-[a-z]+)*)?)-\d{4}-\d{2}-\d{2}-([a-z0-9]+(?:-[a-z0-9]+)*)$/
const QUESTION_FILE = /^(Q-\d{2}-\d{2})-[a-z0-9]+(?:-[a-z0-9]+)*\.md$/
const QUESTION_REF = /^(Q-\d{2}-\d{2})(?:-[a-z0-9]+(?:-[a-z0-9]+)*)?$/
const ADR_REF = /^(ADR-\d{3,})(?:-[a-z0-9]+(?:-[a-z0-9]+)*)?$/
const OBLIGATION_REF = /^(O-\d{2}-\d{2})(?:-[a-z0-9]+(?:-[a-z0-9]+)*)?$/
/** Shaped so that no part of it can match in two ways: a name of any length is read in one pass. */
const SPIKE_REF = /^S\d+(?:[a-z][a-z0-9]*)?(?:-[a-z0-9]+)*$/

/** Matches `T-03-001` anywhere in a table cell. */
export const TASK_ID_IN_TEXT = /\bT-\d{2}-\d{3}\b/g

export interface TaskFile {
  /** The file's name without `.md`: the task's long id. */
  id: string
  shortId: string
  /** The two digits of the proposal that the id names. */
  proposal: string
}

/** The id a task file's name gives, or null when the name is not a task's. */
export function taskFile(name: string): TaskFile | null {
  const match = TASK_FILE.exec(name)
  if (match === null) return null
  return { id: name.slice(0, -'.md'.length), shortId: match[1] as string, proposal: match[2] as string }
}

/** The short id of an epic file (`E-07`), or null when the name is not an epic's. */
export const epicFile = (name: string): string | null => EPIC_FILE.exec(name)?.[1] ?? null

/** The proposal number of a folder under `arch/proposals` (`03`), or null when it is not a proposal's. */
export const proposalDir = (name: string): string | null => PROPOSAL_DIR.exec(name)?.[1] ?? null

/** The short id that a `depends_on` entry names, in short or long form, or null when it is not a task id. */
export const taskRef = (text: string): string | null => TASK_REF.exec(text)?.[1] ?? null

/** The short id of the epic that a task's `epic:` names (`E-07` or `E-07-slug`), or null. */
export const epicRef = (text: string): string | null => EPIC_REF.exec(text)?.[1] ?? null

/** `T-03-001` becomes `t03-001`, the `labelId` template field. */
export const labelId = (shortId: string): string => `t${shortId.slice(2)}`

/** What the name of an ADR, spike or question says: its long id and its short id. */
export interface NameIds {
  id: string
  shortId: string
}

/** The ids that an ADR file's name gives (`arch/004-2026-02-16-sunken-quays.md`), or null when it is not an ADR's. */
export function adrFile(name: string): NameIds | null {
  const match = ADR_FILE.exec(name)
  if (match === null) return null
  return { id: `ADR-${match[1]}-${match[2]}`, shortId: `ADR-${match[1]}` }
}

/** The ids that a spike folder's name gives (`s4d-dock-2026-03-01-crane-limits`), or null when it is not a spike's. */
export function spikeDir(name: string): NameIds | null {
  const match = SPIKE_DIR.exec(name)
  if (match === null) return null
  const shortId = `S${match[1]}${match[2]}`
  return { id: `${shortId}-${match[3]}`, shortId }
}

/** The ids that a question file's name gives (`Q-03-01-which-berth-first.md`), or null when it is not a question's. */
export function questionFile(name: string): NameIds | null {
  const match = QUESTION_FILE.exec(name)
  if (match === null) return null
  return { id: name.slice(0, -'.md'.length), shortId: match[1] as string }
}

/** The short id of the question that a queue entry names, in short or long form, or null when it is not one. */
export const questionRef = (text: string): string | null => QUESTION_REF.exec(text)?.[1] ?? null

/** The short id of the ADR that `supersedes` names (`ADR-001` or `ADR-001-slug`), or null. */
export const adrRef = (text: string): string | null => ADR_REF.exec(text)?.[1] ?? null

/** The short id of the obligation that a question's `blocks` names (`O-03-01` or `O-03-01-slug`), or null. */
export const obligationRef = (text: string): string | null => OBLIGATION_REF.exec(text)?.[1] ?? null

/**
 * The short id of the spike that `text` names, or null when it is not shaped like a spike id. A spike's id may have a
 * hyphen in its suffix and in its slug, so a long id is cut at each hyphen, the longest first, until a spike that
 * exists is named. When none is, the answer is the first part (`S9-never-written` is `S9`): a link to nothing.
 */
export function spikeRef(text: string, known: ReadonlySet<string>): string | null {
  if (!SPIKE_REF.test(text)) return null
  for (let cut = text; ; cut = cut.slice(0, cut.lastIndexOf('-'))) {
    if (known.has(cut)) return cut
    if (!cut.includes('-')) break
  }
  return text.split('-')[0] as string
}
