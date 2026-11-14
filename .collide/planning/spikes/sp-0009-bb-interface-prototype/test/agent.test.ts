// The in-app agent: reducer actions with Undo, and the UI tools run against state (no browser, no server).
import { describe, expect, it } from 'vitest'
import { agentDemoState, BUILD_ORDER_DESC, PANTRY_GROUPS } from '../src/agentDemo'
import { execTool } from '../src/agent/exec'
import { reduce, type Action } from '../src/reducer'
import { initialState, trackRuns, visibleTasks } from '../src/world'
import type { State } from '../src/types'

let t = 1_000_000
const go = (s: State, ...actions: Action[]): State => actions.reduce((x, a) => reduce(x, { ...a, t: (t += 500) }), s)
const base = () => agentDemoState()

/** Run a tool the way the page does: execute against state, dispatch the action, return the outcome and the new state. */
function call(s: State, name: string, args: unknown) {
  const out = execTool(s, name, args, 'test')
  return { out, s: out.ok && out.action ? go(s, out.action) : s }
}

describe('demo data', () => {
  it('has twelve ready tasks in one epic and nine finished runs with distinct build times', () => {
    const s = base()
    const e9 = s.tasks.filter((x) => x.epic === 'E-09')
    expect(e9).toHaveLength(12)
    expect(e9.every((x) => x.status === 'ready' && !/\w+\/\w+\.\w+|\bsrc\/|\.(js|ts|json)\b/.test(x.body))).toBe(true)
    expect(s.runs.filter((r) => r.status === 'done')).toHaveLength(9)
    expect(Object.values(PANTRY_GROUPS).flat().every((id) => e9.some((x) => x.id === id))).toBe(true)
  })
  it('leaves the plain initial state untouched', () => {
    expect(initialState('orchard').tasks).toHaveLength(140)
    expect(initialState('orchard').runs).toHaveLength(0)
  })
})

describe('agentSetView on the Field', () => {
  it('applies a filter, shows an expression chip, and Undo restores the old filter', () => {
    const s0 = go(base(), { type: 'toggleFilter', group: 'status', value: 'blocked' })
    const s1 = go(s0, {
      type: 'agentSetView', surface: 'field', title: 'Larder, ready', source: 'test',
      filter: { epic: ['E-09'], status: ['ready'], label: [], source: [], text: '' }, sort: { by: 'title', dir: 'asc' }, groupBy: 'label',
    })
    expect(visibleTasks(s1)).toHaveLength(12)
    expect(s1.fieldView).toEqual({ sort: { by: 'title', dir: 'asc' }, groupBy: 'label' })
    const c = s1.agent.changes[0]
    expect(c.title).toBe('Larder, ready')
    expect(c.expr).toBe('epic == "E-09" && status == "ready" | sort title asc | group by label')
    const s2 = go(s1, { type: 'agentUndo' })
    expect(s2.filter.status).toEqual(['blocked'])
    expect(s2.filter.epic).toEqual([])
    expect(s2.fieldView).toEqual({ sort: null, groupBy: 'status' })
    expect(s2.agent.changes).toHaveLength(0)
  })
  it('Save as view keeps the view the change made, even after later edits, and the saved view reopens', () => {
    let s = go(base(), { type: 'agentSetView', surface: 'field', title: 'x', source: 't', filter: { epic: ['E-09'], status: [], label: [], source: [], text: '' }, sort: { by: 'id', dir: 'desc' } })
    s = go(s, { type: 'toggleFilter', group: 'label', value: 'ui' })
    s = go(s, { type: 'agentSaveView', changeId: s.agent.changes[0].id, name: 'Larder newest first' })
    expect(s.agent.changes[0].saved).toBe('Larder newest first')
    const v = s.views[s.views.length - 1]
    expect(v.filter.label).toEqual([])
    s = go(s, { type: 'clearFilter' }, { type: 'openView', id: v.id })
    expect(s.filter.epic).toEqual(['E-09'])
    expect(s.fieldView.sort).toEqual({ by: 'id', dir: 'desc' })
  })
})

