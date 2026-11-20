import type { TaskInput } from '@collide/pack-sdk'
import { BLOCKED_BY, STATUSES } from './declaration.ts'
import { epicRef, labelId, taskRef } from './ids.ts'
import type { TaskFile } from './ids.ts'
import { TASK } from './kinds.ts'

const KNOWN_STATUSES = STATUSES.map((status) => status.name)

export interface TaskSource {
  file: TaskFile
  /** The file's path from the checkout, with `/` separators. */
  path: string
  /** The checkout's absolute path. */
  checkout: string
  frontmatter: Record<string, unknown>
  body: string
  version: string
}

/** A task read from a file, and the epic its frontmatter names, if any. */
export interface ReadTask {
  task: TaskInput
  epic: string | null
}

export type TaskResult = { ok: true; value: ReadTask } | { ok: false; reason: string }

const show = (value: unknown): string => JSON.stringify(value) ?? String(value)

const fail = (reason: string): TaskResult => ({ ok: false, reason })

/** `depends_on` as short ids, in order and once each, or the reason it cannot be read. */
function dependencies(value: unknown): { ok: true; value: string[] } | { ok: false; reason: string } {
  if (value === undefined || value === null) return { ok: true, value: [] }
  if (!Array.isArray(value)) return { ok: false, reason: 'depends_on must be a list' }
  const shortIds: string[] = []
  for (const entry of value) {
    if (typeof entry !== 'string') return { ok: false, reason: `depends_on entry ${show(entry)} is not a string` }
    const shortId = taskRef(entry)
    if (shortId === null) return { ok: false, reason: `depends_on entry "${entry}" is not a task id` }
    if (!shortIds.includes(shortId)) shortIds.push(shortId)
  }
  return { ok: true, value: shortIds }
}

/**
 * One task file as a task. Anything the adapter cannot place is a reason, so the file goes to `invalid[]` and no
 * guess is made: a status it did not declare cannot be given a category.
 * `proposal` is the two digits of the folder the file is in, which the task's id must agree with.
 */
export function toTask(source: TaskSource, proposal: string): TaskResult {
  const { file, frontmatter } = source
  if (file.proposal !== proposal) {
    return fail(`the id names proposal ${file.proposal}, but the file is in the folder of proposal ${proposal}`)
  }

  const { title, status } = frontmatter
  if (typeof title !== 'string' || title.trim() === '') {
    return fail(`title must be a non-empty string, got ${show(title)}`)
  }
  if (status === undefined) return fail(`status is missing; it must be one of ${KNOWN_STATUSES.join(', ')}`)
  const declared = STATUSES.find((candidate) => candidate.name === status)
  if (declared === undefined) {
    return fail(`status ${show(status)} is not declared; it must be one of ${KNOWN_STATUSES.join(', ')}`)
  }

  const epicText = frontmatter.epic
  let epic: string | null = null
  if (epicText !== undefined && epicText !== null) {
    epic = typeof epicText === 'string' ? epicRef(epicText) : null
    if (epic === null) return fail(`epic must be an epic id such as E-07, got ${show(epicText)}`)
  }

  const depends = dependencies(frontmatter.depends_on)
  if (!depends.ok) return fail(depends.reason)

  return {
    ok: true,
    value: {
      epic,
      task: {
        key: `plan:${file.shortId}`,
        id: file.id,
        kind: TASK,
        title,
        state: { status: declared.name, category: declared.category },
        version: source.version,
        checkout: source.checkout,
        path: source.path,
        links: depends.value.map((to) => ({ type: BLOCKED_BY, to })),
        templateFields: { labelId: labelId(file.shortId) },
        fields: epic === null ? { proposal } : { proposal, epic },
      },
    },
  }
}
