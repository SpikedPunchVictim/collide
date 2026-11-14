// The nine S6 scenarios and three probes: a defined fake start state, a participant card, and an end-state detector.
import { filterSpans, slowest, NO_FILTER } from './breakdown'
import { FULL_WORKFLOW, localAt, SHORT_WORKFLOW, type GenOpts } from './gen'
import { advanceAll } from './sim'
import { addRun, fmtClock, initialState, matches, parseClock, sameSet, T0 } from './world'
import type { Ctx, Effect, ItemSpec, State } from './types'

export interface Scenario {
  id: string
  kind: 'scenario' | 'probe'
  title: string
  card: (s: State) => string
  /** sim speed the session starts at */
  speed: number
  setup: () => State
  /** the objective end state; undefined means the facilitator stops it */
  detect?: (s: State) => boolean
}

const spec = (kind: ItemSpec['kind'], title: string, body: string, why: string, ctx: Ctx, expect: Effect[], cwd = 'work'): ItemSpec =>
  ({ kind, title, body, why, ctx, expect, cwd })

interface RaiseRun {
  id: string
  taskId: string
  title: string
  phase: string
  frac: number
  /** seconds the item has been waiting */
  age: number
  item?: ItemSpec
  gen?: Partial<GenOpts>
  spend?: number
  budget?: number
}

/** A live run that blocked `age` seconds ago on an item. */
function raiseRun(s: State, r: RaiseRun): void {
  const at = localAt(FULL_WORKFLOW, 1, r.phase, r.frac)
  const run = addRun(s, {
    id: r.id, taskId: r.taskId, title: r.title, startWall: s.simNow - at - r.age, budget: r.budget,
    gen: { seed: r.id.split('').reduce((a, c) => a * 31 + c.charCodeAt(0), 7) % 100000, raises: r.item ? [{ phase: r.phase, frac: r.frac, spec: r.item }] : [], ...r.gen },
  })
  advanceAll(s, s.simNow)
  if (r.spend !== undefined) run.spend = r.spend
}

function finishedRun(s: State, id: string, taskId: string, title: string, endedAgo: number, seed: number, gen: Partial<GenOpts> = {}, review: 'pending' | 'accepted' = 'pending'): void {
  const probe = addRun(s, { id, taskId, title, startWall: 0, review, gen: { seed, ...gen } })
  const total = probe.script[probe.script.length - 1].at
  probe.startWall = s.simNow - endedAgo - total
  advanceAll(s, s.simNow)
}

// ---- 1. fresh repo
const S1: Scenario = {
  id: '1', kind: 'scenario', title: 'Fresh repo to a reviewed run', speed: 30,
  card: () => 'This repository is new: it has no tasks and nothing set up. You want a small change made: add a `--quiet` flag to the command line tool. Get an agent to make that change, then review what it produced.',
  setup: () => initialState('empty'),
  detect: (s) => s.runs.some((r) => r.taskId?.startsWith('S-') && r.status === 'done' && ['accepted', 'reprompted', 'discarded'].includes(r.review)),
}

// ---- 2. build a view
export const S2_TARGET = ['T-07-001', 'T-07-004', 'T-07-007', 'T-07-010', 'T-07-013']
const S2: Scenario = {
  id: '2', kind: 'scenario', title: 'Build a view and save it', speed: 1,
  card: () => 'Show only the tasks in epic E-07 that are ready and carry the label perf. Keep that selection under the name Slow spots, so you can come back to it later.',
  setup: () => initialState('orchard'),
  detect: (s) => s.views.some((v) => /^slow spots$/i.test(v.name.trim()) && sameSet(s.tasks.filter((t) => matches(s, t, v.filter)).map((t) => t.id), S2_TARGET)),
}

// ---- 3. launch a group
export const S3_TARGET = ['T-03-011', 'T-03-014', 'T-03-019']
const S3: Scenario = {
  id: '3', kind: 'scenario', title: 'Launch a group of three from a view', speed: 30,
  card: () => 'The team keeps a saved selection called Ready in E-03. From it, launch tasks T-03-011, T-03-014 and T-03-019 together as one group, and nothing else.',
  setup: () => initialState('orchard'),
  detect: (s) => {
    const groups = new Set(s.runs.filter((r) => r.groupId).map((r) => r.groupId))
    return [...groups].some((g) => {
      const ids = s.runs.filter((r) => r.groupId === g).map((r) => r.taskId ?? '')
      return sameSet(ids, S3_TARGET) && s.runs.length === 3
    })
  },
}