describe('agentSetView on Tracks', () => {
  it('sorts runs by build-phase duration, longest first, and fits the axis', () => {
    const s = go(base(), { type: 'agentSetView', surface: 'tracks', title: 'Longest builds', source: 't', sort: { by: 'phase:build', dir: 'desc' } })
    expect(s.surface).toBe('tracks')
    expect(trackRuns(s).filter((r) => r.status === 'done').map((r) => r.id)).toEqual(BUILD_ORDER_DESC)
    expect(s.tracks.follow).toBe(false)
    expect(s.agent.changes[0].expr).toBe('true | sort phase_duration("build") desc')
  })
  it('lists a live run last because it has no finished build; filters by workflow; Undo puts the axis back', () => {
    const s0 = base()
    let s = go(s0, { type: 'agentSetView', surface: 'tracks', title: 'quick', source: 't', runFilter: { status: [], task: [], workflow: ['quick-fix'], ids: [] }, sort: { by: 'total', dir: 'asc' } })
    expect(trackRuns(s).map((r) => r.id).sort()).toEqual(['R-105', 'R-109'])
    s = go(s, { type: 'agentUndo' })
    expect(s.tracks).toEqual(s0.tracks)
    expect(s.tracksView.sort).toBeNull()
  })
  it('Save as view on tracks stores the run filter and sort', () => {
    let s = go(base(), { type: 'agentSetView', surface: 'tracks', title: 'b', source: 't', sort: { by: 'phase:build', dir: 'desc' } })
    s = go(s, { type: 'agentSaveView', changeId: 'A-1', name: 'Slow builds' }, { type: 'tracksViewReset' })
    expect(s.tracksView.sort).toBeNull()
    s = go(s, { type: 'openView', id: s.views[s.views.length - 1].id })
    expect(s.tracksView.sort).toEqual({ by: 'phase:build', dir: 'desc' })
    expect(s.surface).toBe('tracks')
  })
})

describe('lens', () => {
  const groups = [
    { name: 'Quantities', ids: ['T-09-001', 'T-09-002'], reason: 'both parse amounts' },
    { name: 'Data file', ids: ['T-09-011', 'T-09-012'], reason: 'both change saving' },
  ]
  it('agentShowGroups sets a stamped lens with reasons; Undo removes it; Save snapshot keeps it', () => {
    let s = go(base(), { type: 'agentShowGroups', surface: 'field', title: 'Related code', groups, rest: 'dim', source: 'test' })
    expect(s.lens?.groups).toHaveLength(2)
    expect(s.lens?.at).toBeGreaterThan(0)
    expect(s.agent.changes[0].reasons.map((r) => r.reason)).toEqual(['both parse amounts', 'both change saving'])
    s = go(s, { type: 'agentSaveSnapshot', changeId: 'A-1', name: 'Related code, Nov 14' })
    expect(s.snapshots).toHaveLength(1)
    s = go(s, { type: 'lensClear' })
    expect(s.lens).toBeNull()
    s = go(s, { type: 'openSnapshot', id: s.snapshots[0].id })
    expect(s.lens?.name).toBe('Related code, Nov 14')
    s = go(s, { type: 'agentUndo' })
    expect(s.lens).toBeNull()
  })
  it('selectIds adds a group to the selection and does not launch anything', () => {
    const s0 = go(base(), { type: 'agentShowGroups', surface: 'field', title: 'x', groups, rest: 'dim', source: 't' })
    const s = go(s0, { type: 'selectIds', ids: groups[0].ids })
    expect(s.selected).toEqual(['T-09-001', 'T-09-002'])
    expect(s.runs).toHaveLength(s0.runs.length)
  })
  it('a Tracks lens lists its runs under group names', () => {
    const s = go(base(), { type: 'agentShowGroups', surface: 'tracks', title: 'g', source: 't', rest: 'hide', groups: [{ name: 'long', ids: ['R-102', 'R-108'], reason: 'over 20 min of build' }] })
    expect(trackRuns(s).map((r) => r.id).sort()).toEqual(['R-102', 'R-108'])
  })
})

describe('highlight and open', () => {
  it('highlight marks ids and Undo clears it', () => {
    let s = go(base(), { type: 'agentHighlight', surface: 'field', ids: ['T-09-004'], note: 'start here', title: 'Start here', source: 't' })
    expect(s.highlight).toEqual({ surface: 'field', ids: ['T-09-004'], note: 'start here' })
    s = go(s, { type: 'agentUndo' })
    expect(s.highlight).toBeNull()
  })
  it('open shows a task drawer, a run detail and a queue item; Undo closes it', () => {
    let s = go(base(), { type: 'agentOpen', surface: 'field', id: 'T-09-004', source: 't' })
    expect(s.drawerTask).toBe('T-09-004')
    s = go(s, { type: 'agentOpen', surface: 'tracks', id: 'R-102', source: 't' })
    expect(s.surface).toBe('tracks')
    expect(s.detail?.runId).toBe('R-102')
    s = go(s, { type: 'agentUndo' })
    expect(s.detail).toBeNull()
    expect(s.surface).toBe('field')
  })
})

