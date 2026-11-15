// Validation of an agent's tool input, before anything is applied (0.9: an agent's tool inputs are untrusted). The
// input is `unknown`; each parser gives back a typed value or an error that names the problem and says what to do.
// The rules here are the ones the window's live state decides: keys, surfaces, enumerations, sort keys, phase names,
// a clock window, the limits, and that every id is one the window has. The tool definitions that the agent sees, and
// the MCP request framing, are Stage 6 and 6b.
import type { ActionError } from './actions.ts'
import {
  FIELD_SORT_KEYS,
  GROUP_BYS,
  type ClockWindow,
  type Corpus,
  type FieldFilter,
  type FieldSortKey,
  type GroupBy,
  type LensGroup,
  type Sort,
  type SortDir,
  type SurfaceId,
  SURFACES,
  type ToolName,
  TRACKS_SORT_KEYS,
  type TracksFilter,
  type TracksSortKey,
  VIEW_SURFACES,
  type ViewSurface,
} from './state.ts'

export type Parsed<T> = { readonly ok: true; readonly value: T } | { readonly ok: false; readonly error: ActionError }

/** The limits of 0.9: a call names at most 500 ids, a lens has 1 to 12 groups, and the agent's words are cut. */
export const MAX_IDS = 500
export const MIN_GROUPS = 1
export const MAX_GROUPS = 12
export const MAX_TITLE = 80
export const MAX_NAME = 60
export const MAX_REASON = 600
export const MAX_NOTE = 400
const MAX_FILTER_TEXT = 200
const MAX_FILTER_VALUES = 50
const MAX_FIELDS = 20
const MAX_WORD = 40

const CLOCK = /^([01]\d|2[0-3]):[0-5]\d$/

const fail = <T>(code: ActionError['code'], message: string): Parsed<T> => ({ ok: false, error: { code, message } })
const pass = <T>(value: T): Parsed<T> => ({ ok: true, value })

/** The agent's words as text: tabs and line breaks become a space, other control characters go, and it is cut. */
export function cleanText(raw: string, max: number): string {
  const text = raw.replace(/[\t\n\r]/g, ' ').replace(/[\u0000-\u001f\u007f-\u009f]/g, '')
  return Array.from(text.trim()).slice(0, max).join('').trim()
}

/** A value of the agent's, made safe to echo in an error message. */
function shown(raw: unknown): string {
  return typeof raw === 'string' ? cleanText(raw, MAX_WORD) : typeof raw
}

function list(values: readonly string[]): string {
  return values.join(', ')
}

function plainObject(raw: unknown, what: string): Parsed<Record<string, unknown>> {
  if (typeof raw !== 'object' || raw === null || Array.isArray(raw)) {
    return fail('invalid_input', `${what} must be an object`)
  }
  return pass(raw as Record<string, unknown>)
}

function onlyKeys(object: Record<string, unknown>, tool: string, known: readonly string[]): ActionError | null {
  for (const key of Object.keys(object)) {
    if (!known.includes(key)) {
      return { code: 'invalid_input', message: `unknown key "${shown(key)}" for ${tool}; known: ${list(known)}` }
    }
  }
  return null
}

/** The input object of `tool`, with no key outside `known`. */
export function toolObject(raw: unknown, tool: ToolName, known: readonly string[]): Parsed<Record<string, unknown>> {
  const object = plainObject(raw, `the input of ${tool}`)
  if (!object.ok) return object
  const unknown = onlyKeys(object.value, tool, known)
  return unknown === null ? object : { ok: false, error: unknown }
}

export function surfaceOf<S extends SurfaceId>(raw: unknown, tool: ToolName, supported: readonly S[]): Parsed<S> {
  if (typeof raw === 'string' && (supported as readonly string[]).includes(raw)) return pass(raw as S)
  return fail('invalid_input', `unknown surface "${shown(raw)}" for ${tool}; supported: ${list(supported)}`)
}

export const viewSurfaceOf = (raw: unknown, tool: ToolName) => surfaceOf<ViewSurface>(raw, tool, VIEW_SURFACES)
export const anySurfaceOf = (raw: unknown, tool: ToolName) => surfaceOf<SurfaceId>(raw, tool, SURFACES)

/** Which ids a surface's items have: tasks on the Field, runs on Tracks. */
export function idKindOf(surface: ViewSurface): 'task' | 'run' {
  return surface === 'field' ? 'task' : 'run'
}

function knownIds(corpus: Corpus, kind: 'task' | 'run'): ReadonlySet<string> {
  return new Set(kind === 'task' ? corpus.tasks : corpus.runs)
}

