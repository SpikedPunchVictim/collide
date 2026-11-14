// Shared types for the S6 prototype. All data is fake.

export type TaskStatus = 'backlog' | 'ready' | 'blocked' | 'running' | 'review' | 'done'

export interface Task {
  id: string
  title: string
  epic: string
  status: TaskStatus
  /** status on the second line, when it differs from main */
  release4?: TaskStatus
  labels: string[]
  source: 'tasks/' | 'github'
  dependsOn: string[]
  body: string
}

export interface Epic {
  id: string
  title: string
}

export type RunStatus = 'working' | 'waiting' | 'failed' | 'paused' | 'done' | 'stopped'

export type SpanKind = 'tool' | 'hook' | 'subagent' | 'wait' | 'text' | 'provision'

export interface Span {
  id: string
  runId: string
  /** phase key, e.g. build or review:A */
  phase: string
  kind: SpanKind
  /** display name: Read, Bash, Task, mcp:docs, hook, permission wait */
  tool: string
  /** for Bash: the executable (cargo, pnpm, git) */
  exe?: string
  input: string
  start: number
  /** null while a wait is open */
  end: number | null
  exit?: number
  error?: boolean
  parent?: string
  /** id children use as parent */
  sid?: string
  /** queue item this span belongs to (permission waits) */
  itemId?: string
  /** exact (telemetry) or approximate (stream timing) */
  timing?: 'exact' | 'approx'
}

export interface Phase {
  key: string
  label: string
  reviewer?: 'A' | 'B'
  start: number | null
  end: number | null
}

export interface Gate {
  at: number
  phase: string
  name: string
  verdict: 'pass' | 'fail' | 'unverified'
  note: string
}

export interface Marker {
  at: number
  kind: 'finding'
  text: string
}

export interface DiffFile {
  path: string
  add: number
  del: number
  lines: string[]
  /** a lock or package manifest: always shown */
  dep?: boolean
}

export interface Finding {
  reviewer: 'A' | 'B'
  text: string
  adjudicated: 'accepted' | 'rejected'
}

export type ItemKind = 'permission' | 'command' | 'edit' | 'gate' | 'review' | 'budget'

export type Effect =
  | 'allow' | 'deny' | 'apply' | 'reject'
  | 'proceed' | 'retry' | 'stop' | 'hold'
  | 'accept' | 'discard' | 'reprompt'
  | 'raise' | 'continue' | 'pause'

export interface ItemSpec {
  kind: ItemKind
  title: string
  /** exact command, target path, or gate command */
  body: string
  cwd: string
  /** one line, why Collide asks */
  why: string
  /** facts for the context view */
  ctx: Ctx
  /** effects that count as a sensible answer (advisory, for the log) */
  expect: Effect[]
  warm?: { minutes: number; resume: number }
}

export interface Ctx {
  /** 1: rule, hook reason, failed check with values, budget */
  why: string[]
  /** 2: what it would do; undefined means the context view cannot say */
  does?: string[]
  /** 3: the task */
  task?: string[]
  /** 4: what led here */
  led?: string[]
  /** 5: history */
  history?: string[]
  /** 6: the agent's words, unverified */
  words?: string[]
  /** flagged parts of a command (untrusted arguments, outside paths) */
  flags?: string[]
  diff?: string[]
}

export interface QueueItem extends ItemSpec {
  id: string
  runId: string
  phase: string
  raisedWall: number
  status: 'pending' | 'resolved'
  answer?: { key: string; effect: Effect; wall: number; matched: boolean }
}

export interface ScriptSpan {
  kind: SpanKind
  tool: string
  exe?: string
  input: string
  dur: number
  phase: string
  exit?: number
  error?: boolean
  parent?: string
  /** id used by children as parent */
  sid?: string
}

export type ScriptItem =
  | { at: number; t: 'phase'; key: string; label: string; reviewer?: 'A' | 'B' }
  | { at: number; t: 'phaseEnd'; key: string }
  | { at: number; t: 'span'; span: ScriptSpan }
  | { at: number; t: 'raise'; spec: ItemSpec; phase: string }
  | { at: number; t: 'gate'; phase: string; name: string; verdict: Gate['verdict']; note: string; spec?: ItemSpec }
  | { at: number; t: 'marker'; text: string }
  | { at: number; t: 'end' }

