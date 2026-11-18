/** The id rules of the corpus. A task is `T-<proposal>-<number>-<slug>`; its short form drops the slug. */

export const ID_PATTERN = '^T-[0-9]{2}-[0-9]{3}-[a-z0-9]+(-[a-z0-9]+)*$'
export const SHORT_ID_PATTERN = '^T-[0-9]{2}-[0-9]{3}$'
export const LABEL_ID_PATTERN = '^t\\d{2}-\\d{3}$'

/** What `create` takes for the proposal and the epic: the forms the plan CLI's `new task` is given. */
export const PROPOSAL_ARG = '^P[0-9]{2}$'
export const EPIC_ARG = '^E-[0-9]{2}(-[a-z0-9]+)*$'

const TASK_FILE = /^(T-(\d{2})-\d{3})-[a-z0-9]+(?:-[a-z0-9]+)*\.md$/
const EPIC_FILE = /^(E-\d{2})-[a-z0-9]+(?:-[a-z0-9]+)*\.md$/
const PROPOSAL_DIR = /^(\d{2})-[a-z0-9]+(?:-[a-z0-9]+)*$/
const TASK_REF = /^(T-\d{2}-\d{3})(?:-[a-z0-9]+(?:-[a-z0-9]+)*)?$/
const EPIC_REF = /^(E-\d{2})(?:-[a-z0-9]+(?:-[a-z0-9]+)*)?$/

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
