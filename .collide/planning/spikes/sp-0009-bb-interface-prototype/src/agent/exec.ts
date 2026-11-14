// The UI tools, as pure functions of the app state. The MCP server (dev server side) forwards a call to the page; the
// page runs it here against its live state, dispatches the returned action and sends the result back.
// A tool takes ids, labels and reasons. It never knows why, and none of them launches, accepts, edits or deletes.
import { reduce, type Action } from '../reducer'
import {
  fmtClock, matches, parseClock, phaseSeconds, runMatches, statusOf, totalSeconds, trackRuns, visibleTasks, waitSeconds, WORKFLOWS,
} from '../world'
import type { Filter, GroupBy, LensGroup, Run, RunFilter, RunStatus, SortSpec, State, TaskStatus } from '../types'

export const TOOL_NAMES = ['query', 'set_view', 'show_groups', 'highlight', 'open'] as const
export type ToolName = (typeof TOOL_NAMES)[number]

export const TASK_STATUSES: TaskStatus[] = ['ready', 'blocked', 'backlog', 'running', 'review', 'done']
export const RUN_STATUSES: RunStatus[] = ['working', 'waiting', 'failed', 'paused', 'done', 'stopped']
export const PHASE_KEYS = ['provision', 'build', 'brief', 'review:A', 'review:B', 'adjudicate', 'fix', 'close']
export const FIELD_SORTS = ['id', 'title', 'epic', 'labels', 'status']
export const TRACK_SORTS = ['total', 'start', 'end', 'spend', 'wait', 'phase:<key>']
export const GROUPS: GroupBy[] = ['status', 'epic', 'label', 'source']
export const SETTABLE = ['field', 'tracks']
export const OPENABLE = ['field', 'tracks', 'queue', 'experiments']

export type ToolOutcome = { ok: true; result: unknown; action?: Action } | { ok: false; error: string }

class ToolError extends Error {}
const fail = (msg: string): never => {
  throw new ToolError(msg)
}

const isObj = (x: unknown): x is Record<string, unknown> => typeof x === 'object' && x !== null && !Array.isArray(x)
const list = (xs: readonly string[]) => xs.join(', ')

function strings(x: unknown, path: string): string[] {
  if (!Array.isArray(x) || x.some((v) => typeof v !== 'string')) return fail(`${path} must be a list of strings`)
  return x as string[]
}
function oneOf(vals: string[], allowed: readonly string[], path: string): string[] {
  const bad = vals.filter((v) => !allowed.includes(v))
  if (bad.length) fail(`unknown ${path} ${bad.map((b) => `"${b}"`).join(', ')}; known: ${list(allowed)}`)
  return vals
}
function noExtra(o: Record<string, unknown>, allowed: string[], path: string): void {
  const bad = Object.keys(o).filter((k) => !allowed.includes(k))
  if (bad.length) fail(`${path}: unknown key ${bad.map((b) => `"${b}"`).join(', ')}; allowed: ${list(allowed)}`)
}

function taskFilter(s: State, raw: unknown): Filter {
  const f: Filter = { epic: [], status: [], label: [], source: [], text: '', ids: [] }
  if (raw === undefined) return f
  if (!isObj(raw)) return fail('filter must be an object')
  noExtra(raw, ['epic', 'status', 'label', 'source', 'text', 'ids'], 'filter (tasks)')
  if (raw.epic !== undefined) f.epic = oneOf(strings(raw.epic, 'filter.epic'), s.epics.map((e) => e.id), 'epic')
  if (raw.status !== undefined) f.status = oneOf(strings(raw.status, 'filter.status'), TASK_STATUSES, 'task status') as TaskStatus[]
  if (raw.label !== undefined) f.label = oneOf(strings(raw.label, 'filter.label'), [...new Set(s.tasks.flatMap((t) => t.labels))].sort(), 'label')
  if (raw.source !== undefined) f.source = oneOf(strings(raw.source, 'filter.source'), ['tasks/', 'github'], 'source')
  if (raw.text !== undefined) {
    if (typeof raw.text !== 'string') fail('filter.text must be a string')
    f.text = raw.text as string
  }
  if (raw.ids !== undefined) f.ids = oneOf(strings(raw.ids, 'filter.ids'), s.tasks.map((t) => t.id), 'task id')
  return f
}