describe('ask state', () => {
  it('records time to first tool call and first chip, and the end state', () => {
    let s = go(base(), { type: 'askStart', request: 'q', agent: 'A' })
    expect(s.agent.ask.status).toBe('working')
    s = go(s, { type: 'askLine', text: 'query tasks ok', tool: true })
    s = go(s, { type: 'agentHighlight', surface: 'field', ids: ['T-09-001'], note: '', title: 'h', source: 't' })
    s = go(s, { type: 'askEnd', ok: true, text: 'done', changes: 1, calls: 2, usage: { costUsd: 0.02, inputTokens: 100, outputTokens: 10 } })
    const a = s.agent.ask
    expect(a.status).toBe('done')
    expect(a.firstToolAt).not.toBeNull()
    expect(a.chipAt).toBeGreaterThan(a.firstToolAt!)
    expect(a.usage?.costUsd).toBe(0.02)
  })
  it('a finished ask with no change, and a failed ask, are shown as such', () => {
    let s = go(base(), { type: 'askStart', request: 'q', agent: 'A' }, { type: 'askEnd', ok: true, text: 'I do not know', changes: 0, calls: 1 })
    expect(s.agent.ask.status).toBe('nochange')
    s = go(s, { type: 'askStart', request: 'q', agent: 'A' }, { type: 'askEnd', ok: false, error: 'timed out after 5 s', changes: 0 })
    expect(s.agent.ask.status).toBe('failed')
    expect(s.agent.ask.error).toBe('timed out after 5 s')
  })
})

describe('settings actions', () => {
  const loaded = () => go(base(), {
    type: 'settingsLoaded',
    settings: {
      askAgent: 'claude',
      agents: [{ id: 'claude', name: 'C', kind: 'claude-code', command: 'claude', args: '', model: 'haiku', folder: 'f', budgetUsd: 0.2, timeoutSec: 60, env: '' }],
    },
  })
  it('edit marks dirty, saved clears it, add and remove keep a usable ask agent', () => {
    let s = go(loaded(), { type: 'settingsEdit', id: 'claude', patch: { budgetUsd: 0.5 } })
    expect(s.agentSettings.dirty).toBe(true)
    expect(s.agentSettings.settings?.agents[0].budgetUsd).toBe(0.5)
    s = go(s, { type: 'settingsSaved' })
    expect(s.agentSettings.dirty).toBe(false)
    s = go(s, { type: 'settingsAdd', kind: 'external' })
    expect(s.agentSettings.settings?.agents).toHaveLength(2)
    expect(s.agentSettings.settings?.askAgent).toBe('claude')
    s = go(s, { type: 'settingsAskAgent', id: 'agent-2' })
    expect(s.agentSettings.settings?.askAgent).toBe('claude')
    s = go(s, { type: 'settingsRemove', id: 'claude' })
    expect(s.agentSettings.settings?.askAgent).toBeNull()
  })
})

describe('tools run against state', () => {
  it('query tasks returns the rows the Field shows, filtered', () => {
    const { out } = call(base(), 'query', { subject: 'tasks', filter: { epic: ['E-09'], status: ['ready'] } })
    expect(out.ok && (out.result as { count: number }).count).toBe(12)
    expect(out.ok && (out.result as { rows: { body: string }[] }).rows[0].body).toMatch(/Fractions/)
  })
  it('query runs returns phases with seconds', () => {
    const { out } = call(base(), 'query', { subject: 'runs', filter: { status: ['done'] } })
    const rows = (out.ok ? out.result : { rows: [] }) as { rows: { id: string; phases: { key: string; seconds: number }[] }[] }
    expect(rows.rows).toHaveLength(9)
    expect(rows.rows.find((r) => r.id === 'R-108')?.phases.find((p) => p.key === 'build')?.seconds).toBe(1500)
  })
  it('set_view sorts Tracks by build time through the same reducer', () => {
    const { out, s } = call(base(), 'set_view', { surface: 'tracks', title: 'Longest builds', sort: { by: 'phase:build', dir: 'desc' } })
    expect(out.ok).toBe(true)
    expect(trackRuns(s)[0].id).toBe(BUILD_ORDER_DESC[0])
  })
  it('show_groups draws a lens and reports what was left out', () => {
    const { out, s } = call(base(), 'show_groups', {
      surface: 'field', title: 'g', groups: [{ name: 'a', ids: ['T-09-001'], reason: 'r' }],
    })
    expect(out.ok && (out.result as { grouped: number }).grouped).toBe(1)
    expect(s.lens?.groups[0].name).toBe('a')
  })
  it('no tool launches, accepts, edits or deletes: runs, queue and tasks are unchanged after every tool', () => {
    let s = base()
    const before = JSON.stringify({ r: s.runs.length, q: s.queue, t: s.tasks, v: s.views.length })
    for (const [n, a] of [
      ['query', { subject: 'tasks' }],
      ['set_view', { surface: 'field', title: 't', filter: { epic: ['E-09'] } }],
      ['show_groups', { surface: 'field', title: 'g', groups: [{ name: 'a', ids: ['T-09-001'], reason: 'r' }] }],
      ['highlight', { surface: 'field', ids: ['T-09-002'] }],
      ['open', { surface: 'queue' }],
    ] as const) s = call(s, n, a).s
    expect(JSON.stringify({ r: s.runs.length, q: s.queue, t: s.tasks, v: s.views.length })).toBe(before)
    expect(s.session.attempts).toHaveLength(0)
  })
})

