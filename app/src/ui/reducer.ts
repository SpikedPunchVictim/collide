// The one reducer for UI state (0.15, Pass 1). `applyAction` is the function: it gives back the next state and says
// whether the action was refused, and `reducer` is the `(state, action) => state` form a React `useReducer` takes.
// Neither imports React, and neither throws on a payload it cannot accept: the state comes back as it went in, with an
// error value (0.9).
//
// Every call that changes the screen takes a snapshot of the screen it replaced, and leaves a chip (0.11, Chips). Undo
// puts that snapshot back and drops it, so it undoes the last change only, and a second Undo finds nothing. A call
// that is valid but changes nothing leaves no chip and no snapshot, so it cannot hide the Undo of the call before it.
// A person's own change to the screen drops the snapshot too, so Undo never discards a person's edit: the chip stops
// offering it (`canUndo`). An action that leaves the screen equal, or does not touch it (selecting a task, loading the
// corpus), keeps the Undo.
import type { ActionError, ActionResult, AgentAction, UiAction, UserAction } from './actions.ts'
import {
  type Chip,
  type Highlight,
  type Lens,
  type ScreenState,
  SURFACES,
  type ToolName,
  type UiState,
  type ViewSurface,
} from './state.ts'
import {
  MAX_NOTE,
  MAX_REASON,
  MAX_TITLE,
  anySurfaceOf,
  filterOf,
  groupByOf,
  groupsOf,
  idKindOf,
  idsOf,
  optionalWordsOf,
  sortOf,
  toolObject,
  viewSurfaceOf,
  windowOf,
  wordsOf,
  cleanText,
  type Parsed,
} from './validate.ts'

const refused = (state: UiState, error: ActionError): ActionResult => ({ ok: false, state, error })
const unchanged = (state: UiState): ActionResult => ({ ok: true, state, changed: false })
const withState = (state: UiState, next: UiState): ActionResult =>
  next === state ? unchanged(state) : { ok: true, state: next, changed: true }

/** Whether a chip may offer Undo: it is the change the snapshot belongs to, and a person's edit has not ended it. */
export function canUndo(state: UiState, chipId: string): boolean {
  return state.undo?.chip === chipId
}

/** The next state, and nothing else: the form a React `useReducer` takes. A refused action returns `state` itself. */
export function reducer(state: UiState, action: UiAction): UiState {
  return applyAction(state, action).state
}

export function applyAction(state: UiState, action: UiAction): ActionResult {
  switch (action.type) {
    case 'user/corpus_loaded':
    case 'user/surface_shown':
    case 'user/task_toggled':
    case 'user/filter_set':
    case 'user/lens_cleared':
    case 'user/undo':
      return applyUser(state, action)
    case 'agent/query':
      return applyQuery(state, action)
    case 'agent/set_view':
      return applySetView(state, action)
    case 'agent/show_groups':
      return applyShowGroups(state, action)
    case 'agent/highlight':
      return applyHighlight(state, action)
    case 'agent/open':
      return applyOpen(state, action)
    default: {
      // A new member of `UiAction` without a branch above makes `action` not `never`, and `tsc --noEmit` fails here.
      const missing: never = action
      const type = (missing as { type?: unknown }).type
      return refused(state, { code: 'unknown_action', message: `unknown action type "${String(type)}"` })
    }
  }
}

// --- Users' actions: typed by the window, so they are trusted in shape. ---

