// What the drawer shows of a task, as plain functions over what `task.list` and `task.get` gave (ts-0184-hv; 0.11,
// Task drawer; notes 6, 7, 8 and 18 of the board mock). Nothing here reads the owner or the page, so each rule is
// tested alone.
import { capital, categoryOf, sourceOf, tailOf } from '../field/model.ts'
import type { Category, FieldData, SourceDecl, Task } from '../field/types.ts'
import type { BoardNote, Got, GotLink } from './types.ts'

/** The parts of the drawer, in the order they are drawn. */
export type PartId = 'description' | 'waits' | 'unblocks' | 'part-of' | 'changes' | 'fields' | 'copies' | 'details'

export interface Part {
  readonly id: PartId
  /** The words on its bar. */
  readonly title: string
}

export interface DrawerInput {
  /** The task as the board lists it: the listing has no body, and a collided key has no other. */
  readonly shown: Task
  readonly got: Got
  readonly data: FieldData
  readonly board: BoardNote | null
}

/** The task as `task.get` gave it, or as the board lists it when the daemon names no task for an ambiguous key. */
const taskOf = ({ shown, got }: DrawerInput): Task => got.task ?? shown

const sourceDeclOf = (data: FieldData, task: Task): SourceDecl | undefined =>
  data.sources.find((each) => each.name === sourceOf(task.key))

/** What the source calls a task's state, in words: "being worked on" for the category `active`. */
export const categoryWords = (category: Category): string => categoryOf(category).sub

/** The line under the title: where a change is made. A source in the repo is a file, and any other is its own. */
export function readOnlyLine(task: Task, data: FieldData): string {
  const source = sourceDeclOf(data, task)
  if (source?.scope === 'ref') return 'Read only. Edit the file, or ask an agent; the board follows it.'
  return `Read only. Change it in ${source?.name ?? sourceOf(task.key)}.`
}

export interface BlockingLinks {
  /** The tasks this one waits on. */
  readonly waits: readonly GotLink[]
  /** The tasks that wait on this one. */
  readonly unblocks: readonly GotLink[]
}

/**
 * The links that block, or null for a source with none, which has no Waits on and no Unblocks. A link blocks when its
 * type says so; a source that declares no link types blocks by `blocked-by`, as the Field's cards read it.
 */
export function blockingLinks(got: Got, source: SourceDecl | undefined): BlockingLinks | null {
  const types = got.link_types ?? []
  const declared = types.some((each) => each.blocking)
  if (!declared && source?.blocking_links !== true) return null
  const blocks = (link: GotLink) => {
    const type = types.find((each) => each.name === link.type)
    return type === undefined ? link.type === 'blocked-by' : type.blocking
  }
  const blocking = got.links.filter(blocks)
  return {
    waits: blocking.filter((link) => link.dir === 'out'),
    unblocks: blocking.filter((link) => link.dir === 'in'),
  }
}

/** What a link names: the other task's title, or the end of its key when the board has no such task. */
export const linkWords = (link: GotLink): string => link.title ?? tailOf(link.key)

export interface PartOf {
  /** The epic, from the task's `part-of` link, as the epic's own title or key. */
  readonly epic: string | null
  readonly collections: readonly string[]
  /** The names of the views that hold the task. */
  readonly views: readonly string[]
}

export function partOf(input: DrawerInput): PartOf {
  const link = input.got.links.find((each) => each.type === 'part-of' && each.dir === 'out')
  return {
    epic: link === undefined ? null : linkWords(link),
    collections: taskOf(input).collections,
    views: input.got.views ?? [],
  }
}

export interface FieldRow {
  readonly id: string
  readonly label: string
  /** The value in words, or null for a field with none (a missing value or an empty one). */
  readonly value: string | null
}

/** A field's label: the one its source declares, else its id in words (`story_points` is "Story points"). */
const labelOf = (id: string, declared?: string | null) =>
  declared !== undefined && declared !== null && declared !== '' ? declared : capital(id.replace(/[_-]/g, ' '))

/** A value as the source gave it: text as it is, anything else as JSON. */
function valueWords(value: unknown): string | null {
  if (value === undefined || value === null || value === '') return null
  return typeof value === 'string' ? value : JSON.stringify(value)
}

export function fieldRows(task: Task, source: SourceDecl): FieldRow[] {
  return source.fields.map((field) => ({
    id: field.id,
    label: labelOf(field.id, field.label),
    value: valueWords(task.fields[field.id]),
  }))
}

export type CopyNote = 'shown' | 'differs' | 'same'