/** Rejects every id the window does not have, never repairing one to the nearest (0.9). */
function checkKnown(ids: readonly string[], kind: 'task' | 'run', corpus: Corpus): ActionError | null {
  const known = knownIds(corpus, kind)
  const unknown = [...new Set(ids.filter((id) => !known.has(id)))]
  if (unknown.length === 0) return null
  const named = unknown.slice(0, 10).map(shown)
  const noun = unknown.length === 1 ? `${kind} id` : `${kind} ids`
  return {
    code: 'unknown_id',
    message: `unknown ${noun} ${list(named)}; nothing was shown. Call query for real ids.`,
  }
}

function stringList(raw: unknown, what: string, max: number): Parsed<string[]> {
  if (!Array.isArray(raw)) return fail('invalid_input', `${what} must be an array of strings`)
  if (raw.length > max) return fail('invalid_input', `${what} holds at most ${max} entries`)
  const out: string[] = []
  for (const entry of raw) {
    if (typeof entry !== 'string' || entry.trim() === '') {
      return fail('invalid_input', `${what} must hold only non-empty strings`)
    }
    out.push(entry)
  }
  return pass(out)
}

/** A list of ids of `kind`, at most 500, each one the window has. Repeats are dropped, in order. */
export function idsOf(raw: unknown, what: string, kind: 'task' | 'run', corpus: Corpus): Parsed<string[]> {
  if (Array.isArray(raw) && raw.length > MAX_IDS) {
    return fail('invalid_input', `${what} holds at most ${MAX_IDS} ids`)
  }
  const ids = stringList(raw, what, MAX_IDS)
  if (!ids.ok) return ids
  const unknown = checkKnown(ids.value, kind, corpus)
  return unknown === null ? pass([...new Set(ids.value)]) : { ok: false, error: unknown }
}

/** The filter keys of a surface, in the order a filter is built, so equal filters compare equal as text. */
const FILTER_KEYS = {
  field: ['epic', 'status', 'label', 'source', 'text', 'ids'],
  tracks: ['status', 'task', 'workflow', 'ids'],
} as const

export function filterOf(raw: unknown, surface: 'field', corpus: Corpus): Parsed<FieldFilter>
export function filterOf(raw: unknown, surface: 'tracks', corpus: Corpus): Parsed<TracksFilter>
export function filterOf(raw: unknown, surface: ViewSurface, corpus: Corpus): Parsed<FieldFilter | TracksFilter>
export function filterOf(raw: unknown, surface: ViewSurface, corpus: Corpus): Parsed<FieldFilter | TracksFilter> {
  const object = plainObject(raw, 'filter')
  if (!object.ok) return object
  const known: readonly string[] = FILTER_KEYS[surface]
  for (const key of Object.keys(object.value)) {
    if (!known.includes(key)) {
      return fail('invalid_input', `unknown filter key "${shown(key)}"; known: ${list(known)}`)
    }
  }
  const out: Record<string, string | string[]> = {}
  for (const key of known) {
    const value = object.value[key]
    if (value === undefined) continue
    if (key === 'text') {
      if (typeof value !== 'string') return fail('invalid_input', 'filter text must be a string')
      out[key] = cleanText(value, MAX_FILTER_TEXT)
    } else if (key === 'ids') {
      const ids = idsOf(value, 'filter ids', idKindOf(surface), corpus)
      if (!ids.ok) return ids
      out[key] = ids.value
    } else if (key === 'task') {
      const ids = idsOf(value, 'filter task', 'task', corpus)
      if (!ids.ok) return ids
      out[key] = ids.value
    } else {
      const values = stringList(value, `filter ${key}`, MAX_FILTER_VALUES)
      if (!values.ok) return values
      out[key] = values.value.map((entry) => cleanText(entry, MAX_FILTER_TEXT))
    }
  }
  return pass(out)
}

function dirOf(raw: unknown): Parsed<SortDir> {
  if (raw === 'asc' || raw === 'desc') return pass(raw)
  return fail('invalid_input', `sort dir must be one of: asc, desc`)
}

