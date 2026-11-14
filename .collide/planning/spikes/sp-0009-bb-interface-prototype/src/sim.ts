// Run simulation: a run's script plays against the sim clock. A raise blocks the run until the user answers.
import { genDiff, genFindings, genReport, genScript, type GenOpts, type PhaseSpec } from './gen'
import { reviewSpec } from './items'
import type { Effect, Phase, QueueItem, Run, State } from './types'

export const SIM_ZERO = 9 * 3600 // sim second 0 is 09:00:00 on the fake clock

export interface MakeRun {
  id: string
  taskId: string | null
  title: string
  startWall: number
  gen: GenOpts
  budget?: number
  review?: Run['review']
  line?: string
  groupId?: string
  workflow?: string
  seq: number
}

export function makeRun(m: MakeRun): Run {
  const script = genScript(m.gen)
  return {
    id: m.id,
    taskId: m.taskId,
    title: m.title,
    line: m.line ?? 'main',
    groupId: m.groupId,
    status: 'working',
    startWall: m.startWall,
    endWall: null,
    budget: m.budget ?? 8,
    spend: 0,
    phases: [],
    spans: [],
    gates: [],
    markers: [],
    script,
    idx: 0,
    shift: 0,
    blockedBy: null,
    raisedWall: null,
    review: m.review ?? 'pending',
    diff: genDiff(m.gen.seed),
    report: genReport(m.title),
    findings: genFindings(m.gen.seed),
    seq: m.seq,
    lane: '',
    workflow: m.workflow ?? 'build-review',
  }
}

export const wallOf = (run: Run, at: number): number => run.startWall + at + run.shift

function nextId(s: State, prefix: string): string {
  s.counter += 1
  return `${prefix}${s.counter}`
}

/** Play a run's script up to `now`. Mutates the (draft) state. */
export function advanceRun(s: State, run: Run, now: number): void {
  if (run.status === 'done' || run.status === 'stopped') return
  while (!run.blockedBy && run.idx < run.script.length) {
    const it = run.script[run.idx]
    const w = wallOf(run, it.at)
    if (w > now) break
    run.idx += 1
    switch (it.t) {
      case 'phase':
        run.phases.push({ key: it.key, label: it.label, reviewer: it.reviewer, start: w, end: null })
        break
      case 'phaseEnd': {
        const p = run.phases.find((x) => x.key === it.key)
        if (p) p.end = w
        break
      }
      case 'span': {
        const sp = it.span
        const id = `${run.id}-${run.spans.length}`
        run.spans.push({
          id, runId: run.id, phase: sp.phase, kind: sp.kind, tool: sp.tool, exe: sp.exe, input: sp.input, start: w,
          end: w + sp.dur, exit: sp.exit, error: sp.error, sid: sp.sid,
          parent: sp.parent ? run.spans.find((x) => x.sid === sp.parent)?.id : undefined,
          timing: sp.kind === 'hook' ? 'exact' : 'approx',
        })
        run.spend += 0.003 * (sp.dur + 2)
        break
      }
      case 'gate':
        run.gates.push({ at: w, phase: it.phase, name: it.name, verdict: it.verdict, note: it.note })
        if (it.verdict === 'unverified' && !run.blockedBy) {
          // the build gate came back unverified: the phase is held for a decision
          const spec = it.spec ?? {
            kind: 'gate' as const, title: `${it.name} unverified in ${run.id}`, body: 'tsc --noEmit', cwd: `work/${run.id}`,
            why: 'The typecheck gate could not run, so Collide cannot call it a pass.',
            ctx: { why: [`gate ${it.name}: exit 127`, 'tsc not found in the worktree'] }, expect: ['retry', 'hold', 'stop'] as Effect[],
          }
          raise(s, run, spec, it.phase, w)
        }
        break
      case 'marker':
        run.markers.push({ at: w, kind: 'finding', text: it.text })
        break
      case 'raise':
        raise(s, run, it.spec, it.phase, w)
        break
      case 'end':
        run.status = 'done'
        run.endWall = w
        if (run.review === 'pending') {
          const id = nextId(s, 'Q-')
          s.queue.push({ ...reviewSpec(run, s), id, runId: run.id, phase: 'close', raisedWall: w, status: 'pending' })
        }
        return
    }
  }
}