export interface Run {
  id: string
  taskId: string | null
  title: string
  line: string
  groupId?: string
  status: RunStatus
  startWall: number
  endWall: number | null
  budget: number
  spend: number
  phases: Phase[]
  spans: Span[]
  gates: Gate[]
  markers: Marker[]
  script: ScriptItem[]
  idx: number
  shift: number
  blockedBy: string | null
  raisedWall: number | null
  review: 'none' | 'pending' | 'accepted' | 'reprompted' | 'discarded'
  diff: DiffFile[]
  report: string
  findings: Finding[]
  /** order in which the run appeared, for the lane and position identities */
  seq: number
  lane: string
  /** name of the workflow, shown in Launch */
  workflow: string
}

export interface View {
  id: string
  name: string
  cel: string
  filter: Filter
  /** absent means the Field, as before the in-app agent */
  surface?: 'field' | 'tracks'
  sort?: SortSpec | null
  groupBy?: GroupBy
  runFilter?: RunFilter
}

export interface Filter {
  epic: string[]
  status: TaskStatus[]
  label: string[]
  source: string[]
  text: string
  /** exact task ids; set by the in-app agent, not by a chip */
  ids?: string[]
}

export interface Branch {
  name: string
  mergeBase: string
  stat: { path: string; add: number; del: number; protectedPath?: boolean }[]
}

export interface Experiment {
  id: string
  taskId: string
  armA: string
  armB: string
  startedWall: number
  status: 'running' | 'complete'
  /** number of seconds of sim time until the result arrives */
  duration: number
}

export type IdentityVariant = 'id' | 'lane' | 'position' | 'colour'
export type KeysVariant = 'literal' | 'safe' | 'confirm'
export type OpenVariant = 'moment' | 'track' | 'output'
export type LineVariant = 'top' | 'filter'

export interface Variants {
  identity: IdentityVariant
  keys: KeysVariant
  open: OpenVariant
  line: LineVariant
}

export interface BreakdownFilter {
  tool: string | null
  minDur: number
  win: [number, number] | null
  phase: string | null
}

export interface LogEntry {
  t: number
  a: string
  d?: string
}

export interface ErrorEntry {
  t: number
  note: string
  auto: boolean
}

export interface Attempt {
  scenario: string
  title: string
  variants: Variants
  startedAt: number
  endedAt: number
  ms: number
  errors: number
  errorLog: ErrorEntry[]
  autoFlags: string[]
  notes: string
  stop: 'detected' | 'manual'
  /** probes stopped by hand: the facilitator's call */
  correct?: boolean | null
  detected: boolean
  decisions: { item: string; kind: ItemKind; key: string; effect: Effect; matched: boolean; t: number }[]
  missing: Record<string, string>
  finding?: string
  log: LogEntry[]
}

export interface Probe {
  id: string
  label: string
  ms: number
  correct: boolean | null
}

export interface Session {
  facilitator: boolean
  participant: string
  scenarioId: string | null
  phase: 'idle' | 'loaded' | 'running' | 'done'
  startedAt: number | null
  errors: number
  errorLog: ErrorEntry[]
  autoFlags: string[]
  notes: string
  attempts: Attempt[]
  probes: Probe[]
  probe: { id: string; startedAt: number } | null
  log: LogEntry[]
  decisions: Attempt['decisions']
  missing: Record<string, string>
  finding: string | null
  detected: boolean
}

export type Surface = 'field' | 'tracks' | 'queue' | 'experiments' | 'compare' | 'settings'
export type DetailTab = 'output' | 'breakdown' | 'review'

export interface Detail {
  runId: string
  tab: DetailTab
  phase: string | null
  focusCall: string | null
  follow: boolean
  bd: BreakdownFilter
}