// ---- 4. clear five decisions
export const S4_RUNS = ['R-401', 'R-402', 'R-403', 'R-404', 'R-405']
function setup4(): State {
  const s = initialState('orchard')
  raiseRun(s, {
    id: 'R-401', taskId: 'T-02-009', title: 'Add priority lanes to the job queue', phase: 'build', frac: 0.3, age: 840,
    item: spec('permission', 'pnpm add -D msw@2.4.9', 'pnpm add -D msw@2.4.9', 'No allow rule matches `pnpm add`, so Collide asks.', {
      why: ['rule: Bash(pnpm add *) is not allowed or denied, so the default is ask'],
      does: ['installs a dev dependency into work/R-401', 'network: registry.npmjs.org', 'edits package.json and pnpm-lock.yaml'],
      task: ['T-02-009 Add priority lanes to the job queue', 'phase build, single pass'],
      led: ['wrote tests/queue_lanes.rs', 'cargo test -p queue: 2 failed'],
      history: ['same request this repo: 0 approved, 0 denied', 'pnpm add this repo: 5 approved, 0 denied'],
      words: ['"The lane tests need a mock server." (unverified)'],
    }, ['allow', 'deny'], 'work/R-401'),
  })
  raiseRun(s, {
    id: 'R-402', taskId: 'T-06-005', title: 'Env overrides ignore empty values', phase: 'build', frac: 0.5, age: 540,
    item: spec('edit', 'Edit src/config/env.rs', 'src/config/env.rs', 'Edits outside the task\'s listed paths ask first.', {
      why: ['rule: Edit(src/config/**) asks in phase build'],
      does: ['replaces 4 lines in src/config/env.rs, +6 -2'],
      diff: ['-    let v = std::env::var(name).ok();', '+    let v = std::env::var(name).ok().filter(|x| !x.is_empty());'],
      task: ['T-06-005 Env overrides ignore empty values'],
      led: ['read src/config/env.rs and tests/config_env.rs'],
      history: ['same request this run: 0'],
      words: ['"Empty values should fall back to the file default." (unverified)'],
    }, ['apply'], 'work/R-402'),
  })
  raiseRun(s, {
    id: 'R-403', taskId: 'T-01-008', title: 'Fix dedupe pass on empty batches', phase: 'build', frac: 1, age: 360,
    gen: {
      buildGate: 'unverified',
      gateSpec: spec('gate', 'typecheck unverified in R-403', 'tsc --noEmit', 'The typecheck gate could not run, so Collide cannot call it a pass.', {
        why: ['gate typecheck read exit 127', 'stderr: sh: tsc: command not found'],
        does: ['retry runs `tsc --noEmit` again in work/R-403', 'proceed moves to brief without a verdict'],
        task: ['T-01-008 Fix dedupe pass on empty batches'],
        led: ['last call: pnpm install --frozen-lockfile, exit 0', '3 files changed so far'],
      }, ['retry', 'hold', 'stop'], 'work/R-403'),
    },
  })
  raiseRun(s, {
    id: 'R-404', taskId: 'T-05-004', title: 'Summary table drops the last row', phase: 'build', frac: 0.7, age: 180, budget: 6, spend: 6.4,
    item: spec('budget', 'Phase build is over its $6.00 budget', 'build: $6.40 of $6.00', 'The phase has spent more than its budget.', {
      why: ['phase budget $6.00, spent $6.40', 'run budget $12.00, spent $6.40'],
      does: ['raise lets the phase run on; stop ends the run and keeps its work'],
      task: ['T-05-004 Summary table drops the last row'],
      led: ['41 tool calls, 3 test runs, 2 failing'],
      history: ['builds in this repo cost $3.10 on median'],
    }, ['raise', 'continue', 'pause', 'stop'], 'work/R-404'),
  })
  finishedRun(s, 'R-405', 'T-08-003', 'Add a lint rule for stale TODOs', 60, 405, { phases: SHORT_WORKFLOW })
  return s
}
const S4: Scenario = {
  id: '4', kind: 'scenario', title: 'Clear five pending decisions', speed: 1,
  card: () => 'Five things are waiting for an answer from you. Deal with all of them, as you would on your own project.',
  setup: setup4,
  detect: (s) => S4_RUNS.every((rid) => {
    const items = s.queue.filter((q) => q.runId === rid)
    return items.length > 0 && items.every((q) => q.status === 'resolved')
  }),
}

