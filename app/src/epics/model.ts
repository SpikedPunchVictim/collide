// The epics strip's rules (ts-0216-ve; Stage 6a, Field, Epics; notes 58 to 61 and 63 of the board mock): which epics
// the strip lists, how they are ordered, what a card counts, and what the lit line says of a pick. Plain functions over
// what `task.list` gave, so each rule is tested with no page. An epic is a collection a source lists (`plan:<epic id>`)
// and each task names its collections; no status name is read here, only a task's category.
import { type Folds, isShut } from '../field/folds.ts'
import { type Lane, cardIdOf, tailOf } from '../field/model.ts'
import type { Category, Collection, FieldData, Task, ViewReport } from '../field/types.ts'
import { stepsOf } from '../field/walk.ts'

const isOpen = (task: Task) => task.state.category !== 'done' && task.state.category !== 'dropped'

/** The tasks of the board, one for each key: the first copy a listing gives, since a collided key is listed twice. */
export function distinct(tasks: readonly Task[]): Task[] {
  const first = new Map<string, Task>()
  for (const task of tasks) if (!first.has(task.key)) first.set(task.key, task)
  return [...first.values()]
}

/** Every task of the board, which a card's counts are over: the shown ones when the board was not read whole. */
export const boardOf = (data: FieldData): readonly Task[] => (data.corpus.length > 0 ? data.corpus : data.tasks)

/** The tasks of the board in a collection, one for each key. */
export function tasksIn(tasks: readonly Task[], collection: string): Task[] {
  return distinct(tasks.filter((task) => task.collections.includes(collection)))
}

/** What a card says of an epic, counted over the whole board and not over the view. */
export interface EpicCard {
  /** The collection's id, as a task names it: `plan:ep-006-wq`. */
  readonly id: string
  readonly title: string
  /** The id without its source: `ep-006-wq`. */
  readonly short: string
  readonly counts: Readonly<Record<Category, number>>
  readonly total: number
  /** Tasks in the category `done`. A dropped task is not one. */
  readonly done: number
  /** Tasks not done and not dropped. */
  readonly open: number
  /** Tasks being worked on or in review. */
  readonly busy: number
  /** Where the source keeps the epic, when it says. */
  readonly path: string | null
}

const NO_COUNTS: Record<Category, number> = { backlog: 0, ready: 0, active: 0, review: 0, done: 0, dropped: 0 }

export function cardOf(collection: Collection, board: readonly Task[]): EpicCard {
  const counts = { ...NO_COUNTS }
  const tasks = tasksIn(board, collection.id)
  for (const task of tasks) counts[task.state.category] += 1
  return {
    id: collection.id,
    title: collection.title,
    short: tailOf(collection.id),
    counts,
    total: tasks.length,
    done: counts.done,
    open: tasks.filter(isOpen).length,
    busy: counts.active + counts.review,
    path: collection.path ?? null,
  }
}

/**
 * Open work first (the mock's `epicRank`): an epic with a task under way or in review, then one with a ready task, then
 * one with only tasks in the backlog, and finished ones last. It reads categories, never a status name.
 */
export function rankOf(card: EpicCard): number {
  if (card.counts.active > 0 || card.counts.review > 0) return 0
  if (card.counts.ready > 0) return 1
  return card.counts.backlog > 0 ? 2 : 3
}

/**
 * The epics the strip lists: each collection with a task in the view before the filter bar's choices, so a filter never
 * hides an epic, in the order of `rankOf` and then by id. The counts are over the whole board.
 */
export function epicsOf(data: FieldData): EpicCard[] {
  const board = boardOf(data)
  const inView = new Set(data.inView.flatMap((task) => task.collections))
  return data.collections
    .filter((collection) => inView.has(collection.id))
    .map((collection) => cardOf(collection, board))
    .sort((a, b) => rankOf(a) - rankOf(b) || a.id.localeCompare(b.id))
}

/** The epics whose name or id has the text in it, in any case. No text keeps them all. */
export function findEpics(cards: readonly EpicCard[], text: string): EpicCard[] {
  const wanted = text.trim().toLowerCase()
  if (wanted === '') return [...cards]
  return cards.filter((card) => `${card.id} ${card.title}`.toLowerCase().includes(wanted))
}

