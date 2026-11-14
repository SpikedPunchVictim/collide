// The app's state machine. Every user action is one reducer action, so the pilot tests can drive it without a DOM.
import { produce, setAutoFreeze } from 'immer'
import { actFor, KIND_ORDER, needsConfirm } from './items'
import { FULL_WORKFLOW, SHORT_WORKFLOW } from './gen'
import { scenarioById } from './scenarios'
import { advanceAll, resolveItem } from './sim'
import { addRun, celOf, EMPTY_FILTER, emptySession, isLive, taskOf, visibleTasks } from './world'
import type {
  Attempt, BreakdownFilter, DetailTab, Effect, Filter, QueueItem, State, Surface, Variants,
} from './types'
import { NO_FILTER } from './breakdown'

setAutoFreeze(false)

export type Action =
  | { type: 'tick'; t: number }
  | { type: 'advance'; seconds: number }
  | { type: 'nav'; surface: Surface }
  | { type: 'toggleFilter'; group: 'epic' | 'status' | 'label' | 'source'; value: string }
  | { type: 'filterText'; text: string }
  | { type: 'clearFilter' }
  | { type: 'startSave' }
  | { type: 'cancelSave' }
  | { type: 'saveView'; name: string }
  | { type: 'openView'; id: string }
  | { type: 'toggleSelect'; id: string }
  | { type: 'selectVisible' }
  | { type: 'clearSelect' }
  | { type: 'openLaunch' }
  | { type: 'closeLaunch' }
  | { type: 'setLaunch'; mode?: 'separate' | 'group'; workflow?: string }
  | { type: 'confirmLaunch' }
  | { type: 'promptText'; text: string }
  | { type: 'promptLaunch' }
  | { type: 'addPack' }
  | { type: 'newSession' }
  | { type: 'openDrawer'; id: string }
  | { type: 'closeDrawer' }
  | { type: 'selectRun'; runId: string; tab?: DetailTab; phase?: string | null }
  | { type: 'closeDetail' }
  | { type: 'setTab'; tab: DetailTab }
  | { type: 'setBd'; patch: Partial<BreakdownFilter> }
  | { type: 'focusCall'; id: string }
  | { type: 'follow'; on: boolean }
  | { type: 'queueSelect'; id: string }
  | { type: 'queueMove'; d: number }
  | { type: 'toggleCtx' }
  | { type: 'answer'; key: string }
  | { type: 'disarm' }
  | { type: 'undo' }
  | { type: 'openTrack' }
  | { type: 'back' }
  | { type: 'zoom'; dir: 1 | -1 }
  | { type: 'pan'; dir: 1 | -1 }
  | { type: 'trackFollow'; on: boolean }
  | { type: 'hover'; runId: string | null }
  | { type: 'compareToggle'; runId: string }
  | { type: 'openCompare' }
  | { type: 'compareNote'; text: string }
  | { type: 'compareSave' }
  | { type: 'reviewDecide'; runId: string; effect: 'accept' | 'reprompt' | 'discard' }
  | { type: 'expDraft'; patch: Partial<State['expDraft']> }
  | { type: 'expStart' }
  | { type: 'expOpen'; id: string }
  | { type: 'expNote'; text: string }
  | { type: 'expNoteSave' }
  | { type: 'setLine'; line: string }
  | { type: 'setVariant'; key: keyof Variants; value: string }
  | { type: 'missingNote'; itemId: string; text: string }
  | { type: 'loadScenario'; id: string }
  | { type: 'startTimer' }
  | { type: 'stopTimer'; correct?: boolean | null }
  | { type: 'addError'; note?: string }
  | { type: 'setNotes'; text: string }
  | { type: 'setParticipant'; name: string }
  | { type: 'setSpeed'; speed: number }
  | { type: 'setPaused'; paused: boolean }
  | { type: 'setFacilitator'; on: boolean }
  | { type: 'resetSession' }

export type Timed = Action & { t?: number }

