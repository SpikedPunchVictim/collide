// The state of the window's UI: one value, changed only by the reducer (0.15, Pass 1: one reducer for UI state). It is
// plain data with no React type, so a reducer test needs no browser. Ids are plain strings: the window's own state
// mirrors no protocol type, because it holds what the screen shows, not what the daemon stores.

/** The four surfaces the in-app agent may open (0.11). Queue and Experiments have no view of their own yet. */
export const SURFACES = ['field', 'tracks', 'queue', 'experiments'] as const
export type SurfaceId = (typeof SURFACES)[number]

/** The two surfaces that have a view the agent may set, a lens and highlights (0.11, `set_view`). */
export const VIEW_SURFACES = ['field', 'tracks'] as const
export type ViewSurface = (typeof VIEW_SURFACES)[number]

export type SortDir = 'asc' | 'desc'

/** Task filters are epic, status, label, source, text and ids (0.11). Each part present narrows; `{}` shows all. */
export interface FieldFilter {
  readonly epic?: readonly string[]
  readonly status?: readonly string[]
  readonly label?: readonly string[]
  readonly source?: readonly string[]
  readonly text?: string
  readonly ids?: readonly string[]
}

/** Run filters are status, task, workflow and ids (0.11). */
export interface TracksFilter {
  readonly status?: readonly string[]
  readonly task?: readonly string[]
  readonly workflow?: readonly string[]
  readonly ids?: readonly string[]
}

export const FIELD_SORT_KEYS = ['id', 'title', 'epic', 'labels', 'status'] as const
export type FieldSortKey = (typeof FIELD_SORT_KEYS)[number]

/** Tracks sorts are total, start, end, spend, wait, and `phase:<name>`, the seconds a run spent in that phase. */
export const TRACKS_SORT_KEYS = ['total', 'start', 'end', 'spend', 'wait'] as const
export type TracksSortKey = (typeof TRACKS_SORT_KEYS)[number] | `phase:${string}`

export interface Sort<Key extends string> {
  readonly by: Key
  readonly dir: SortDir
}

export const GROUP_BYS = ['status', 'epic', 'label', 'source'] as const
export type GroupBy = (typeof GROUP_BYS)[number]

/** A clock window on Tracks: two times such as `09:15` (0.11). */
export interface ClockWindow {
  readonly from: string
  readonly to: string
}

/** A view is typed state and stays live over the corpus (0.11, a view and a lens are two things). */
export interface FieldView {
  readonly filter: FieldFilter
  readonly sort: Sort<FieldSortKey> | null
  readonly groupBy: GroupBy | null
}

export interface TracksView {
  readonly filter: TracksFilter
  readonly sort: Sort<TracksSortKey> | null
  readonly window: ClockWindow | null
}

/** Who made a change: the agent and model that made it, or a client outside the window (0.11, Chips). */
export type ActionSource =
  | { readonly kind: 'agent'; readonly agent: string; readonly model: string }
  | { readonly kind: 'external' }

export interface LensGroup {
  readonly name: string
  readonly ids: readonly string[]
  readonly reason: string
}

/**
 * A lens is a judgement at a moment, not an expression, so it does not follow the corpus (0.11). The ids left out of
 * every group show as "not grouped", dimmed or hidden by `rest`. The strings are the agent's words, cleaned.
 */
export interface Lens {
  readonly title: string
  readonly groups: readonly LensGroup[]
  readonly rest: 'dim' | 'hide'
  readonly source: ActionSource
  readonly at: string
}

/** Marks ids and filters nothing (0.11, `highlight`). */
export interface Highlight {
  readonly ids: readonly string[]
  readonly note: string | null
  readonly title: string | null
  readonly source: ActionSource
  readonly at: string
}

export type PerViewSurface<T> = { readonly [S in ViewSurface]: T }

/**
 * Everything an agent tool call may change, and so everything a snapshot holds. A snapshot is a reference to the
 * value that was replaced, which is safe because the reducer never mutates one.
 */
export interface ScreenState {
  readonly surface: SurfaceId
  /** The item open on each surface, from `open` with an id. */
  readonly selection: { readonly [S in SurfaceId]: string | null }
  readonly field: FieldView
  readonly tracks: TracksView
  readonly lens: PerViewSurface<Lens | null>
  readonly highlight: PerViewSurface<Highlight | null>
}

/**
 * What the window holds of the corpus that the agent's calls are validated against (0.9: ids are checked against the
 * window's live state, and an invented id rejects the whole call). Only ids and names: the rows live elsewhere.
 */
export interface Corpus {
  readonly tasks: readonly string[]
  readonly runs: readonly string[]
  readonly phases: readonly string[]
}

export type ToolName = 'query' | 'set_view' | 'show_groups' | 'highlight' | 'open'

/**
 * The chip a screen-changing call leaves (0.11, Chips). `statement` is Collide's own words, built from the validated
 * action. `title` and `reason` are the agent's, shown as plain text and marked as the agent's.
 */
export interface Chip {
  readonly id: string
  readonly tool: ToolName
  readonly source: ActionSource
  readonly at: string
  readonly statement: string
  readonly title: string | null
  readonly reason: string | null
  readonly undone: boolean
}

/** The state a change replaced, taken at that call, for Undo (0.11). One deep: Undo undoes the last change only. */
export interface UndoSnapshot {
  readonly chip: string
  readonly before: ScreenState
}

export interface UiState {
  readonly screen: ScreenState
  readonly corpus: Corpus
  /** The tasks a person selected, to Launch (0.11). Not part of a snapshot. */
  readonly selectedTasks: readonly string[]
  readonly chips: readonly Chip[]
  readonly undo: UndoSnapshot | null
  readonly nextChip: number
}

export function initialState(): UiState {
  return {
    screen: {
      surface: 'field',
      selection: { field: null, tracks: null, queue: null, experiments: null },
      field: { filter: {}, sort: null, groupBy: null },
      tracks: { filter: {}, sort: null, window: null },
      lens: { field: null, tracks: null },
      highlight: { field: null, tracks: null },
    },
    corpus: { tasks: [], runs: [], phases: [] },
    selectedTasks: [],
    chips: [],
    undo: null,
    nextChip: 1,
  }
}