/** The id of each card of the picked epic that the Field draws, one for each place a key is at. */
export function litIds(data: FieldData, collection: string | null): ReadonlySet<string> {
  if (collection === null) return new Set()
  const lit = [...data.tasks, ...data.corpus].filter((task) => task.collections.includes(collection))
  return new Set(lit.map(cardIdOf))
}

/** How the lit cards sit in the board's columns. */
export interface BoardLit {
  /** Lit cards in columns that show. */
  readonly shown: number
  /** Lit cards in columns that are folded. */
  readonly folded: number
  /** The lit cards of each folded column, by its key. */
  readonly byFolded: ReadonlyMap<string, number>
}

export function boardLit(lanes: readonly Lane[], folds: Folds, lit: ReadonlySet<string>): BoardLit {
  let shown = 0
  let folded = 0
  const byFolded = new Map<string, number>()
  for (const lane of lanes) {
    const count = lane.cards.filter((card) => lit.has(card.id)).length
    if (!isShut(lane.column, folds)) shown += count
    else if (count > 0) {
      folded += count
      byFolded.set(lane.column.key, count)
    }
  }
  return { shown, folded, byFolded }
}

/** How many of the epic's tasks the shown board leaves out, because the view or the filter bar does. */
export function outsideView(data: FieldData, collection: string): number {
  const all = tasksIn(boardOf(data), collection).length
  return Math.max(0, all - tasksIn(data.tasks, collection).length)
}

const plural = (count: number, one: string, many: string) => `${count} ${count === 1 ? one : many}`

/** The lit line over the board: how many cards are lit, sit in folded columns, and are outside the view. */
export function boardWords(lit: BoardLit, outside: number): string {
  const parts = [plural(lit.shown, 'card lit', 'cards lit')]
  if (lit.folded > 0) parts.push(`${lit.folded} in folded columns`)
  if (outside > 0) parts.push(`${outside} outside the view`)
  return parts.join(' · ')
}

/** The lit line over the run order and the graph: the epic's open tasks that are drawn, and what they wait on. */
export function walkWords(steps: readonly (readonly Task[])[], collection: string): string {
  const drawn = distinct(steps.flat())
  const own = drawn.filter((task) => task.collections.includes(collection)).length
  if (own === 0) return 'Nothing open in it to draw'
  const others = drawn.length - own
  const lit = plural(own, 'open task lit', 'open tasks lit')
  return others > 0 ? `${lit} · ${others} of other epics they wait on` : lit
}

/** The words of a card's second line: "2 of 5 done", then how many are open and under way, or that it is finished. */
export function progressWords(card: EpicCard): { readonly done: string; readonly open: string } {
  const open =
    card.open === 0 ? 'Finished' : `${card.open} open${card.busy > 0 ? `, ${card.busy} under way` : ''}`
  return { done: `${card.done} of ${card.total} done`, open }
}

/** The categories a card's bar is drawn in, from the finished end to the waiting end. */
export const BAR_ORDER: readonly Category[] = ['done', 'dropped', 'review', 'active', 'ready', 'backlog']

/** What the epic's panel lists: the run order of its open tasks, and the tasks that are finished. */
export interface EpicRun {
  /** The steps of the closure: the epic's open tasks, and the open tasks of other epics that they wait on. */
  readonly steps: readonly (readonly Task[])[]
  /** How many of the epic's own tasks are open. */
  readonly open: number
  /** The epic's tasks that are done or dropped. */
  readonly finished: readonly Task[]
}

export function runOf(collection: string, data: FieldData, report: ViewReport | null): EpicRun {
  const tasks = tasksIn(boardOf(data), collection)
  return {
    steps: stepsOf(report, data),
    open: tasks.filter(isOpen).length,
    finished: tasks.filter((task) => !isOpen(task)),
  }
}

/** The words that say how many tasks of other epics the steps hold, and that they are waited on. */
export function outsideSteps(steps: readonly (readonly Task[])[], collection: string): number {
  return distinct(steps.flat()).filter((task) => !task.collections.includes(collection)).length
}

/** The title of the epic a task is in, as the board lists it, for a task shown among another epic's. */
export function epicTitleOf(task: Task, collections: readonly Collection[]): string {
  const found = collections.find((each) => task.collections.includes(each.id))
  return found?.title ?? 'no epic'
}