/** Pending items in display order: grouped by kind, longest-blocking first within a group. */
export function queueOrder(s: State): QueueItem[] {
  return s.queue
    .filter((q) => q.status === 'pending')
    .sort((a, b) => KIND_ORDER.indexOf(a.kind) - KIND_ORDER.indexOf(b.kind) || a.raisedWall - b.raisedWall)
}

const TICK_CAP_MS = 1000

export function reduce(state: State, action: Timed): State {
  const next = produce(state, (s) => {
    if (action.t !== undefined) {
      if (action.type === 'tick') {
        const dt = s.realNow ? Math.min(TICK_CAP_MS, Math.max(0, action.t - s.realNow)) : 0
        s.realNow = action.t
        if (!s.paused && dt > 0) step(s, (dt / 1000) * s.speed)
        return finish(s, action)
      }
      s.realNow = action.t
    }
    apply(s, action, state)
    finish(s, action)
  })
  return next
}

function step(s: State, seconds: number): void {
  advanceAll(s, s.simNow + seconds)
  if (s.tracks.follow) s.tracks.start = s.simNow - s.tracks.span * 0.7
}

/** Logging, end-state detection and auto flags, after every action. */
function finish(s: State, a: Timed): void {
  const ss = s.session
  if (ss.phase !== 'running' || !ss.scenarioId) return
  if (a.type !== 'tick' && a.type !== 'advance' && a.type !== 'hover') {
    const { t: _t, ...rest } = a as Action & { t?: number }
    ss.log.push({ t: s.realNow, a: a.type, d: JSON.stringify(rest).slice(0, 140) })
  }
  if (ss.scenarioId === '9' && (s.detail || s.surface === 'tracks') && !ss.autoFlags.includes('left the context view')) {
    ss.autoFlags.push('left the context view')
  }
  const sc = scenarioById(ss.scenarioId)
  if (sc?.detect && sc.detect(s)) stop(s, 'detected', null)
}

function autoError(s: State, note: string): void {
  s.session.errors += 1
  s.session.errorLog.push({ t: s.realNow, note, auto: true })
}

function stop(s: State, reason: 'detected' | 'manual', correct: boolean | null): void {
  const ss = s.session
  if (ss.phase !== 'running' || !ss.scenarioId || ss.startedAt === null) return
  const sc = scenarioById(ss.scenarioId)
  const att: Attempt = {
    scenario: ss.scenarioId, title: sc?.title ?? '', variants: { ...s.variants }, startedAt: ss.startedAt, endedAt: s.realNow,
    ms: s.realNow - ss.startedAt, errors: ss.errors, errorLog: ss.errorLog, autoFlags: ss.autoFlags, notes: ss.notes,
    stop: reason, detected: reason === 'detected', correct: reason === 'manual' ? correct : undefined,
    decisions: ss.decisions, missing: ss.missing, finding: ss.finding ?? undefined,
    log: ss.log.filter((l) => l.t >= (ss.startedAt ?? 0)),
  }
  ss.attempts.push(att)
  ss.phase = 'done'
  ss.detected = reason === 'detected'
}