function applyUser(state: UiState, action: UserAction): ActionResult {
  switch (action.type) {
    case 'user/corpus_loaded':
      return withState(state, { ...state, corpus: action.corpus })
    case 'user/surface_shown':
      if (!(SURFACES as readonly string[]).includes(action.surface)) {
        return refused(state, { code: 'invalid_input', message: `unknown surface "${String(action.surface)}"` })
      }
      return withState(state, setScreen(state, { ...state.screen, surface: action.surface }))
    case 'user/task_toggled': {
      if (!state.corpus.tasks.includes(action.id)) {
        return refused(state, { code: 'unknown_id', message: `unknown task id ${action.id}` })
      }
      const selected = state.selectedTasks.includes(action.id)
        ? state.selectedTasks.filter((id) => id !== action.id)
        : [...state.selectedTasks, action.id]
      return withState(state, { ...state, selectedTasks: selected })
    }
    case 'user/filter_set':
      return action.surface === 'field'
        ? withState(state, setScreen(state, { ...state.screen, field: { ...state.screen.field, filter: action.filter } }))
        : withState(state, setScreen(state, { ...state.screen, tracks: { ...state.screen.tracks, filter: action.filter } }))
    case 'user/lens_cleared':
      return withState(
        state,
        state.screen.lens[action.surface] === null
          ? state
          : setScreen(state, { ...state.screen, lens: { ...state.screen.lens, [action.surface]: null } }),
      )
    case 'user/undo':
      return undo(state)
  }
}

/**
 * A person's change to the screen: the state with `screen` in place and the Undo ended, since Undo must never discard
 * a person's edit. When the screen is equal it is the same state, so a no-op is told by identity and keeps the Undo.
 */
function setScreen(state: UiState, screen: ScreenState): UiState {
  return sameScreen(state.screen, screen) ? state : { ...state, screen, undo: null }
}

function sameScreen(a: ScreenState, b: ScreenState): boolean {
  return JSON.stringify(a) === JSON.stringify(b)
}

function undo(state: UiState): ActionResult {
  if (state.undo === null) return unchanged(state)
  const chipId = state.undo.chip
  return {
    ok: true,
    changed: true,
    state: {
      ...state,
      screen: state.undo.before,
      undo: null,
      chips: state.chips.map((chip) => (chip.id === chipId ? { ...chip, undone: true } : chip)),
    },
  }
}

// --- Agent tool calls: untrusted input, validated against the state before anything is applied. ---

/** What a validated, screen-changing call leaves in the chip. */
interface ChipText {
  readonly statement: string
  readonly title: string | null
  readonly reason: string | null
}

/**
 * Applies a screen change: takes the snapshot of the screen it replaced and adds the chip. A change that leaves the
 * screen equal is not one, so the state comes back as it was.
 */
function change(state: UiState, action: AgentAction, tool: ToolName, screen: ScreenState, text: ChipText): ActionResult {
  if (sameScreen(state.screen, screen)) return unchanged(state)
  const chip: Chip = {
    id: `chip-${state.nextChip}`,
    tool,
    source: action.source,
    at: action.at,
    ...text,
    undone: false,
  }
  return {
    ok: true,
    changed: true,
    state: {
      ...state,
      screen,
      chips: [...state.chips, chip],
      undo: { chip: chip.id, before: state.screen },
      nextChip: state.nextChip + 1,
    },
  }
}

const QUERY_SUBJECTS = ['tasks', 'runs'] as const

/** `query` changes no state. Its rows are read from the corpus by the window (Stage 6), so here it is validated only. */
function applyQuery(state: UiState, action: Extract<AgentAction, { type: 'agent/query' }>): ActionResult {
  const input = toolObject(action.input, 'query', ['subject', 'filter', 'fields', 'limit', 'cursor'])
  if (!input.ok) return refused(state, input.error)
  const { subject, filter, fields, limit, cursor } = input.value
  if (subject !== 'tasks' && subject !== 'runs') {
    return refused(state, {
      code: 'invalid_input',
      message: `unknown subject "${cleanText(String(subject), 40)}" for query; supported: ${QUERY_SUBJECTS.join(', ')}`,
    })
  }
  if (filter !== undefined) {
    const parsed = filterOf(filter, subject === 'tasks' ? 'field' : 'tracks', state.corpus)
    if (!parsed.ok) return refused(state, parsed.error)
  }
  if (fields !== undefined && (!Array.isArray(fields) || fields.length > 20 || fields.some((f) => typeof f !== 'string'))) {
    return refused(state, { code: 'invalid_input', message: 'fields must be an array of at most 20 strings' })
  }
  if (limit !== undefined && (!Number.isInteger(limit) || (limit as number) < 1 || (limit as number) > 500)) {
    return refused(state, { code: 'invalid_input', message: 'limit must be a whole number from 1 to 500' })
  }
  if (cursor !== undefined && (typeof cursor !== 'string' || cursor.length > 200)) {
    return refused(state, { code: 'invalid_input', message: 'cursor must be a string of at most 200 characters' })
  }
  return unchanged(state)
}

