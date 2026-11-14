// Reducer cases for the in-app agent and the Agents settings. Every agent tool call becomes one of these actions, the
// same way a click does. The agent only changes view state: no case here launches, accepts, edits or deletes anything.
import { celOf, EMPTY_FILTER, emptyAsk, emptyRunFilter, runExpr, trackRuns } from './world'
import type {
  AgentChange, AgentConfig, AgentKind, AgentSettings, Filter, GroupBy, LensGroup, RunFilter, Snap, SortSpec, State,
} from './types'

export type AgentAction =
  | { type: 'agentSetView'; surface: 'field' | 'tracks'; title: string; reason?: string; filter?: Filter; runFilter?: RunFilter; sort?: SortSpec | null; groupBy?: GroupBy; window?: { from: number; to: number }; source: string }
  | { type: 'agentShowGroups'; surface: 'field' | 'tracks'; title: string; groups: LensGroup[]; rest: 'dim' | 'hide'; source: string }
  | { type: 'agentHighlight'; surface: 'field' | 'tracks'; ids: string[]; note: string; title: string; source: string }
  | { type: 'agentOpen'; surface: 'field' | 'tracks' | 'queue' | 'experiments'; id?: string; source: string }
  | { type: 'agentUndo' }
  | { type: 'agentSaveView'; changeId: string; name: string }
  | { type: 'agentSaveSnapshot'; changeId: string; name: string }
  | { type: 'openSnapshot'; id: string }
  | { type: 'lensClear' }
  | { type: 'selectIds'; ids: string[] }
  | { type: 'fieldViewReset' }
  | { type: 'tracksViewReset' }
  | { type: 'askStart'; request: string; agent: string }
  | { type: 'askLine'; text: string; tool?: boolean }
  | { type: 'askEnd'; ok: boolean; text?: string; error?: string; usage?: AgentStateUsage; calls?: number; changes: number }
  | { type: 'askDismiss' }
  | { type: 'settingsLoaded'; settings: AgentSettings }
  | { type: 'settingsError'; error: string }
  | { type: 'settingsEdit'; id: string; patch: Partial<Omit<AgentConfig, 'id'>> }
  | { type: 'settingsAdd'; kind: AgentKind }
  | { type: 'settingsRemove'; id: string }
  | { type: 'settingsAskAgent'; id: string }
  | { type: 'settingsSaved' }

type AgentStateUsage = NonNullable<State['agent']['ask']['usage']>

const clone = <T,>(x: T): T => JSON.parse(JSON.stringify(x))

export function takeSnap(s: State): Snap {
  return clone({
    surface: s.surface, filter: s.filter, activeView: s.activeView, fieldView: s.fieldView, tracksView: s.tracksView,
    tracks: s.tracks, lens: s.lens, highlight: s.highlight, drawerTask: s.drawerTask, detail: s.detail, queueSel: s.queueSel,
    expOpen: s.expOpen,
  })
}

function restore(s: State, p: Snap): void {
  s.surface = p.surface
  s.filter = p.filter
  s.activeView = p.activeView
  s.fieldView = p.fieldView
  s.tracksView = p.tracksView
  s.tracks = p.tracks
  s.lens = p.lens
  s.highlight = p.highlight
  s.drawerTask = p.drawerTask
  s.detail = p.detail
  s.queueSel = p.queueSel
  s.expOpen = p.expOpen
}

export const fieldExpr = (s: State): string => {
  const v = s.fieldView
  const parts = [celOf(s.filter)]
  if (v.sort) parts.push(`sort ${v.sort.by} ${v.sort.dir}`)
  if (v.groupBy !== 'status') parts.push(`group by ${v.groupBy}`)
  return parts.join(' | ')
}

/** The first change made while an ask is running is the moment the chip appears. */
function markChip(s: State): void {
  const ask = s.agent.ask
  if (ask.status === 'working' && ask.chipAt === null) ask.chipAt = s.realNow
}

function push(s: State, c: Omit<AgentChange, 'id' | 'at' | 'prev' | 'saved'>, prev: Snap): void {
  s.agent.n += 1
  s.agent.changes.push({ ...c, id: `A-${s.agent.n}`, at: s.realNow, prev, saved: null })
  markChip(s)
}