function runFilterOf(s: State, raw: unknown): RunFilter {
  const f: RunFilter = { status: [], task: [], workflow: [], ids: [] }
  if (raw === undefined) return f
  if (!isObj(raw)) return fail('filter must be an object')
  noExtra(raw, ['status', 'task', 'workflow', 'ids'], 'filter (runs)')
  if (raw.status !== undefined) f.status = oneOf(strings(raw.status, 'filter.status'), RUN_STATUSES, 'run status') as RunStatus[]
  if (raw.task !== undefined) f.task = oneOf(strings(raw.task, 'filter.task'), s.tasks.map((t) => t.id), 'task id')
  if (raw.workflow !== undefined) f.workflow = oneOf(strings(raw.workflow, 'filter.workflow'), WORKFLOWS, 'workflow')
  if (raw.ids !== undefined) f.ids = oneOf(strings(raw.ids, 'filter.ids'), s.runs.map((r) => r.id), 'run id')
  return f
}

function sortOf(s: State, raw: unknown, surface: string): SortSpec {
  if (!isObj(raw)) return fail('sort must be an object {by, dir?}')
  noExtra(raw, ['by', 'dir'], 'sort')
  if (typeof raw.by !== 'string') return fail('sort.by must be a string')
  const by = raw.by
  const dir = raw.dir ?? 'asc'
  if (dir !== 'asc' && dir !== 'desc') return fail(`sort.dir must be "asc" or "desc", got "${String(dir)}"`)
  if (surface === 'field') {
    if (!FIELD_SORTS.includes(by)) fail(`unknown sort "${by}" for field; known: ${list(FIELD_SORTS)}`)
  } else if (by.startsWith('phase:')) {
    const key = by.slice(6)
    const known = [...new Set([...PHASE_KEYS, ...s.runs.flatMap((r) => r.phases.map((p) => p.key))])]
    if (!known.includes(key)) fail(`unknown phase "${key}" in sort; known: ${list(known)}`)
  } else if (!TRACK_SORTS.includes(by)) fail(`unknown sort "${by}" for tracks; known: ${list(TRACK_SORTS)}`)
  return { by, dir }
}

function title(raw: Record<string, unknown>): string {
  if (typeof raw.title !== 'string' || !raw.title.trim()) return fail('title is required: a short label (under 60 characters) for the chip the user sees')
  return raw.title.trim().slice(0, 80)
}

function surfaceOf(raw: Record<string, unknown>, allowed: string[], tool: string): string {
  if (typeof raw.surface !== 'string') return fail(`surface is required; ${tool} supports: ${list(allowed)}`)
  if (!allowed.includes(raw.surface)) fail(`unknown surface "${raw.surface}" for ${tool}; supported: ${list(allowed)}`)
  return raw.surface
}

export function taskRow(s: State, t: State['tasks'][number]) {
  return { id: t.id, title: t.title, body: t.body, epic: t.epic, status: statusOf(s, t), labels: t.labels, source: t.source, depends_on: t.dependsOn }
}

export function runRow(s: State, r: Run) {
  return {
    id: r.id, task: r.taskId, title: r.title, status: r.status, workflow: r.workflow, line: r.line, started: fmtClock(r.startWall),
    total_seconds: Math.round(totalSeconds(s, r)), wait_seconds: Math.round(waitSeconds(s, r)), spend_usd: Number(r.spend.toFixed(2)),
    review: r.review,
    phases: r.phases.map((p) => ({ key: p.key, seconds: Math.round(phaseSeconds(s, r, p.key) ?? 0) })),
  }
}

function query(s: State, a: Record<string, unknown>): ToolOutcome {
  if (a.subject !== 'tasks' && a.subject !== 'runs') return fail(`unknown subject "${String(a.subject)}"; known: tasks, runs`)
  const max = a.limit === undefined ? 200 : Number(a.limit)
  if (!Number.isInteger(max) || max < 1 || max > 500) return fail('limit must be an integer from 1 to 500')
  if (a.subject === 'tasks') {
    const f = taskFilter(s, a.filter)
    const all = s.tasks.filter((t) => matches(s, t, f))
    return { ok: true, result: { subject: 'tasks', count: all.length, truncated: all.length > max, rows: all.slice(0, max).map((t) => taskRow(s, t)) } }
  }
  const f = runFilterOf(s, a.filter)
  const all = s.runs.filter((r) => runMatches(r, f))
  return { ok: true, result: { subject: 'runs', count: all.length, truncated: all.length > max, rows: all.slice(0, max).map((r) => runRow(s, r)) } }
}

