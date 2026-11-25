// The filter bar's rules (ts-0190-tz; Stage 6a, Filter bar; 0.11, Filter bar to view): what a person has chosen,
// the CEL that is, the groups the tasks offer and their counts, and the view that Save as a view writes. Plain
// functions over data, so each rule is tested with no page. The bar is a form for the `filter` of `task.list`: the
// board is the view asked for, narrowed by this expression.
import type { ViewInput } from '../owner/generated.ts'
import { CATEGORIES, sourceOf } from '../field/model.ts'
import type { Category, Collection, Task, ViewEntry } from '../field/types.ts'

/** What the person has chosen. A group with nothing in it is no choice; the groups are and-ed, the values or-ed. */
export interface Filters {
  /** Collection ids, as `plan:ep-...`: the Epic box. */
  readonly epics: readonly string[]
  readonly categories: readonly Category[]
  readonly sources: readonly string[]
  readonly labels: readonly string[]
  /** Searched for in the title and the key. */
  readonly text: string
}

export const NO_FILTERS: Filters = { epics: [], categories: [], sources: [], labels: [], text: '' }

/** What joins one term to the next in the expression: a line each, so Advanced reads as the mock's does. */
const AND = '\n  && '

export const isFiltering = (filters: Filters) => celOf(filters) !== null

/** A value in or out of a group, the others keeping their order. */
export function toggle<T>(chosen: readonly T[], value: T): T[] {
  return chosen.includes(value) ? chosen.filter((each) => each !== value) : [...chosen, value]
}

/** A CEL string. JSON's quoting is CEL's: `"` and `\` are escaped, so any text stays one string. */
const quoted = (text: string) => JSON.stringify(text)

/** The text as a pattern that matches only itself. */
const literal = (text: string) => text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')

const anyOf = (terms: readonly string[]) => (terms.length === 1 ? terms[0]! : `(${terms.join(' || ')})`)

/**
 * The expression the choices write, or null for none. `where` is the view's own and goes first, in brackets. The
 * groups come in the order of the bar: Epic, where it stands, where it comes from, labels, then the search.
 */
export function celOf(filters: Filters, where?: string | null): string | null {
  const terms: string[] = []
  if (where !== undefined && where !== null && where.trim() !== '') terms.push(`(${where})`)
  if (filters.epics.length > 0) terms.push(anyOf(filters.epics.map((id) => `${quoted(id)} in collections`)))
  if (filters.categories.length > 0) terms.push(`category in [${filters.categories.map(quoted).join(', ')}]`)
  if (filters.sources.length > 0) terms.push(`source in [${filters.sources.map(quoted).join(', ')}]`)
  if (filters.labels.length > 0) terms.push(anyOf(filters.labels.map((label) => `${quoted(label)} in labels`)))
  const text = filters.text.trim()
  if (text !== '') {
    // The strings extension is not on, so `matches` with the case flag stands for a lower-cased `contains`.
    const pattern = quoted(`(?i)${literal(text)}`)
    terms.push(`(title.matches(${pattern}) || key.matches(${pattern}))`)
  }
  return terms.length === 0 ? null : terms.join(AND)
}

export interface CategoryGroup {
  readonly id: Category
  readonly word: string
  readonly count: number
}

/** What the view's tasks offer to choose between, with how many tasks each choice would keep. */
export interface Groups {
  /** All six, in their order, even those with no task. */
  readonly categories: readonly CategoryGroup[]
  /** The epics the tasks are in. None unless there are two or more to choose between. */
  readonly epics: readonly { readonly id: string; readonly title: string; readonly count: number }[]
  /** None unless the tasks come from more than one source. */
  readonly sources: readonly { readonly name: string; readonly count: number }[]
  /** None unless a task carries a label. */
  readonly labels: readonly { readonly label: string; readonly count: number }[]
}

function tally(keys: readonly (readonly string[])[]): Map<string, number> {
  const counts = new Map<string, number>()
  for (const each of keys) for (const key of new Set(each)) counts.set(key, (counts.get(key) ?? 0) + 1)
  return counts
}