// ---- 5. review and accept
function setup5(): State {
  const s = initialState('orchard')
  finishedRun(s, 'R-501', 'T-05-009', 'Add column picker to the export job', 420, 501)
  finishedRun(s, 'R-502', 'T-08-003', 'Add a lint rule for stale TODOs', 900, 502)
  return s
}
const S5: Scenario = {
  id: '5', kind: 'scenario', title: 'Review a finished run and accept it', speed: 1,
  card: () => 'The run for task T-05-009 has finished. Look at what it did, and accept it.',
  setup: setup5,
  detect: (s) => s.runs.some((r) => r.taskId === 'T-05-009' && r.review === 'accepted'),
}

// ---- 6. compare two runs
function setup6(): State {
  const s = initialState('orchard')
  finishedRun(s, 'R-601', 'T-04-003', 'Warmup job for the shard map', 3600, 601, {}, 'accepted')
  finishedRun(s, 'R-602', 'T-04-003', 'Warmup job for the shard map', 600, 602, {
    waits: [
      { phase: 'fix', frac: 0.15, dur: 480, label: 'Bash pnpm add -D tiny-backoff' },
      { phase: 'fix', frac: 0.6, dur: 600, label: 'Edit Cargo.toml' },
    ],
  }, 'accepted')
  finishedRun(s, 'R-603', 'T-04-004', 'Stale reads after eviction', 5400, 603, {}, 'accepted')
  finishedRun(s, 'R-604', 'T-07-003', 'Cut the hot loop allocation', 7200, 604, {}, 'accepted')
  return s
}
const S6: Scenario = {
  id: '6', kind: 'scenario', title: 'Compare two runs and say where the time went', speed: 1,
  card: () => 'Task T-04-003 was run twice and both runs finished. The second took much longer than the first. Compare the two runs and write down, in one line, where the extra time went.',
  setup: setup6,
  detect: (s) => !!s.compare?.saved && /fix/i.test(s.compare.saved) && [s.compare.a, s.compare.b].sort().join() === 'R-601,R-602',
}

// ---- 7. experiment
const S7: Scenario = {
  id: '7', kind: 'scenario', title: 'A/B experiment from two branches', speed: 30,
  card: () => 'On task T-04-012, set up a test of branch exp/ttl-cache against branch exp/lru-cache. Run it, read the result, and write down which branch won and by how much.',
  setup: () => initialState('orchard'),
  detect: (s) => {
    const e = s.experiments.find((x) => x.taskId === 'T-04-012' && [x.armA, x.armB].sort().join() === 'exp/lru-cache,exp/ttl-cache')
    return !!e && e.status === 'complete' && !!s.expNote.saved && /lru/i.test(s.expNote.saved)
  },
}