function setView(s: State, a: Record<string, unknown>, source: string): ToolOutcome {
  const surface = surfaceOf(a, SETTABLE, 'set_view') as 'field' | 'tracks'
  const ttl = title(a)
  if (a.filter === undefined && a.sort === undefined && a.group_by === undefined && a.window === undefined) {
    fail('set_view needs at least one of filter, sort, group_by, window')
  }
  if (a.reason !== undefined && typeof a.reason !== 'string') fail('reason must be a string')
  const base = { title: ttl, reason: a.reason as string | undefined, source }
  let action: Action
  if (surface === 'field') {
    if (a.window !== undefined) fail('window applies to tracks only')
    let groupBy: GroupBy | undefined
    if (a.group_by !== undefined) {
      if (typeof a.group_by !== 'string' || !GROUPS.includes(a.group_by as GroupBy)) fail(`unknown group_by "${String(a.group_by)}" for field; known: ${list(GROUPS)}`)
      groupBy = a.group_by as GroupBy
    }
    action = {
      type: 'agentSetView', surface, ...base, filter: a.filter !== undefined ? taskFilter(s, a.filter) : undefined,
      sort: a.sort !== undefined ? sortOf(s, a.sort, 'field') : undefined, groupBy,
    }
  } else {
    if (a.group_by !== undefined) fail('group_by is not supported for tracks; use show_groups to group runs')
    let window: { from: number; to: number } | undefined
    if (a.window !== undefined) {
      if (!isObj(a.window)) return fail('window must be {from, to} as clock times such as "09:15" (the same clock query shows in "started")')
      const from = parseClock(String(a.window.from))
      const to = parseClock(String(a.window.to))
      if (from === null || to === null) return fail('window.from and window.to must look like "09:15" or "09:15:30"')
      if (to <= from) return fail('window.to must be after window.from')
      window = { from, to }
    }
    action = {
      type: 'agentSetView', surface, ...base, runFilter: a.filter !== undefined ? runFilterOf(s, a.filter) : undefined,
      sort: a.sort !== undefined ? sortOf(s, a.sort, 'tracks') : undefined, window,
    }
  }
  const next = reduce(s, action)
  const change = next.agent.changes[next.agent.changes.length - 1]
  const matching = surface === 'field' ? visibleTasks(next).length : trackRuns(next).length
  const n = surface === 'field' ? `${matching} of ${s.tasks.length} tasks` : `${matching} runs`
  return { ok: true, action, result: { applied: change.expr, shown: n, change: change.id, note: 'The user sees this as a chip with Undo and Save as view.' } }
}

function showGroups(s: State, a: Record<string, unknown>, source: string): ToolOutcome {
  const surface = surfaceOf(a, SETTABLE, 'show_groups') as 'field' | 'tracks'
  const ttl = title(a)
  const rest = a.rest ?? 'dim'
  if (rest !== 'dim' && rest !== 'hide') return fail(`rest must be "dim" or "hide", got "${String(rest)}"`)
  if (!Array.isArray(a.groups) || a.groups.length < 1 || a.groups.length > 12) return fail('groups must be a list of 1 to 12 groups')
  const known = new Set(surface === 'field' ? s.tasks.map((t) => t.id) : s.runs.map((r) => r.id))
  const kind = surface === 'field' ? 'task' : 'run'
  const seen = new Map<string, string>()
  const unknown: string[] = []
  const dup: string[] = []
  const groups: LensGroup[] = a.groups.map((g: unknown, i: number) => {
    if (!isObj(g)) return fail(`groups[${i}] must be {name, ids, reason}`)
    noExtra(g, ['name', 'ids', 'reason'], `groups[${i}]`)
    if (typeof g.name !== 'string' || !g.name.trim()) fail(`groups[${i}].name is required`)
    if (typeof g.reason !== 'string' || !g.reason.trim()) fail(`groups[${i}].reason is required: say why these belong together`)
    const ids = strings(g.ids, `groups[${i}].ids`)
    if (!ids.length) fail(`groups[${i}] ("${g.name}") has no ids`)
    for (const id of ids) {
      if (!known.has(id)) unknown.push(id)
      else if (seen.has(id)) dup.push(`${id} (in "${seen.get(id)}" and "${g.name}")`)
      else seen.set(id, String(g.name))
    }
    return { name: String(g.name).trim().slice(0, 60), ids, reason: String(g.reason).trim().slice(0, 600) }
  })
  if (unknown.length) fail(`unknown ${kind} id ${[...new Set(unknown)].join(', ')}; nothing was shown`)
  if (dup.length) fail(`each id may be in one group only; repeated: ${dup.join('; ')}. Leave an unsure ${kind} out and it appears under "not grouped"`)
  const action: Action = { type: 'agentShowGroups', surface, title: ttl, groups, rest, source }
  const next = reduce(s, action)
  const change = next.agent.changes[next.agent.changes.length - 1]
  const grouped = seen.size
  const shown = surface === 'field' ? visibleTasks(next).length : trackRuns(next).length
  return {
    ok: true, action,
    result: {
      change: change.id, groups: groups.length, grouped, not_grouped: Math.max(0, shown - grouped), rest,
      note: 'The user sees the groups with your reasons as a snapshot, with Undo and Save snapshot. They can launch a group themselves.',
    },
  }
}