/** Fit the Tracks axis to the runs a view lists, so a sorted view is readable without panning. */
function fitTracks(s: State): void {
  const runs = trackRuns(s)
  if (!runs.length) return
  const a = Math.min(...runs.map((r) => r.startWall))
  const b = Math.max(...runs.map((r) => r.endWall ?? s.simNow))
  s.tracks.follow = false
  s.tracks.start = a - 60
  s.tracks.span = Math.max(300, b - a + 120)
}

export function applyAgent(s: State, a: AgentAction): boolean {
  switch (a.type) {
    case 'agentSetView': {
      const prev = takeSnap(s)
      let expr: string
      let view: AgentChange['view']
      if (a.surface === 'field') {
        if (a.filter) s.filter = clone({ ...EMPTY_FILTER, ...a.filter })
        if (a.sort !== undefined) s.fieldView.sort = a.sort
        if (a.groupBy) s.fieldView.groupBy = a.groupBy
        s.activeView = null
        s.lens = null
        expr = fieldExpr(s)
        view = { surface: 'field', cel: expr, filter: clone(s.filter), sort: clone(s.fieldView.sort), groupBy: s.fieldView.groupBy }
      } else {
        if (a.runFilter) s.tracksView.filter = clone(a.runFilter)
        if (a.sort !== undefined) s.tracksView.sort = a.sort
        s.lens = s.lens?.surface === 'tracks' ? null : s.lens
        if (a.window) {
          s.tracks.follow = false
          s.tracks.start = a.window.from
          s.tracks.span = Math.max(300, a.window.to - a.window.from)
        } else fitTracks(s)
        expr = runExpr(s.tracksView.filter, s.tracksView.sort)
        view = { surface: 'tracks', cel: expr, filter: clone(EMPTY_FILTER), sort: clone(s.tracksView.sort), runFilter: clone(s.tracksView.filter) }
      }
      s.surface = a.surface
      s.returnTo = null
      push(s, {
        tool: 'set_view', surface: a.surface, title: a.title, expr, reasons: a.reason ? [{ name: '', reason: a.reason, n: 0 }] : [],
        note: '', source: a.source, view, lensId: null,
      }, prev)
      return true
    }
    case 'agentShowGroups': {
      const prev = takeSnap(s)
      const lens = {
        id: `L-${s.agent.n + 1}`, surface: a.surface, title: a.title, groups: clone(a.groups), rest: a.rest, at: s.realNow, source: a.source,
      }
      s.lens = lens
      s.surface = a.surface
      s.returnTo = null
      push(s, {
        tool: 'show_groups', surface: a.surface, title: a.title, expr: '',
        reasons: a.groups.map((g) => ({ name: g.name, reason: g.reason, n: g.ids.length })), note: '', source: a.source, view: null,
        lensId: lens.id,
      }, prev)
      return true
    }
    case 'agentHighlight': {
      const prev = takeSnap(s)
      s.highlight = { surface: a.surface, ids: [...a.ids], note: a.note }
      s.surface = a.surface
      s.returnTo = null
      push(s, {
        tool: 'highlight', surface: a.surface, title: a.title, expr: '', reasons: [], note: a.note, source: a.source, view: null, lensId: null,
      }, prev)
      return true
    }
    case 'agentOpen': {
      const prev = takeSnap(s)
      s.surface = a.surface
      s.returnTo = null
      if (a.surface === 'field' && a.id) s.drawerTask = a.id
      if (a.surface === 'tracks' && a.id) {
        s.detail = { runId: a.id, tab: 'output', phase: null, focusCall: null, follow: true, bd: { tool: null, minDur: 0, win: null, phase: null } }
      }
      if (a.surface === 'queue' && a.id) s.queueSel = a.id
      if (a.surface === 'experiments' && a.id) s.expOpen = a.id
      push(s, {
        tool: 'open', surface: a.surface, title: a.id ? `Open ${a.surface} ${a.id}` : `Open ${a.surface}`, expr: '', reasons: [], note: '',
        source: a.source, view: null, lensId: null,
      }, prev)
      return true
    }
    case 'agentUndo': {
      const c = s.agent.changes.pop()
      if (c) restore(s, c.prev)
      return true
    }
    case 'agentSaveView': {
      const c = s.agent.changes.find((x) => x.id === a.changeId)
      const name = a.name.trim()
      if (!c?.view || !name) return true
      const id = `v-${s.views.length + 1}`
      s.views.push({ ...clone(c.view), id, name })
      c.saved = name
      if (c.view.surface !== 'tracks') s.activeView = id
      return true
    }
    case 'agentSaveSnapshot': {
      const c = s.agent.changes.find((x) => x.id === a.changeId)
      const name = a.name.trim()
      const lens = c?.lensId ? (s.lens?.id === c.lensId ? s.lens : null) : null
      if (!c || !name || !lens) return true
      lens.name = name
      s.snapshots.push(clone(lens))
      c.saved = name
      return true
    }
    case 'openSnapshot': {
      const l = s.snapshots.find((x) => x.id === a.id)
      if (!l) return true
      s.lens = clone(l)
      s.surface = l.surface
      s.returnTo = null
      return true
    }
    case 'lensClear':
      s.lens = null
      s.highlight = null
      return true
    case 'selectIds':
      s.selected = [...new Set([...s.selected, ...a.ids])]
      return true
    case 'fieldViewReset':
      s.fieldView = { sort: null, groupBy: 'status' }
      return true
    case 'tracksViewReset':
      s.tracksView = { filter: emptyRunFilter(), sort: null }
      return true
    case 'askStart':
      s.agent.ask = { ...emptyAsk(), status: 'working', request: a.request, agent: a.agent, startedAt: s.realNow }
      return true
    case 'askLine': {
      const ask = s.agent.ask
      ask.lines.push({ t: s.realNow, text: a.text })
      if (a.tool) {
        ask.calls += 1
        if (ask.firstToolAt === null) ask.firstToolAt = s.realNow
      }
      return true
    }
    case 'askEnd': {
      const ask = s.agent.ask
      ask.doneAt = s.realNow
      ask.text = a.text ?? ''
      ask.error = a.error ?? ''
      ask.usage = a.usage ?? null
      if (a.calls !== undefined) ask.calls = a.calls
      ask.status = !a.ok ? 'failed' : a.changes === 0 ? 'nochange' : 'done'
      return true
    }
    case 'askDismiss':
      s.agent.ask = emptyAsk()
      return true
    case 'settingsLoaded':
      s.agentSettings = { settings: clone(a.settings), loaded: true, dirty: false, error: null, savedAt: s.agentSettings.savedAt }
      return true
    case 'settingsError':
      s.agentSettings.error = a.error
      return true
    case 'settingsEdit': {
      const c = s.agentSettings.settings?.agents.find((x) => x.id === a.id)
      if (!c) return true
      Object.assign(c, a.patch)
      s.agentSettings.dirty = true
      return true
    }
    case 'settingsAdd': {
      const st = s.agentSettings.settings
      if (!st) return true
      let n = st.agents.length + 1
      while (st.agents.some((x) => x.id === `agent-${n}`)) n += 1
      const base: AgentConfig = {
        id: `agent-${n}`, name: a.kind === 'external' ? 'My own agent' : a.kind === 'acp' ? 'ACP agent' : 'Claude Code', kind: a.kind,
        command: a.kind === 'claude-code' ? 'claude' : '', args: a.kind === 'acp' ? 'acp' : '', model: a.kind === 'claude-code' ? 'haiku' : '',
        folder: st.agents[0]?.folder ?? '', budgetUsd: 0.25, timeoutSec: 180, env: '',
      }
      st.agents.push(base)
      st.askAgent ??= base.id
      s.agentSettings.dirty = true
      return true
    }
    case 'settingsRemove': {
      const st = s.agentSettings.settings
      if (!st) return true
      st.agents = st.agents.filter((x) => x.id !== a.id)
      if (st.askAgent === a.id) st.askAgent = st.agents.find((x) => x.kind !== 'external')?.id ?? null
      s.agentSettings.dirty = true
      return true
    }
    case 'settingsAskAgent': {
      const st = s.agentSettings.settings
      if (st && st.agents.some((x) => x.id === a.id && x.kind !== 'external')) {
        st.askAgent = a.id
        s.agentSettings.dirty = true
      }
      return true
    }
    case 'settingsSaved':
      s.agentSettings.dirty = false
      s.agentSettings.error = null
      s.agentSettings.savedAt = s.realNow
      return true
  }
  return false
}