/** The groups of the tasks the view holds before any choice, so a count never shrinks as the bar is used. */
export function groupsOf(tasks: readonly Task[], collections: readonly Collection[]): Groups {
  // A key listed at two places is one task.
  const distinct = [...new Map(tasks.map((task) => [task.key, task])).values()]
  const byCategory = tally(distinct.map((task) => [task.state.category]))
  const byEpic = tally(distinct.map((task) => task.collections))
  const bySource = tally(distinct.map((task) => [sourceOf(task.key)]))
  const byLabel = tally(distinct.map((task) => task.labels))
  const epics = collections
    .filter((each) => byEpic.has(each.id))
    .map((each) => ({ id: each.id, title: each.title, count: byEpic.get(each.id)! }))
  return {
    categories: CATEGORIES.map((each) => ({ id: each.id, word: each.word, count: byCategory.get(each.id) ?? 0 })),
    epics: epics.length >= 2 ? epics : [],
    sources: bySource.size >= 2 ? [...bySource].map(([name, count]) => ({ name, count })) : [],
    labels: [...byLabel].sort(([a], [b]) => a.localeCompare(b)).map(([label, count]) => ({ label, count })),
  }
}

const titleOf = (id: string, collections: readonly Collection[]) =>
  collections.find((each) => each.id === id)?.title ?? id

/** What the Epic box says it holds: Any, the one epic by its title, or how many. */
export function epicWords(filters: Filters, collections: readonly Collection[]): string {
  if (filters.epics.length === 0) return 'Any'
  if (filters.epics.length === 1) return titleOf(filters.epics[0]!, collections)
  return `${filters.epics.length} epics`
}

/** One line for each group in force, for the folded bar. */
export function summaryOf(filters: Filters, collections: readonly Collection[]): string[] {
  const lines: string[] = []
  if (filters.epics.length > 0) lines.push(`Epic: ${epicWords(filters, collections)}`)
  if (filters.categories.length > 0) {
    const words = CATEGORIES.filter((each) => filters.categories.includes(each.id)).map((each) => each.word)
    lines.push(`Where it stands: ${words.join(', ')}`)
  }
  if (filters.sources.length > 0) lines.push(`Comes from: ${filters.sources.join(', ')}`)
  if (filters.labels.length > 0) lines.push(`Labels: ${filters.labels.join(', ')}`)
  if (filters.text.trim() !== '') lines.push(`Search: "${filters.text.trim()}"`)
  return lines
}

/** A view to write, or the words of why none can be. */
export type Save = { readonly view: ViewInput } | { readonly why: string }

/**
 * The words of why a view cannot be saved with the choices in it, or null when it can. A built-in view is a rule no
 * file can say. A view that names tasks or walks to a goal has members its `where` never sees, which the choices
 * would not reach in a file.
 */
export function cannotSave(base: ViewEntry | undefined): string | null {
  if (base?.source.type === 'builtin') {
    return `${base.view.name} is a built-in view, a rule no file can say. Start from All tasks instead.`
  }
  if ((base?.view.include?.length ?? 0) > 0 || (base?.view.closure ?? null) !== null) {
    return 'This view names tasks or walks to a goal, which a filter in a file would not reach.'
  }
  return null
}

/**
 * The view that lists what the board shows: the base view's `where` with the choices and-ed in, and its `exclude` and
 * `layout` as they are.
 */
export function saveOf(base: ViewEntry | undefined, filters: Filters, name: string): Save {
  const title = name.trim()
  if (title === '') return { why: 'Give the view a name.' }
  if (!isFiltering(filters)) return { why: 'Choose something to filter by first.' }
  const why = cannotSave(base)
  if (why !== null) return { why }
  const view = base?.view
  const saved: { -readonly [K in keyof ViewInput]: ViewInput[K] } = { name: title, where: celOf(filters, view?.where)! }
  if ((view?.exclude?.length ?? 0) > 0) saved.exclude = view!.exclude
  if (view?.layout != null) saved.layout = view.layout
  return { view: saved }
}