function applySetView(state: UiState, action: Extract<AgentAction, { type: 'agent/set_view' }>): ActionResult {
  const input = toolObject(action.input, 'set_view', ['surface', 'title', 'reason', 'filter', 'sort', 'group_by', 'window'])
  if (!input.ok) return refused(state, input.error)
  const surface = viewSurfaceOf(input.value.surface, 'set_view')
  if (!surface.ok) return refused(state, surface.error)
  const title = wordsOf(input.value.title, 'title', MAX_TITLE)
  if (!title.ok) return refused(state, title.error)
  const reason = optionalWordsOf(input.value.reason, 'reason', MAX_REASON)
  if (!reason.ok) return refused(state, reason.error)

  const { filter, sort, group_by: groupBy, window } = input.value
  if (filter === undefined && sort === undefined && groupBy === undefined && window === undefined) {
    return refused(state, {
      code: 'invalid_input',
      message: 'set_view sets nothing; give at least one of filter, sort, group_by or window',
    })
  }
  if (groupBy !== undefined && surface.value !== 'field') {
    return refused(state, { code: 'invalid_input', message: 'group_by applies to the field surface only' })
  }
  if (window !== undefined && surface.value !== 'tracks') {
    return refused(state, { code: 'invalid_input', message: 'window applies to the tracks surface only' })
  }

  const parts: string[] = []
  const patch: Record<string, unknown> = {}
  if (filter !== undefined) {
    const parsed = filterOf(filter, surface.value, state.corpus)
    if (!parsed.ok) return refused(state, parsed.error)
    patch.filter = parsed.value
    parts.push(describeFilter(parsed.value))
  }
  if (sort !== undefined) {
    const parsed = sortOf(sort, surface.value, state.corpus)
    if (!parsed.ok) return refused(state, parsed.error)
    patch.sort = parsed.value
    parts.push(`sorted by ${parsed.value.by}, ${parsed.value.dir === 'asc' ? 'ascending' : 'descending'}`)
  }
  if (groupBy !== undefined) {
    const parsed = groupByOf(groupBy)
    if (!parsed.ok) return refused(state, parsed.error)
    patch.groupBy = parsed.value
    parts.push(`grouped by ${parsed.value}`)
  }
  if (window !== undefined) {
    const parsed = windowOf(window)
    if (!parsed.ok) return refused(state, parsed.error)
    patch.window = parsed.value
    parts.push(`window ${parsed.value.from} to ${parsed.value.to}`)
  }

  const view = { ...state.screen[surface.value], ...patch }
  const next: ScreenState = { ...state.screen, [surface.value]: view }
  return change(state, action, 'set_view', next, {
    statement: `set_view: ${surface.value} ${parts.join('; ')}`,
    title: title.value,
    reason: reason.value,
  })
}

function describeFilter(filter: object): string {
  const entries = Object.entries(filter) as Array<[string, string | readonly string[]]>
  if (entries.length === 0) return 'filter cleared'
  return `filter ${entries.map(([key, value]) => `${key}=${typeof value === 'string' ? value : value.join('|')}`).join(', ')}`
}

const counted = (n: number, noun: string) => `${n} ${noun}${n === 1 ? '' : 's'}`
const nounOf = (surface: ViewSurface) => idKindOf(surface)