export function sortOf(raw: unknown, surface: 'field', corpus: Corpus): Parsed<Sort<FieldSortKey>>
export function sortOf(raw: unknown, surface: 'tracks', corpus: Corpus): Parsed<Sort<TracksSortKey>>
export function sortOf(raw: unknown, surface: ViewSurface, corpus: Corpus): Parsed<Sort<string>>
export function sortOf(raw: unknown, surface: ViewSurface, corpus: Corpus): Parsed<Sort<string>> {
  const object = plainObject(raw, 'sort')
  if (!object.ok) return object
  for (const key of Object.keys(object.value)) {
    if (key !== 'by' && key !== 'dir') return fail('invalid_input', `unknown key "${shown(key)}" for sort; known: by, dir`)
  }
  const dir = dirOf(object.value.dir)
  if (!dir.ok) return dir
  const by = object.value.by
  if (typeof by !== 'string') return fail('invalid_input', 'sort by must be a string')
  if (surface === 'field') {
    if ((FIELD_SORT_KEYS as readonly string[]).includes(by)) return pass({ by, dir: dir.value })
    return fail('invalid_input', `unknown sort key "${shown(by)}" for field; supported: ${list(FIELD_SORT_KEYS)}`)
  }
  if ((TRACKS_SORT_KEYS as readonly string[]).includes(by)) return pass({ by, dir: dir.value })
  if (by.startsWith('phase:')) {
    const phase = by.slice('phase:'.length)
    if (corpus.phases.includes(phase)) return pass({ by, dir: dir.value })
    return fail('invalid_input', `unknown sort phase "${shown(phase)}"; known: ${list(corpus.phases)}`)
  }
  const supported = [...TRACKS_SORT_KEYS, 'phase:<name>']
  return fail('invalid_input', `unknown sort key "${shown(by)}" for tracks; supported: ${list(supported)}`)
}

export function groupByOf(raw: unknown): Parsed<GroupBy> {
  if (typeof raw === 'string' && (GROUP_BYS as readonly string[]).includes(raw)) return pass(raw as GroupBy)
  return fail('invalid_input', `unknown group_by "${shown(raw)}"; supported: ${list(GROUP_BYS)}`)
}

export function windowOf(raw: unknown): Parsed<ClockWindow> {
  const bad = () => fail<ClockWindow>('invalid_input', 'window must be {from, to}, two clock times as HH:MM')
  const object = plainObject(raw, 'window')
  if (!object.ok) return bad()
  const { from, to, ...rest } = object.value
  if (Object.keys(rest).length > 0) return bad()
  if (typeof from !== 'string' || typeof to !== 'string' || !CLOCK.test(from) || !CLOCK.test(to)) return bad()
  return pass({ from, to })
}

/** A required piece of the agent's words, cleaned and cut. */
export function wordsOf(raw: unknown, what: string, max: number): Parsed<string> {
  if (typeof raw !== 'string') return fail('invalid_input', `${what} is required and must be a string`)
  const text = cleanText(raw, max)
  return text === '' ? fail('invalid_input', `${what} must not be empty`) : pass(text)
}

/** An optional piece of the agent's words: absent is null, and one that cleans to nothing is null too. */
export function optionalWordsOf(raw: unknown, what: string, max: number): Parsed<string | null> {
  if (raw === undefined) return pass(null)
  if (typeof raw !== 'string') return fail('invalid_input', `${what} must be a string`)
  const text = cleanText(raw, max)
  return pass(text === '' ? null : text)
}

/** A lens's groups: 1 to 12, each with a name, at least one id and a reason, and an id in one group only. */
export function groupsOf(raw: unknown, kind: 'task' | 'run', corpus: Corpus): Parsed<LensGroup[]> {
  if (!Array.isArray(raw) || raw.length < MIN_GROUPS || raw.length > MAX_GROUPS) {
    return fail('invalid_input', `groups must be an array of ${MIN_GROUPS} to ${MAX_GROUPS} groups`)
  }
  const seen = new Set<string>()
  const groups: LensGroup[] = []
  let total = 0
  for (const entry of raw) {
    const object = plainObject(entry, 'a group')
    if (!object.ok) return object
    const unknown = onlyKeys(object.value, 'a group', ['name', 'ids', 'reason'])
    if (unknown !== null) return { ok: false, error: unknown }
    const name = wordsOf(object.value.name, 'a group name', MAX_NAME)
    if (!name.ok) return name
    const reason = wordsOf(object.value.reason, `the reason of group "${name.value}"`, MAX_REASON)
    if (!reason.ok) return reason
    const rawIds = object.value.ids
    if (!Array.isArray(rawIds) || rawIds.length === 0) {
      return fail('invalid_input', `group "${name.value}" needs at least one id`)
    }
    total += rawIds.length
    if (total > MAX_IDS) return fail('invalid_input', `a call names at most ${MAX_IDS} ids`)
    const ids = idsOf(rawIds, `the ids of group "${name.value}"`, kind, corpus)
    if (!ids.ok) return ids
    for (const id of rawIds as string[]) {
      if (seen.has(id)) {
        return fail('duplicate_id', `id ${shown(id)} is in more than one group; nothing was shown. Put each id in one group only.`)
      }
      seen.add(id)
    }
    groups.push({ name: name.value, ids: ids.value, reason: reason.value })
  }
  return pass(groups)
}