function raise(s: State, run: Run, spec: Omit<QueueItem, 'id' | 'runId' | 'phase' | 'raisedWall' | 'status'>, phase: string, w: number): void {
  const id = nextId(s, 'Q-')
  s.queue.push({ ...spec, id, runId: run.id, phase, raisedWall: w, status: 'pending' })
  run.blockedBy = id
  run.raisedWall = w
  run.status = spec.kind === 'gate' ? 'failed' : 'waiting'
  run.spans.push({
    id: `${run.id}-${run.spans.length}`, runId: run.id, phase, kind: 'wait', tool: 'queue wait', input: spec.title, start: w,
    end: null, itemId: id,
  })
}

export function advanceAll(s: State, now: number): void {
  s.simNow = now
  for (const r of s.runs) advanceRun(s, r, now)
  for (const e of s.experiments) {
    if (e.status === 'running' && now >= e.startedWall + e.duration) e.status = 'complete'
  }
}

/** Resolve a pending item with an effect: the run resumes, stops or pauses. */
export function resolveItem(s: State, itemId: string, key: string, effect: Effect, matched: boolean): void {
  const it = s.queue.find((q) => q.id === itemId)
  if (!it || it.status !== 'pending') return
  const run = s.runs.find((r) => r.id === it.runId)
  it.status = 'resolved'
  it.answer = { key, effect, wall: s.simNow, matched }
  if (!run) return
  if (it.kind === 'review') {
    run.review = effect === 'accept' ? 'accepted' : effect === 'reprompt' ? 'reprompted' : 'discarded'
    return
  }
  const w = run.spans.find((x) => x.itemId === itemId && x.end === null)
  if (w) w.end = s.simNow
  if (run.raisedWall !== null) run.shift += s.simNow - run.raisedWall
  run.raisedWall = null
  if (effect === 'stop' || effect === 'discard') {
    run.status = 'stopped'
    run.blockedBy = itemId
    run.endWall = s.simNow
  } else if (effect === 'hold' || effect === 'pause') {
    run.status = 'paused'
    run.blockedBy = itemId
  } else {
    run.blockedBy = null
    run.status = 'working'
    if (effect === 'deny' || effect === 'reject') {
      run.spans.push({
        id: `${run.id}-${run.spans.length}`, runId: run.id, phase: it.phase, kind: 'tool', tool: 'Bash', exe: 'git',
        input: `denied: ${it.title}`, start: s.simNow, end: s.simNow + 0.2, error: true, exit: 1,
      })
    }
    advanceRun(s, run, s.simNow)
  }
}

/** Phases not yet started, with projected times, for the dashed remainder on the axis. */
export function plannedPhases(run: Run, now: number): Phase[] {
  if (run.status === 'done' || run.status === 'stopped') return []
  const out: Phase[] = []
  const blocked = run.blockedBy && run.raisedWall !== null ? now - run.raisedWall : 0
  for (let i = run.idx; i < run.script.length; i++) {
    const it = run.script[i]
    if (it.t !== 'phase') continue
    const end = run.script.find((x, j) => j > i && x.t === 'phaseEnd' && x.key === it.key)
    out.push({
      key: it.key, label: it.label, reviewer: it.reviewer,
      start: wallOf(run, it.at) + blocked, end: end ? wallOf(run, end.at) + blocked : null,
    })
  }
  return out
}

export function projectedEnd(run: Run, now: number): number {
  if (run.endWall !== null) return run.endWall
  const last = run.script[run.script.length - 1]
  const blocked = run.blockedBy && run.raisedWall !== null ? now - run.raisedWall : 0
  return wallOf(run, last.at) + blocked
}

export function liveRuns(s: State): Run[] {
  return s.runs.filter((r) => r.status !== 'done' && r.status !== 'stopped')
}

export function phaseSpecDuration(ps: PhaseSpec[]): number {
  return ps.reduce((a, p) => a + p.dur, 0)
}