export interface CopyRow {
  readonly id: string
  /** Where the copy is: the run whose workspace it is, else its checkout's own name. */
  readonly label: string
  readonly where: string
  readonly status: string
  readonly category: Category
  /** How the copy compares with the one the board shows. Never set for a collided id, which is another task. */
  readonly note: CopyNote | null
  /** For a copy of a collided id that is another task: its title and path. */
  readonly other: { readonly title: string | null; readonly path: string } | null
}

/** The daemon names no task for a collided key: it lists the copies, which are different tasks. */
export const isCollided = (got: Got): boolean => got.task === undefined || got.task === null

const baseName = (path: string) => path.split(/[\\/]/).filter((part) => part !== '').pop() ?? path

/** One row for each copy of the project that has the task: the shown copy, then how each other compares with it. */
export function copyRows({ shown, got, data }: DrawerInput): CopyRow[] {
  const runs = new Map(
    (data.divergence.find((each) => each.key === shown.key)?.copies ?? []).map((copy) => [copy.checkout, copy.run]),
  )
  const collided = isCollided(got)
  const isShownCopy = (copy: Got['copies'][number]) =>
    copy.checkout === shown.checkout && (copy.path ?? '') === (shown.path ?? '')
  const at = got.copies.findIndex(isShownCopy)
  const first = Math.max(at, 0)
  const base = got.copies[first]
  return got.copies.map((copy, index): CopyRow => {
    const run = runs.get(copy.checkout)
    const isShown = index === first
    const other = collided && !isShown
    const same = base !== undefined && copy.state.status === base.state.status && copy.version === base.version
    return {
      id: `${copy.checkout}|${copy.path ?? ''}`,
      label: run === undefined || run === null ? baseName(copy.checkout) : `Run ${run}`,
      where: copy.checkout,
      status: copy.state.status,
      category: copy.state.category,
      note: collided ? (isShown ? 'shown' : null) : noteOf(isShown, same),
      other: other
        ? {
            title: data.tasks.find((each) => each.key === shown.key && each.path === copy.path)?.title ?? null,
            path: copy.path ?? '',
          }
        : null,
    }
  })
}

const noteOf = (isShown: boolean, same: boolean): CopyNote => (isShown ? 'shown' : same ? 'same' : 'differs')

export interface Detail {
  readonly name: string
  readonly value: string
  readonly mono?: boolean
}

export interface Details {
  readonly rows: readonly Detail[]
  /** The task as the source gave it, as JSON. */
  readonly raw: string
}

export function detailsOf(input: DrawerInput): Details {
  const task = taskOf(input)
  const source = sourceDeclOf(input.data, task)
  const rows: Detail[] = [
    { name: 'Source', value: `${source?.name ?? sourceOf(task.key)} · ${source?.pack ?? 'built in'}` },
  ]
  if (task.path !== undefined && task.path !== null) rows.push({ name: 'File', value: task.path, mono: true })
  rows.push({
    name: 'Copies',
    value:
      source?.scope === 'ref'
        ? 'one per checkout: it lives in the repo'
        : 'one for every checkout: it lives outside the repo',
  })
  const { board } = input
  if (board !== null) {
    rows.push({ name: 'Board', value: board.line === null ? board.name : `${board.name} · line ${board.line}` })
  }
  const raw = {
    key: task.key,
    status: task.state.status,
    category: task.state.category,
    title: task.title,
    owners: task.owners,
    labels: task.labels,
    links: task.links,
    fields: task.fields,
  }
  return { rows, raw: JSON.stringify(raw, null, 2) }
}

/** The parts that have something to say about the task, in order. Details for engineers is always there. */
export function partsOf(input: DrawerInput): Part[] {
  const { got, data } = input
  const task = taskOf(input)
  const source = sourceDeclOf(data, task)
  const parts: Part[] = []
  const body = got.task?.body
  if (body !== undefined && body !== null && body.trim() !== '') {
    parts.push({ id: 'description', title: 'Description' })
  }
  // A collided key has no links to read: the daemon names no task for it.
  if (!isCollided(got) && blockingLinks(got, source) !== null) {
    parts.push({ id: 'waits', title: 'Waits on' }, { id: 'unblocks', title: 'Unblocks' })
  }
  const part = partOf(input)
  if (part.epic !== null || part.collections.length > 0 || part.views.length > 0) {
    parts.push({ id: 'part-of', title: 'Part of' })
  }
  // A collided key names no task for `task.changes` to ask about, as it names none for the links above.
  if (!isCollided(got)) parts.push({ id: 'changes', title: 'Changes' })
  if (source !== undefined && source.fields.length > 0) {
    parts.push({ id: 'fields', title: `Fields from ${source.name}` })
  }
  if (source?.scope === 'ref') parts.push({ id: 'copies', title: 'In each copy of the project' })
  parts.push({ id: 'details', title: 'Details for engineers' })
  return parts
}