// ---- 8. live output and tool calls
export const S8_RUN = 'R-801'
const S8_AGO = 1390
function s8Window(): [number, number] {
  const start = T0 - S8_AGO + 180 // build starts after the 180 s provision phase
  const a = Math.ceil((start + 120) / 60) * 60
  return [a, a + 420]
}
function setup8(): State {
  const s = initialState('orchard')
  const run = addRun(s, { id: S8_RUN, taskId: 'T-07-003', title: 'Cut the hot loop allocation', startWall: T0 - S8_AGO, gen: { seed: 808 } })
  advanceAll(s, T0)
  plant8(run)
  return s
}
/** Make the data fit the task: one clear slowest call in the window, a near miss, and a slower decoy outside. */
function plant8(run: State['runs'][number]): void {
  const [w0, w1] = s8Window()
  const inWin = (x: { start: number }) => x.start > w0 && x.start < w1
  // no subagents inside the window: turn them into short greps and drop their children
  const subs = run.spans.filter((x) => x.phase === 'build' && x.kind === 'subagent' && inWin(x))
  for (const sa of subs) {
    sa.kind = 'tool'; sa.tool = 'Grep'; sa.input = '"retry" src/'; sa.end = sa.start + 0.4; sa.sid = undefined
  }
  run.spans = run.spans.filter((x) => !(x.parent && subs.some((sa) => sa.id === x.parent)))
  const build = run.spans.filter((x) => x.phase === 'build' && x.kind === 'tool' && !x.parent)
  for (const x of build) if (inWin(x) && x.end! - x.start > 5) x.end = x.start + 4.9
  const plant = (x: (typeof build)[number] | undefined, exe: string, input: string, dur: number) => {
    if (!x) return
    x.tool = 'Bash'; x.exe = exe; x.input = input; x.exit = 0; x.error = false; x.end = x.start + dur
  }
  const inside = build.filter((x) => x.start > w0 + 10 && x.start < w1 - 30 && x.end! - x.start < 1.5)
  const n = inside.length
  plant(inside[Math.floor(n * 0.15)], 'cargo', 'cargo test -p importer', 7.3)
  plant(inside[Math.floor(n * 0.4)], 'pnpm', 'pnpm lint', 1.95)
  plant(inside[Math.floor(n * 0.6)], 'pnpm', 'pnpm test --run', 2.4)
  plant(inside[Math.floor(n * 0.8)], 'cargo', 'cargo clippy', 3.1)
  const outside = build.find((x) => x.start > w1 + 20 && x.end! - x.start < 3)
  plant(outside, 'cargo', 'cargo build --release', 14.2)
}
export function s8Target(s: State): { id: string; win: [number, number] } | null {
  const run = s.runs.find((r) => r.id === S8_RUN)
  if (!run) return null
  const win = s8Window()
  const hit = slowest(filterSpans(run, { ...NO_FILTER, phase: 'build', minDur: 2, win }, s.simNow), s.simNow)
  return hit ? { id: hit.id, win } : null
}
const S8: Scenario = {
  id: '8', kind: 'scenario', title: 'Live output, then slow tool calls', speed: 1,
  card: () => {
    const [a, b] = s8Window()
    return `A run is working on task T-07-003 right now. Watch what it is doing. Then, in its build phase, find the tool calls that took longer than 2 seconds between ${fmtClock(a)} and ${fmtClock(b)}, and open the output at the slowest of them.`
  },
  setup: setup8,
  detect: (s) => {
    const d = s.detail
    const t = s8Target(s)
    if (!d || !t || d.runId !== S8_RUN || d.focusCall !== t.id || d.tab !== 'output') return false
    const w = d.bd.win
    return d.bd.minDur >= 2 && d.bd.minDur <= 2.5 && !!w && Math.abs(w[0] - t.win[0]) <= 20 && Math.abs(w[1] - t.win[1]) <= 20
  },
}