function highlight(s: State, a: Record<string, unknown>, source: string): ToolOutcome {
  const surface = surfaceOf(a, SETTABLE, 'highlight') as 'field' | 'tracks'
  const ids = strings(a.ids, 'ids')
  if (!ids.length) fail('ids is empty')
  const known = new Set(surface === 'field' ? s.tasks.map((t) => t.id) : s.runs.map((r) => r.id))
  const bad = ids.filter((i) => !known.has(i))
  if (bad.length) fail(`unknown ${surface === 'field' ? 'task' : 'run'} id ${bad.join(', ')}; nothing was highlighted`)
  if (a.note !== undefined && typeof a.note !== 'string') fail('note must be a string')
  const note = ((a.note as string | undefined) ?? '').trim().slice(0, 400)
  const ttl = typeof a.title === 'string' && a.title.trim() ? a.title.trim().slice(0, 80) : `Highlight ${ids.length} ${surface === 'field' ? 'task' : 'run'}${ids.length === 1 ? '' : 's'}`
  const action: Action = { type: 'agentHighlight', surface, ids, note, title: ttl, source }
  const next = reduce(s, action)
  return { ok: true, action, result: { change: next.agent.changes[next.agent.changes.length - 1].id, highlighted: ids.length } }
}

function open(s: State, a: Record<string, unknown>, source: string): ToolOutcome {
  const surface = surfaceOf(a, OPENABLE, 'open') as 'field' | 'tracks' | 'queue' | 'experiments'
  let id: string | undefined
  if (a.id !== undefined) {
    if (typeof a.id !== 'string') return fail('id must be a string')
    id = a.id
    const pool: Record<string, string[]> = {
      field: s.tasks.map((t) => t.id), tracks: s.runs.map((r) => r.id), queue: s.queue.map((q) => q.id), experiments: s.experiments.map((e) => e.id),
    }
    if (!pool[surface].includes(id)) fail(`unknown ${surface} id "${id}"`)
  }
  const action: Action = { type: 'agentOpen', surface, id, source }
  const next = reduce(s, action)
  return { ok: true, action, result: { change: next.agent.changes[next.agent.changes.length - 1].id, opened: id ? `${surface} ${id}` : surface } }
}

export function execTool(s: State, name: string, raw: unknown, source = 'external'): ToolOutcome {
  try {
    if (!isObj(raw)) return { ok: false, error: 'arguments must be an object' }
    switch (name) {
      case 'query': return query(s, raw)
      case 'set_view': return setView(s, raw, source)
      case 'show_groups': return showGroups(s, raw, source)
      case 'highlight': return highlight(s, raw, source)
      case 'open': return open(s, raw, source)
      default: return { ok: false, error: `unknown tool "${name}"; known: ${list(TOOL_NAMES)}` }
    }
  } catch (e) {
    if (e instanceof ToolError) return { ok: false, error: e.message }
    throw e
  }
}
