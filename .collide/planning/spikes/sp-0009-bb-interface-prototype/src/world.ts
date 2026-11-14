// Initial state, derived values and small state helpers shared by the reducer and the scenarios.
import { BRANCHES, EPICS, makeTasks } from './corpus'
import { makeRun, SIM_ZERO, type MakeRun } from './sim'
import type { Filter, Run, State, Task, TaskStatus, Variants, View } from './types'

export const EMPTY_FILTER: Filter = { epic: [], status: [], label: [], source: [], text: '' }
export const DEFAULT_VARIANTS: Variants = { identity: 'id', keys: 'literal', open: 'moment', line: 'top' }
export const T0 = SIM_ZERO + 3600 // 10:00:00 on the fake clock
export const WORKFLOWS = ['build-review', 'quick-fix']

export function emptySession(facilitator = false): State['session'] {
  return {
    facilitator, participant: '', scenarioId: null, phase: 'idle', startedAt: null, errors: 0, errorLog: [], autoFlags: [],
    notes: '', attempts: [], probes: [], probe: null, log: [], decisions: [], missing: {}, finding: null, detected: false,
  }
}

export function seedViews(): View[] {
  return [
    { id: 'v-1', name: 'All ready', cel: 'status == "ready"', filter: { ...EMPTY_FILTER, status: ['ready'] } },
    {
      id: 'v-2', name: 'Ready in E-03', cel: 'epic == "E-03" && status == "ready"',
      filter: { ...EMPTY_FILTER, epic: ['E-03'], status: ['ready'] },
    },
    { id: 'v-3', name: 'Flaky', cel: '"flaky" in labels', filter: { ...EMPTY_FILTER, label: ['flaky'] } },
  ]
}

export function initialState(repo: 'orchard' | 'empty' = 'orchard', facilitator = false): State {
  return {
    realNow: 0,
    simNow: T0,
    speed: 1,
    paused: false,
    repo,
    tasks: repo === 'orchard' ? makeTasks() : [],
    epics: repo === 'orchard' ? EPICS : [],
    branches: BRANCHES,
    lines: ['main', 'release-4'],
    line: 'main',
    runs: [],
    queue: [],
    counter: 0,
    views: repo === 'orchard' ? seedViews() : [],
    activeView: null,
    filter: { ...EMPTY_FILTER },
    saving: false,
    selected: [],
    launch: { open: false, mode: 'separate', workflow: WORKFLOWS[0] },
    surface: 'field',
    returnTo: null,
    detail: null,
    drawerTask: null,
    queueSel: null,
    ctxOpen: false,
    armed: null,
    lastAnswer: null,
    tracks: { start: T0 - 900, span: 3600, follow: true, focus: null },
    hover: null,
    compareSel: [],
    compare: null,
    experiments: [],
    expDraft: { taskId: '', armA: '', armB: '' },
    expOpen: null,
    expNote: { text: '', saved: null },
    promptText: '',
    packAdded: false,
    variants: { ...DEFAULT_VARIANTS },
    session: emptySession(facilitator),
  }
}

export const isLive = (r: Run): boolean => r.status !== 'done' && r.status !== 'stopped'

/** Add a run, giving it the lowest free lane letter. Mutates the (draft) state. */
export function addRun(s: State, m: Omit<MakeRun, 'seq'>): Run {
  const run = makeRun({ ...m, seq: s.runs.length + 1 })
  const used = new Set(s.runs.filter(isLive).map((r) => r.lane))
  run.lane = 'ABCDEFGHIJKL'.split('').find((l) => !used.has(l)) ?? '?'
  s.runs.push(run)
  return run
}

export function statusOf(s: State, t: Task): TaskStatus {
  const runs = s.runs.filter((r) => r.taskId === t.id)
  if (runs.some(isLive)) return 'running'
  const last = runs[runs.length - 1]
  if (last && last.status === 'done') {
    if (last.review === 'pending') return 'review'
    if (last.review === 'accepted') return 'done'
  }
  if (s.line === 'main') return t.status
  return t.release4 ?? t.status
}

export function taskOf(s: State, id: string | null): Task | undefined {
  return id ? s.tasks.find((t) => t.id === id) : undefined
}

export function runOf(s: State, id: string | null | undefined): Run | undefined {
  return id ? s.runs.find((r) => r.id === id) : undefined
}

export function matches(s: State, t: Task, f: Filter): boolean {
  if (f.epic.length && !f.epic.includes(t.epic)) return false
  if (f.status.length && !f.status.includes(statusOf(s, t))) return false
  if (f.label.length && !f.label.some((l) => t.labels.includes(l))) return false
  if (f.source.length && !f.source.includes(t.source)) return false
  const q = f.text.trim().toLowerCase()
  if (q && !(t.id.toLowerCase().includes(q) || t.title.toLowerCase().includes(q))) return false
  return true
}

export function visibleTasks(s: State): Task[] {
  return s.tasks.filter((t) => matches(s, t, s.filter))
}

const q = (x: string) => `"${x}"`
const list = (xs: string[]) => `[${xs.map(q).join(', ')}]`

export function celOf(f: Filter): string {
  const parts: string[] = []
  if (f.epic.length) parts.push(f.epic.length === 1 ? `epic == ${q(f.epic[0])}` : `epic in ${list(f.epic)}`)
  if (f.status.length) parts.push(f.status.length === 1 ? `status == ${q(f.status[0])}` : `status in ${list(f.status)}`)
  if (f.label.length) {
    parts.push(f.label.length === 1 ? `${q(f.label[0])} in labels` : `labels.exists(l, l in ${list(f.label)})`)
  }
  if (f.source.length) parts.push(f.source.length === 1 ? `source == ${q(f.source[0])}` : `source in ${list(f.source)}`)
  if (f.text.trim()) parts.push(`(id.contains(${q(f.text.trim())}) || title.contains(${q(f.text.trim())}))`)
  return parts.length ? parts.join(' && ') : 'true'
}

export function sameSet(a: string[], b: string[]): boolean {
  return a.length === b.length && a.every((x) => b.includes(x))
}

export const livePos = (s: State, r: Run): number =>
  1 + s.runs.filter(isLive).sort((a, b) => a.seq - b.seq).findIndex((x) => x.id === r.id)

export const pendingItems = (s: State) => s.queue.filter((q) => q.status === 'pending')

export const fmtClock = (wall: number): string => {
  const t = Math.max(0, Math.floor(wall))
  const h = Math.floor(t / 3600) % 24
  const m = Math.floor((t % 3600) / 60)
  const sec = t % 60
  return `${String(h).padStart(2, '0')}:${String(m).padStart(2, '0')}:${String(sec).padStart(2, '0')}`
}
export const fmtDur = (sec: number): string => {
  if (sec < 10) return `${sec.toFixed(1)} s`
  if (sec < 120) return `${Math.round(sec)} s`
  return `${Math.floor(sec / 60)} min ${Math.round(sec % 60)} s`
}
export const parseClock = (txt: string): number | null => {
  const m = txt.trim().match(/^(\d{1,2}):(\d{2})(?::(\d{2}))?$/)
  if (!m) return null
  return Number(m[1]) * 3600 + Number(m[2]) * 60 + Number(m[3] ?? 0)
}