function applyShowGroups(state: UiState, action: Extract<AgentAction, { type: 'agent/show_groups' }>): ActionResult {
  const input = toolObject(action.input, 'show_groups', ['surface', 'title', 'groups', 'rest'])
  if (!input.ok) return refused(state, input.error)
  const surface = viewSurfaceOf(input.value.surface, 'show_groups')
  if (!surface.ok) return refused(state, surface.error)
  const title = wordsOf(input.value.title, 'title', MAX_TITLE)
  if (!title.ok) return refused(state, title.error)
  const rest = input.value.rest
  if (rest !== undefined && rest !== 'dim' && rest !== 'hide') {
    return refused(state, { code: 'invalid_input', message: 'rest must be one of: dim, hide' })
  }
  const groups = groupsOf(input.value.groups, idKindOf(surface.value), state.corpus)
  if (!groups.ok) return refused(state, groups.error)

  const lens: Lens = { title: title.value, groups: groups.value, rest: rest ?? 'dim', source: action.source, at: action.at }
  const ids = groups.value.reduce((n, group) => n + group.ids.length, 0)
  const next: ScreenState = { ...state.screen, lens: { ...state.screen.lens, [surface.value]: lens } }
  return change(state, action, 'show_groups', next, {
    statement: `show_groups: ${counted(groups.value.length, 'group')}, ${counted(ids, nounOf(surface.value))}, the rest ${lens.rest === 'dim' ? 'dimmed' : 'hidden'}`,
    title: title.value,
    reason: null,
  })
}

function applyHighlight(state: UiState, action: Extract<AgentAction, { type: 'agent/highlight' }>): ActionResult {
  const input = toolObject(action.input, 'highlight', ['surface', 'ids', 'note', 'title'])
  if (!input.ok) return refused(state, input.error)
  const surface = viewSurfaceOf(input.value.surface, 'highlight')
  if (!surface.ok) return refused(state, surface.error)
  const note = optionalWordsOf(input.value.note, 'note', MAX_NOTE)
  if (!note.ok) return refused(state, note.error)
  const title = optionalWordsOf(input.value.title, 'title', MAX_TITLE)
  if (!title.ok) return refused(state, title.error)
  const ids = idsOf(input.value.ids, 'ids', idKindOf(surface.value), state.corpus)
  if (!ids.ok) return refused(state, ids.error)

  const highlight: Highlight = { ids: ids.value, note: note.value, title: title.value, source: action.source, at: action.at }
  const next: ScreenState = { ...state.screen, highlight: { ...state.screen.highlight, [surface.value]: highlight } }
  return change(state, action, 'highlight', next, {
    statement: `highlight: ${counted(ids.value.length, nounOf(surface.value))} on ${surface.value}`,
    title: title.value,
    reason: note.value,
  })
}

const MAX_ITEM_ID = 200

function applyOpen(state: UiState, action: Extract<AgentAction, { type: 'agent/open' }>): ActionResult {
  const input = toolObject(action.input, 'open', ['surface', 'id'])
  if (!input.ok) return refused(state, input.error)
  const surface = anySurfaceOf(input.value.surface, 'open')
  if (!surface.ok) return refused(state, surface.error)
  const id = openedId(input.value.id, surface.value, state)
  if (!id.ok) return refused(state, id.error)

  const selection = id.value === null ? state.screen.selection : { ...state.screen.selection, [surface.value]: id.value }
  const next: ScreenState = { ...state.screen, surface: surface.value, selection }
  return change(state, action, 'open', next, {
    statement: `open: ${surface.value}${id.value === null ? '' : `, ${id.value}`}`,
    title: null,
    reason: null,
  })
}

/** An id to open on a surface: a task on the Field, a run on Tracks, and for the surfaces with no corpus, its shape only. */
function openedId(raw: unknown, surface: ScreenState['surface'], state: UiState): Parsed<string | null> {
  if (raw === undefined) return { ok: true, value: null }
  if (surface === 'field' || surface === 'tracks') {
    const ids = idsOf([raw], 'id', idKindOf(surface), state.corpus)
    return ids.ok ? { ok: true, value: ids.value[0] ?? null } : ids
  }
  if (typeof raw !== 'string' || raw.trim() === '' || raw.length > MAX_ITEM_ID) {
    return { ok: false, error: { code: 'invalid_input', message: `id must be a string of at most ${MAX_ITEM_ID} characters` } }
  }
  return { ok: true, value: raw }
}

