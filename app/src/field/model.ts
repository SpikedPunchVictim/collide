// The Field's columns and cards, as plain functions over what `task.list` gave (ts-0181-ft; 0.10, statuses as data;
// 0.11, the Field; notes 3 to 7 of the board mock). Nothing here reads the owner or the page, so each rule is tested
// alone.
import type {
  Category,
  Collision,
  Divergence,
  DivergentCopy,
  FieldData,
  SourceDecl,
  Task,
  ViewReport,
} from './types.ts'

/** What a column is made from: the declared statuses of a source, or the six categories. */
export type GroupMode = 'status' | 'category'

export interface Column {
  /** The status name or the category, which a card's own is compared with. */
  readonly key: string
  readonly label: string
  readonly category: Category
  /** The line under the label: what the category means, or that the status counts as one. */
  readonly sub: string
}

export interface Chip {
  readonly id: 'status' | 'waits' | 'divergence' | 'collision'
  readonly text: string
  /** The category a status chip is coloured by. */
  readonly category?: Category
}

export interface Card {
  /** One for each place a key is at, since a collided key is listed at each of its paths. */
  readonly id: string
  readonly key: string
  readonly title: string
  readonly shortId: string
  readonly stage: string | null
  readonly category: Category
  readonly chips: readonly Chip[]
}

export interface Lane {
  readonly column: Column
  readonly cards: readonly Card[]
}

/** The six categories of 0.10, in their order, with the words the mock gives them (`CATS`). */
export const CATEGORIES: readonly { readonly id: Category; readonly word: string; readonly sub: string }[] = [
  { id: 'backlog', word: 'Backlog', sub: 'not ready to start' },
  { id: 'ready', word: 'Ready', sub: 'can start' },
  { id: 'active', word: 'Active', sub: 'being worked on' },
  { id: 'review', word: 'In review', sub: 'waiting on a review' },
  { id: 'done', word: 'Done', sub: 'finished' },
  { id: 'dropped', word: 'Dropped', sub: 'not doing it' },
]

const categoryOf = (id: Category) => CATEGORIES.find((each) => each.id === id)!

const capital = (text: string) => text.charAt(0).toUpperCase() + text.slice(1)

const sourceOf = (key: string) => (key.includes(':') ? key.slice(0, key.indexOf(':')) : '')

/** The part of a key after its source: `plan:ts-0001-aa` is `ts-0001-aa`. */
const tailOf = (key: string) => (key.includes(':') ? key.slice(key.indexOf(':') + 1) : key)

const isOpen = (task: Task) => task.state.category !== 'done' && task.state.category !== 'dropped'

/**
 * How the shown tasks are grouped: by status when they come from one source, and by category otherwise (0.10, revision
 * 23). A view's `layout.groupBy` of `status` or `category` decides instead.
 */
export function groupMode(tasks: readonly Task[], groupBy?: string | null): GroupMode {
  if (groupBy === 'status' || groupBy === 'category') return groupBy
  return new Set(tasks.map((task) => sourceOf(task.key))).size <= 1 ? 'status' : 'category'
}

/**
 * The columns. By category they are the six, whatever is shown. By status they are the declared statuses of the
 * sources shown, in the order declared, then any status a task has that its source did not declare. With nothing shown
 * they are the first source's.
 */
export function columnsOf(mode: GroupMode, tasks: readonly Task[], sources: readonly SourceDecl[]): Column[] {
  if (mode === 'category') {
    return CATEGORIES.map((each) => ({ key: each.id, label: each.word, category: each.id, sub: each.sub }))
  }
  const shown = new Set(tasks.map((task) => sourceOf(task.key)))
  const declared = sources.filter((source) => shown.has(source.name))
  const used = declared.length > 0 ? declared : sources.slice(0, 1)
  const columns: Column[] = []
  for (const status of used.flatMap((source) => source.statuses)) {
    if (columns.some((column) => column.key === status.name)) continue
    const meaning = categoryOf(status.category)
    columns.push({
      key: status.name,
      label: capital(status.name),
      category: status.category,
      sub: status.name === status.category ? meaning.sub : `counts as ${meaning.word.toLowerCase()}`,
    })
  }
  if (used.length === 0) return columns
  for (const task of tasks) {
    const { status, category } = task.state
    if (columns.some((column) => column.key === status)) continue
    columns.push({
      key: status,
      label: capital(status),
      category,
      sub: `counts as ${categoryOf(category).word.toLowerCase()}`,
    })
  }
  return columns
}

/**
 * A task's short id: what its source's `short_id_pattern` finds in the part of the key after the source, and that part
 * whole when the pattern finds nothing or is not a pattern. `plan:ts-0181-ft` is `0181-ft`; `eight:12` is `12`.
 */
export function shortIdOf(task: Task, sources: readonly SourceDecl[]): string {
  const tail = tailOf(task.key)
  const pattern = sources.find((source) => source.name === sourceOf(task.key))?.short_id_pattern
  if (pattern === undefined || pattern === '') return tail
  try {
    return new RegExp(pattern.replace(/^\^/, '').replace(/\$$/, '')).exec(tail)?.[0] ?? tail
  } catch {
    return tail
  }
}

/** A task's stage: the epic it is part of, as its source names it, and nothing when it is part of none. */
export function stageOf(task: Task): string | null {
  return task.links.find((link) => link.type === 'part-of')?.to ?? null
}

/** Where a copy is: the run whose workspace it is, else the name of its checkout. */
function whereOf(copy: DivergentCopy): string {
  if (copy.run !== undefined && copy.run !== null) return `Run ${copy.run}`
  return copy.checkout.split(/[\\/]/).filter((part) => part !== '').pop() ?? copy.checkout
}