describe('tool input errors', () => {
  const s = base()
  const err = (name: string, args: unknown): string => {
    const o = execTool(s, name, args)
    if (o.ok) throw new Error('expected an error')
    return o.error
  }
  it('names an unknown surface and lists the supported ones', () => {
    expect(err('set_view', { surface: 'queue', title: 'x', sort: { by: 'id' } })).toMatch(/unknown surface "queue" for set_view; supported: field, tracks/)
    expect(err('open', { surface: 'compare' })).toMatch(/unknown surface "compare" for open; supported: field, tracks, queue, experiments/)
    expect(err('highlight', { ids: ['T-09-001'] })).toMatch(/surface is required/)
  })
  it('names an unknown id and changes nothing', () => {
    expect(err('show_groups', { surface: 'field', title: 'g', groups: [{ name: 'a', ids: ['T-09-001', 'T-99-001'], reason: 'r' }] })).toMatch(/unknown task id T-99-001; nothing was shown/)
    expect(err('highlight', { surface: 'tracks', ids: ['R-999'] })).toMatch(/unknown run id R-999/)
    expect(err('open', { surface: 'field', id: 'nope' })).toMatch(/unknown field id "nope"/)
  })
  it('rejects an id in two groups, an empty group, a missing reason or title', () => {
    expect(err('show_groups', { surface: 'field', title: 'g', groups: [{ name: 'a', ids: ['T-09-001'], reason: 'r' }, { name: 'b', ids: ['T-09-001'], reason: 'r' }] })).toMatch(/one group only/)
    expect(err('show_groups', { surface: 'field', title: 'g', groups: [{ name: 'a', ids: [], reason: 'r' }] })).toMatch(/has no ids/)
    expect(err('show_groups', { surface: 'field', title: 'g', groups: [{ name: 'a', ids: ['T-09-001'], reason: '' }] })).toMatch(/reason is required/)
    expect(err('set_view', { surface: 'field', filter: {} })).toMatch(/title is required/)
  })
  it('rejects bad filters, sorts, grouping and windows with what is allowed', () => {
    expect(err('query', { subject: 'tasks', filter: { epic: ['E-77'] } })).toMatch(/unknown epic "E-77"; known: E-01/)
    expect(err('query', { subject: 'runs', filter: { epic: ['E-09'] } })).toMatch(/unknown key "epic"; allowed: status, task, workflow, ids/)
    expect(err('query', { subject: 'things' })).toMatch(/unknown subject/)
    expect(err('set_view', { surface: 'tracks', title: 't', sort: { by: 'phase:nope' } })).toMatch(/unknown phase "nope"/)
    expect(err('set_view', { surface: 'tracks', title: 't', sort: { by: 'cost' } })).toMatch(/unknown sort "cost" for tracks/)
    expect(err('set_view', { surface: 'field', title: 't', group_by: 'colour' })).toMatch(/unknown group_by "colour"/)
    expect(err('set_view', { surface: 'tracks', title: 't', group_by: 'epic' })).toMatch(/not supported for tracks/)
    expect(err('set_view', { surface: 'tracks', title: 't', window: { from: '10:00', to: '09:00' } })).toMatch(/after window.from/)
    expect(err('set_view', { surface: 'field', title: 't' })).toMatch(/at least one of/)
  })
  it('an unknown tool is named', () => {
    expect(err('launch', { ids: ['T-09-001'] })).toMatch(/unknown tool "launch"; known: query, set_view, show_groups, highlight, open/)
  })
})