function apply(s: State, a: Timed, base: State): void {
  switch (a.type) {
    case 'tick':
      return
    case 'advance':
      return step(s, a.seconds)
    case 'nav':
      s.surface = a.surface
      s.returnTo = null
      s.tracks.focus = null
      return
    case 'toggleFilter': {
      const arr = s.filter[a.group] as string[]
      const i = arr.indexOf(a.value)
      if (i >= 0) arr.splice(i, 1)
      else arr.push(a.value)
      s.activeView = null
      return
    }
    case 'filterText':
      s.filter.text = a.text
      s.activeView = null
      return
    case 'clearFilter':
      s.filter = { ...EMPTY_FILTER, epic: [], status: [], label: [], source: [] }
      s.activeView = null
      s.saving = false
      return
    case 'startSave':
      s.saving = true
      return
    case 'cancelSave':
      s.saving = false
      return
    case 'saveView': {
      const name = a.name.trim()
      if (!name) return
      const id = `v-${s.views.length + 1}`
      const f: Filter = JSON.parse(JSON.stringify(s.filter))
      s.views.push({ id, name, cel: celOf(f), filter: f })
      s.activeView = id
      s.saving = false
      return
    }
    case 'openView': {
      const v = s.views.find((x) => x.id === a.id)
      if (!v) return
      s.filter = JSON.parse(JSON.stringify(v.filter))
      s.activeView = v.id
      return
    }
    case 'toggleSelect': {
      const i = s.selected.indexOf(a.id)
      if (i >= 0) s.selected.splice(i, 1)
      else s.selected.push(a.id)
      return
    }
    case 'selectVisible':
      s.selected = visibleTasks(s).filter((t) => t.status !== 'done').map((t) => t.id)
      return
    case 'clearSelect':
      s.selected = []
      s.launch.open = false
      return
    case 'openLaunch':
      if (s.selected.length) s.launch.open = true
      return
    case 'closeLaunch':
      s.launch.open = false
      return
    case 'setLaunch':
      if (a.mode) s.launch.mode = a.mode
      if (a.workflow) s.launch.workflow = a.workflow
      return
    case 'confirmLaunch': {
      if (!s.selected.length) return
      const group = s.launch.mode === 'group' && s.selected.length > 1
      const gid = group ? `G-${s.counter + 1}` : undefined
      if (!group && s.session.scenarioId === '3' && s.session.phase === 'running') autoError(s, 'launched separately, not as a group')
      for (const id of s.selected) {
        const t = taskOf(s, id)
        if (!t) continue
        s.counter += 1
        addRun(s, {
          id: `R-${s.counter}`, taskId: id, title: t.title, startWall: s.simNow, groupId: gid, line: s.line,
          workflow: s.launch.workflow,
          gen: { seed: 1000 + s.counter, phases: s.launch.workflow === 'quick-fix' ? SHORT_WORKFLOW : FULL_WORKFLOW },
        })
      }
      s.selected = []
      s.launch.open = false
      return
    }
    case 'promptText':
      s.promptText = a.text
      return
    case 'promptLaunch': {
      const text = s.promptText.trim()
      if (!text) return
      s.counter += 1
      const id = `S-${s.counter}`
      s.tasks.push({ id, title: text, epic: '', status: 'ready', labels: [], source: 'tasks/', dependsOn: [], body: text })
      addRun(s, { id: `R-${s.counter}`, taskId: id, title: text, startWall: s.simNow, workflow: 'built-in', gen: { seed: 2000 + s.counter, phases: SHORT_WORKFLOW } })
      s.promptText = ''
      return
    }
    case 'addPack':
      s.packAdded = true
      return
    case 'newSession':
      return
    case 'openDrawer':
      s.drawerTask = a.id
      return
    case 'closeDrawer':
      s.drawerTask = null
      return
    case 'selectRun':
      s.detail = {
        runId: a.runId, tab: a.tab ?? 'output', phase: a.phase ?? null, focusCall: null, follow: true,
        bd: { ...NO_FILTER, phase: a.phase ?? null },
      }
      return
    case 'closeDetail':
      s.detail = null
      return
    case 'setTab':
      if (s.detail) s.detail.tab = a.tab
      return
    case 'setBd':
      if (s.detail) s.detail.bd = { ...s.detail.bd, ...a.patch }
      return
    case 'focusCall':
      if (s.detail) {
        s.detail.tab = 'output'
        s.detail.focusCall = a.id
        s.detail.follow = false
      }
      return
    case 'follow':
      if (s.detail) {
        s.detail.follow = a.on
        if (a.on) s.detail.focusCall = null
      }
      return
    case 'queueSelect':
      s.queueSel = a.id
      s.armed = null
      return
    case 'queueMove': {
      const order = queueOrder(s)
      if (!order.length) return
      const i = order.findIndex((q) => q.id === s.queueSel)
      s.queueSel = order[Math.max(0, Math.min(order.length - 1, (i < 0 ? 0 : i) + a.d))].id
      s.armed = null
      return
    }
    case 'toggleCtx':
      s.ctxOpen = !s.ctxOpen
      return
    case 'disarm':
      s.armed = null
      return
    case 'answer': {
      const order = queueOrder(s)
      const it = order.find((q) => q.id === s.queueSel) ?? order[0]
      if (!it) return
      const act = actFor(s.variants.keys, it.kind, a.key)
      if (!act) return
      if (needsConfirm(s.variants.keys, act) && !(s.armed && s.armed.itemId === it.id && s.armed.key === a.key)) {
        s.armed = { itemId: it.id, key: a.key }
        return
      }
      s.armed = null
      const before = base.runs.find((r) => r.id === it.runId)
      const matched = it.expect.includes(act.effect)
      resolveItem(s, it.id, a.key, act.effect, matched)
      if (before) s.lastAnswer = { itemId: it.id, run: JSON.parse(JSON.stringify(before)), at: s.realNow }
      s.session.decisions.push({ item: it.id, kind: it.kind, key: a.key, effect: act.effect, matched, t: s.realNow })
      const rest = queueOrder(s)
      s.queueSel = rest.find((q) => q.kind === it.kind)?.id ?? rest[0]?.id ?? null
      return
    }
    case 'undo': {
      const la = s.lastAnswer
      if (!la) return
      const it = s.queue.find((q) => q.id === la.itemId)
      const i = s.runs.findIndex((r) => r.id === la.run.id)
      if (!it || i < 0) return
      s.runs[i] = la.run
      it.status = 'pending'
      it.answer = undefined
      s.queueSel = it.id
      s.lastAnswer = null
      return
    }
    case 'openTrack': {
      const order = queueOrder(s)
      const it = order.find((q) => q.id === s.queueSel) ?? order[0]
      if (!it) return
      const v = s.variants.open
      if (v === 'output') {
        const spans = s.runs.find((r) => r.id === it.runId)?.spans.filter((x) => x.start <= it.raisedWall && x.kind !== 'wait') ?? []
        s.detail = { runId: it.runId, tab: 'output', phase: null, focusCall: spans[spans.length - 1]?.id ?? null, follow: false, bd: { ...NO_FILTER } }
        return
      }
      s.returnTo = 'queue'
      s.surface = 'tracks'
      s.tracks.focus = { runId: it.runId, wall: it.raisedWall, itemId: it.id }
      if (v === 'moment') {
        s.tracks.follow = false
        s.tracks.span = 600
        s.tracks.start = it.raisedWall - 360
      } else {
        s.tracks.follow = true
        s.tracks.span = 3600
        s.tracks.start = s.simNow - 3600 * 0.7
      }
      return
    }
    case 'back':
      if (s.detail) s.detail = null
      else if (s.returnTo) {
        s.surface = s.returnTo
        s.returnTo = null
        s.tracks.focus = null
        s.tracks.follow = true
        s.tracks.span = 3600
      }
      return
    case 'zoom': {
      const centre = s.tracks.start + s.tracks.span / 2
      s.tracks.span = Math.max(300, Math.min(14400, a.dir > 0 ? s.tracks.span / 2 : s.tracks.span * 2))
      s.tracks.start = centre - s.tracks.span / 2
      s.tracks.follow = false
      return
    }
    case 'pan':
      s.tracks.start += (a.dir * s.tracks.span) / 4
      s.tracks.follow = false
      return
    case 'trackFollow':
      s.tracks.follow = a.on
      if (a.on) s.tracks.start = s.simNow - s.tracks.span * 0.7
      return
    case 'hover':
      s.hover = a.runId
      return
    case 'compareToggle': {
      const i = s.compareSel.indexOf(a.runId)
      if (i >= 0) s.compareSel.splice(i, 1)
      else {
        s.compareSel.push(a.runId)
        if (s.compareSel.length > 2) s.compareSel.shift()
      }
      return
    }
    case 'openCompare':
      if (s.compareSel.length === 2) {
        s.compare = { a: s.compareSel[0], b: s.compareSel[1], note: '', saved: null }
        s.surface = 'compare'
      }
      return
    case 'compareNote':
      if (s.compare) s.compare.note = a.text
      return
    case 'compareSave':
      if (s.compare && s.compare.note.trim()) {
        s.compare.saved = s.compare.note.trim()
        s.session.finding = s.compare.saved
        if (s.session.scenarioId === '6' && !/fix/i.test(s.compare.saved)) autoError(s, `finding names the wrong place: "${s.compare.saved}"`)
      }
      return
    case 'reviewDecide': {
      const run = s.runs.find((r) => r.id === a.runId)
      if (!run) return
      const it = s.queue.find((q) => q.runId === a.runId && q.kind === 'review' && q.status === 'pending')
      if (it) {
        resolveItem(s, it.id, 'ui', a.effect as Effect, true)
        s.session.decisions.push({ item: it.id, kind: 'review', key: 'ui', effect: a.effect as Effect, matched: true, t: s.realNow })
      } else run.review = a.effect === 'accept' ? 'accepted' : a.effect === 'reprompt' ? 'reprompted' : 'discarded'
      return
    }
    case 'expDraft':
      s.expDraft = { ...s.expDraft, ...a.patch }
      return
    case 'expStart': {
      const d = s.expDraft
      if (!d.taskId || !d.armA || !d.armB || d.armA === d.armB) return
      s.counter += 1
      const id = `X-${s.counter}`
      s.experiments.push({ id, taskId: d.taskId, armA: d.armA, armB: d.armB, startedWall: s.simNow, status: 'running', duration: 480 })
      s.expOpen = id
      return
    }
    case 'expOpen':
      s.expOpen = a.id
      return
    case 'expNote':
      s.expNote.text = a.text
      return
    case 'expNoteSave':
      if (s.expNote.text.trim()) {
        s.expNote.saved = s.expNote.text.trim()
        s.session.finding = s.expNote.saved
        if (s.session.scenarioId === '7' && !/lru/i.test(s.expNote.saved)) autoError(s, `finding names the wrong branch: "${s.expNote.saved}"`)
      }
      return
    case 'setLine':
      s.line = a.line
      return
    case 'setVariant':
      ;(s.variants as unknown as Record<string, string>)[a.key] = a.value
      return
    case 'missingNote':
      s.session.missing[a.itemId] = a.text
      return
    case 'loadScenario': {
      const sc = scenarioById(a.id)
      if (!sc) return
      const keep = { realNow: s.realNow, variants: s.variants, session: s.session }
      const fresh = sc.setup()
      Object.assign(s, fresh, { realNow: keep.realNow, variants: keep.variants })
      s.speed = sc.speed
      s.session = {
        ...keep.session, scenarioId: a.id, phase: 'loaded', startedAt: null, errors: 0, errorLog: [], autoFlags: [], notes: '',
        decisions: [], missing: {}, finding: null, detected: false,
      }
      return
    }
    case 'startTimer':
      if (s.session.scenarioId && s.session.phase === 'loaded') {
        s.session.phase = 'running'
        s.session.startedAt = s.realNow
        s.session.log.push({ t: s.realNow, a: 'start', d: s.session.scenarioId })
      }
      return
    case 'stopTimer':
      stop(s, 'manual', a.correct ?? null)
      return
    case 'addError':
      if (s.session.phase === 'running') {
        s.session.errors += 1
        s.session.errorLog.push({ t: s.realNow, note: a.note ?? '', auto: false })
      }
      return
    case 'setNotes':
      s.session.notes = a.text
      return
    case 'setParticipant':
      s.session.participant = a.name
      return
    case 'setSpeed':
      s.speed = a.speed
      return
    case 'setPaused':
      s.paused = a.paused
      return
    case 'setFacilitator':
      s.session.facilitator = a.on
      return
    case 'resetSession':
      s.session = emptySession(s.session.facilitator)
      return
  }
}

export const live = (s: State) => s.runs.filter(isLive)