/** The state of a copy and where it is: "Review in Run r-121" (note 6). */
export function copyWords(copy: DivergentCopy): string {
  return `${capital(copy.status)} in ${whereOf(copy)}`
}

/** The chip of a key whose copies differ: the one copy named, or how many differ. */
export function divergenceWords(entry: Divergence): string {
  const [only] = entry.copies
  if (entry.copies.length === 1 && only !== undefined) return copyWords(only)
  return `Differs in ${entry.copies.length} copies`
}

/** The chip of an id that two tasks share (note 7). */
export function collisionWords(collision: Collision): string {
  return `Same id in ${collision.locations.length} copies`
}

/** The tasks of the board by key, and by `source:alias`, which is how a link names a task by another of its names. */
function indexOf(corpus: readonly Task[]): Map<string, Task> {
  const index = new Map<string, Task>()
  for (const task of corpus) {
    if (!index.has(task.key)) index.set(task.key, task)
  }
  for (const task of corpus) {
    for (const alias of task.aliases) {
      const key = `${sourceOf(task.key)}:${alias}`
      if (!index.has(key)) index.set(key, task)
    }
  }
  return index
}

/**
 * How many tasks this one waits on that are still open. A link counts when it says it blocks, or says nothing and is a
 * `blocked-by` of a source that has blocking links: the wire has the link types' own flags only on `task.get`.
 */
function openBlockers(task: Task, sources: readonly SourceDecl[], index: Map<string, Task>): number {
  const source = sources.find((each) => each.name === sourceOf(task.key))
  const blocking = (type: string, flag: boolean | null | undefined) =>
    flag ?? (type === 'blocked-by' && (source?.blocking_links ?? true))
  const waiting = new Set<string>()
  for (const link of task.links) {
    if (!blocking(link.type, link.blocking)) continue
    const target = index.get(link.to.includes(':') ? link.to : `${sourceOf(task.key)}:${link.to}`)
    if (target !== undefined && isOpen(target)) waiting.add(target.key)
  }
  return waiting.size
}

/** The columns with their cards: each task in the column of its status, or of its category. */
export function lanesOf(data: FieldData, groupBy?: string | null): { mode: GroupMode; lanes: Lane[] } {
  const mode = groupMode(data.tasks, groupBy)
  const index = indexOf(data.corpus.length > 0 ? data.corpus : data.tasks)
  const cards = data.tasks.map((task): [string, Card] => {
    const chips: Chip[] = []
    const { status, category } = task.state
    if (mode === 'category') chips.push({ id: 'status', text: status, category })
    const waits = openBlockers(task, data.sources, index)
    if (waits > 0) chips.push({ id: 'waits', text: `Waits on ${waits}` })
    const shortId = shortIdOf(task, data.sources)
    const collided = data.collisions.find(
      (each) =>
        tailOf(task.key) === each.shortId &&
        (task.path == null || each.locations.some((place) => place.path === task.path)),
    )
    const differs = data.divergence.find((each) => each.key === task.key && each.copies.length > 0)
    // A collided id is never shown as a difference (note 7).
    if (differs !== undefined && collided === undefined) {
      chips.push({ id: 'divergence', text: divergenceWords(differs) })
    }
    if (collided !== undefined) chips.push({ id: 'collision', text: collisionWords(collided) })
    const card: Card = {
      id: `${task.key}|${task.path ?? ''}`,
      key: task.key,
      title: task.title,
      shortId,
      stage: stageOf(task),
      category,
      chips,
    }
    return [mode === 'category' ? category : status, card]
  })
  const lanes = columnsOf(mode, data.tasks, data.sources).map((column) => ({
    column,
    cards: cards.filter(([key]) => key === column.key).map(([, card]) => card),
  }))
  return { mode, lanes }
}

const plural = (count: number, one: string, many: string) => (count === 1 ? `1 ${one}` : `${count} ${many}`)

/**
 * What a view says of itself, as short lines: nothing for a clean one. The mock has no words for this, so they follow
 * what `collide tasks --view` prints on standard error, in a person's terms.
 */
export function reportLines(report: ViewReport | null): string[] {
  if (report === null) return []
  if (report.error !== undefined && report.error !== null) return [`This view has an error: ${report.error}`]
  const lines: string[] = []
  if (report.cycle.length > 0) {
    const so = report.name === 'ready' ? ', so none of them is ready' : ''
    lines.push(`Tasks that wait on each other${so}: ${report.cycle.join(', ')}`)
  }
  const unknown = report.unknown_blockers ?? []
  for (const each of unknown.slice(0, 3)) {
    lines.push(`${each.from} waits on ${each.to}, which is not a task, so it is not ready`)
  }
  if (unknown.length > 3) lines.push(`${unknown.length - 3} more tasks wait on something that is not a task`)
  if (report.unmatched_include.length > 0) {
    lines.push(`No task matches what the view includes: ${report.unmatched_include.join(', ')}`)
  }
  if (report.goal_unmatched !== undefined && report.goal_unmatched !== null) {
    lines.push(`The goal of the view, ${report.goal_unmatched}, matches no task`)
  }
  for (const [reason, count] of Object.entries(report.errors)) {
    lines.push(`${plural(count, 'task', 'tasks')} could not be checked: ${reason}`)
  }
  if (report.over_task_budget > 0) {
    const [count, it] =
      report.over_task_budget === 1 ? ['1 task ran out', 'it'] : [`${report.over_task_budget} tasks ran out`, 'them']
    lines.push(`${count} of steps while the view checked ${it}`)
  }
  if (report.pass_budget_exhausted) {
    const left = report.not_reached > 0 ? `: ${report.not_reached} not reached` : ''
    lines.push(`The view stopped before it had checked every task${left}`)
  }
  return lines
}