export interface State {
  realNow: number
  simNow: number
  speed: number
  paused: boolean
  repo: 'orchard' | 'empty'
  tasks: Task[]
  epics: Epic[]
  branches: Branch[]
  lines: string[]
  line: string
  runs: Run[]
  queue: QueueItem[]
  counter: number
  views: View[]
  activeView: string | null
  filter: Filter
  saving: boolean
  selected: string[]
  launch: { open: boolean; mode: 'separate' | 'group'; workflow: string }
  surface: Surface
  returnTo: Surface | null
  detail: Detail | null
  drawerTask: string | null
  queueSel: string | null
  ctxOpen: boolean
  armed: { itemId: string; key: string } | null
  lastAnswer: { itemId: string; run: Run; at: number } | null
  tracks: { start: number; span: number; follow: boolean; focus: { runId: string; wall: number; itemId: string } | null }
  hover: string | null
  compareSel: string[]
  compare: { a: string; b: string; note: string; saved: string | null } | null
  experiments: Experiment[]
  expDraft: { taskId: string; armA: string; armB: string }
  expOpen: string | null
  expNote: { text: string; saved: string | null }
  promptText: string
  packAdded: boolean
  variants: Variants
  session: Session
  fieldView: FieldView
  tracksView: TracksView
  lens: Lens | null
  highlight: Highlight | null
  snapshots: Lens[]
  agent: AgentState
  agentSettings: AgentSettingsState
}

// ---- in-app agent (UI tools) ----

export interface SortSpec {
  /** Field: id, title, epic, labels, status. Tracks: total, start, end, spend, wait, phase:<key> */
  by: string
  dir: 'asc' | 'desc'
}
export type GroupBy = 'status' | 'epic' | 'label' | 'source'
export interface RunFilter {
  status: RunStatus[]
  task: string[]
  workflow: string[]
  ids: string[]
}
export interface FieldView {
  sort: SortSpec | null
  groupBy: GroupBy
}
export interface TracksView {
  filter: RunFilter
  sort: SortSpec | null
}

export interface LensGroup {
  name: string
  ids: string[]
  reason: string
}
/** The agent's judgement: named groups of ids with a reason each. A snapshot, stamped with its time. */
export interface Lens {
  id: string
  surface: 'field' | 'tracks'
  title: string
  groups: LensGroup[]
  rest: 'dim' | 'hide'
  /** real clock, ms since epoch, when the agent made it */
  at: number
  source: string
  /** set once the user saves it as a snapshot */
  name?: string
}
export interface Highlight {
  surface: 'field' | 'tracks'
  ids: string[]
  note: string
}

/** What Undo puts back. The agent cannot change runs, the queue or tasks, so only view state is kept. */
export interface Snap {
  surface: Surface
  filter: Filter
  activeView: string | null
  fieldView: FieldView
  tracksView: TracksView
  tracks: State['tracks']
  lens: Lens | null
  highlight: Highlight | null
  drawerTask: string | null
  detail: Detail | null
  queueSel: string | null
  expOpen: string | null
}

export type AgentTool = 'set_view' | 'show_groups' | 'highlight' | 'open'

export interface AgentChange {
  id: string
  tool: AgentTool
  surface: string
  title: string
  /** the expression for a view; empty for a lens */
  expr: string
  /** a lens: one line per group; a highlight or view: the reason, if given */
  reasons: { name: string; reason: string; n: number }[]
  note: string
  source: string
  at: number
  prev: Snap
  /** a view change carries the view it produced, so Save as view does not depend on later edits */
  view: Omit<View, 'id' | 'name'> | null
  lensId: string | null
  saved: string | null
}

export interface AskState {
  status: 'idle' | 'working' | 'done' | 'failed' | 'nochange'
  request: string
  agent: string
  startedAt: number
  firstToolAt: number | null
  chipAt: number | null
  doneAt: number | null
  lines: { t: number; text: string }[]
  text: string
  error: string
  usage: { costUsd: number | null; inputTokens: number | null; outputTokens: number | null } | null
  calls: number
}
export interface AgentState {
  changes: AgentChange[]
  n: number
  ask: AskState
}

export type AgentKind = 'claude-code' | 'acp' | 'external'
export interface AgentConfig {
  id: string
  name: string
  kind: AgentKind
  /** claude-code: the executable (claude). acp: the command to run. external: unused */
  command: string
  /** acp: extra arguments, space separated */
  args: string
  model: string
  /** the folder the agent may read; also its working directory */
  folder: string
  budgetUsd: number
  timeoutSec: number
  /** acp: KEY=VALUE per line, added to a minimal environment */
  env: string
}
export interface AgentSettings {
  agents: AgentConfig[]
  askAgent: string | null
}
export interface AgentSettingsState {
  settings: AgentSettings | null
  loaded: boolean
  dirty: boolean
  error: string | null
  savedAt: number | null
}