// ---- 9. decide from the context view
export const S9_RUNS = ['R-901', 'R-902', 'R-903']
function setup9(): State {
  const s = initialState('orchard')
  raiseRun(s, {
    id: 'R-901', taskId: 'T-03-004', title: 'Import log loses the first line', phase: 'build', frac: 0.4, age: 420,
    item: spec('command', 'rm -rf node_modules ../shared/build', 'rm -rf node_modules ../shared/build && pnpm install --frozen-lockfile', 'A hook asked: this command deletes outside the worktree.', {
      why: ['hook guard/no-delete: "deletes a path outside the worktree"', 'rule: Bash(rm *) asks'],
      does: ['working directory: work/R-901', 'deletes: node_modules, ../shared/build', 'installs: pnpm install --frozen-lockfile', 'network: registry fetch'],
      flags: ['../shared/build is outside the worktree'],
      task: ['T-03-004 Import log loses the first line', 'phase build, single pass'],
      led: ['cargo test -p importer: 1 failed, exit 101', '2 files changed so far'],
      history: ['rm in this repo: 4 approved, 1 denied', 'same request this run: 0'],
      words: ['"Clean install to rule out stale modules." (unverified)'],
    }, ['deny', 'reject'], 'work/R-901'),
  })
  raiseRun(s, {
    id: 'R-902', taskId: 'T-02-006', title: 'Retry timer drifts after sleep', phase: 'build', frac: 0.55, age: 300,
    item: spec('edit', 'Edit src/queue/retry.rs', 'src/queue/retry.rs', 'Edits to files outside the task\'s paths ask first.', {
      why: ['rule: Edit(src/queue/**) asks in phase build'],
      does: ['replaces 5 lines in src/queue/retry.rs, +7 -3'],
      diff: ['-    let wait = self.base * 2u32.pow(n);', '+    let wait = (self.base * 2u32.pow(n)).min(self.cap);'],
      task: ['T-02-006 Retry timer drifts after sleep', 'checklist: cap the wait; test sleep gaps'],
      led: ['read src/queue/retry.rs', 'wrote tests/queue_retry.rs, 1 failing'],
      words: ['"Cap the wait so a long sleep cannot overflow it." (unverified)'],
    }, ['apply'], 'work/R-902'),
  })
  raiseRun(s, {
    id: 'R-903', taskId: 'T-01-005', title: 'Add a checksum step to intake', phase: 'build', frac: 1, age: 200,
    gen: {
      buildGate: 'unverified',
      gateSpec: spec('gate', 'typecheck unverified in R-903', 'tsc --noEmit', 'The typecheck gate could not run, so Collide cannot call it a pass.', {
        why: ['gate typecheck read exit 127', 'stderr: sh: tsc: command not found'],
        task: ['T-01-005 Add a checksum step to intake'],
        led: ['3 files changed so far'],
      }, ['retry', 'hold', 'stop'], 'work/R-903'),
    },
  })
  return s
}
const S9: Scenario = {
  id: '9', kind: 'scenario', title: 'Decide three items from the context view', speed: 1,
  card: () => 'Three requests are waiting: a command, an edit and a failed gate. Decide each one using only the explanation the app gives for why it is being asked, without opening the run\'s own output. After each decision, say what you wanted to know and could not find, or that nothing was missing.',
  setup: setup9,
  detect: (s) => S9_RUNS.every((rid) => {
    const items = s.queue.filter((q) => q.runId === rid)
    return items.length > 0 && items.every((q) => q.status === 'resolved' && s.session.missing[q.id] !== undefined)
  }),
}

// ---- probes: they isolate the open questions that the nine scenarios only touch
export const P_TASKS = ['T-07-003', 'T-07-001', 'T-07-004', 'T-07-007']
function probeBase(): State {
  const s = initialState('orchard')
  const ages = [1200, 900, 600, 300]
  P_TASKS.forEach((t, i) => {
    const task = s.tasks.find((x) => x.id === t)!
    const item = i === 1
      ? spec('permission', 'cargo add tiny-backoff', 'cargo add tiny-backoff', 'No allow rule matches `cargo add`, so Collide asks.', {
        why: ['rule: Bash(cargo add *) asks'], does: ['edits Cargo.toml and Cargo.lock'], task: [task.title],
        led: ['cargo test -p queue, exit 101', 'cargo test -p queue, exit 101', 'Edit tests/queue_retry.rs'],
      }, ['allow', 'deny'], 'work/R-702')
      : undefined
    raiseRun(s, { id: `R-70${i + 1}`, taskId: t, title: task.title, phase: 'build', frac: 0.5, age: i === 1 ? 120 : 0, item })
    void ages
  })
  return s
}
const P1: Scenario = {
  id: 'P1', kind: 'probe', title: 'Probe: switch line', speed: 1,
  card: () => 'The team also works on a second line, release-4. Switch to it.',
  setup: probeBase,
  detect: (s) => s.line === 'release-4',
}
const P2: Scenario = {
  id: 'P2', kind: 'probe', title: 'Probe: find a task\'s run', speed: 1,
  card: () => 'Task T-07-003 is being worked on right now. Open the track of the run that is working on it.',
  setup: probeBase,
  detect: (s) => s.surface === 'tracks' && s.detail?.runId === 'R-701',
}
const P3: Scenario = {
  id: 'P3', kind: 'probe', title: 'Probe: what led to the request', speed: 1,
  card: () => 'A permission request is waiting. Find out what the agent was doing in the two minutes before it asked, and tell me.',
  setup: probeBase,
}

export const SCENARIOS: Scenario[] = [S1, S2, S3, S4, S5, S6, S7, S8, S9, P1, P2, P3]
export const scenarioById = (id: string): Scenario | undefined => SCENARIOS.find((x) => x.id === id)
export { parseClock }
