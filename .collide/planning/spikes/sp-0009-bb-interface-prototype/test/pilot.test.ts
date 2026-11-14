// Pilot walkthroughs: one scripted run per scenario, over the reducer. Each proves the scenario can be completed and
// that the app detects the end state, which stops the timer.
import { describe, expect, it } from 'vitest'
import { s8Target, S8_RUN } from '../src/scenarios'
import { fmtClock } from '../src/world'
import { answerItem, begin, go, itemFor, last } from './helpers'

describe('S1 fresh repo to a reviewed run', () => {
  it('prompt, launch, wait for the run, review it', () => {
    let s = begin('1')
    expect(s.tasks).toHaveLength(0)
    s = go(s, { type: 'promptText', text: 'Add a --quiet flag to the CLI' }, { type: 'promptLaunch' })
    expect(s.runs).toHaveLength(1)
    s = go(s, { type: 'advance', seconds: 60 })
    expect(s.session.phase).toBe('running')
    s = go(s, { type: 'advance', seconds: 600 })
    expect(s.runs[0].status).toBe('done')
    const it = itemFor(s, s.runs[0].id)!
    expect(it.kind).toBe('review')
    s = go(s, { type: 'nav', surface: 'queue' }, { type: 'queueSelect', id: it.id }, { type: 'answer', key: 'a' })
    expect(s.session.phase).toBe('done')
    expect(last(s).detected).toBe(true)
  })
})

describe('S2 build a view and save it', () => {
  it('chips, then Save as view', () => {
    let s = begin('2')
    s = go(s,
      { type: 'toggleFilter', group: 'epic', value: 'E-07' },
      { type: 'toggleFilter', group: 'status', value: 'ready' },
      { type: 'toggleFilter', group: 'label', value: 'perf' },
      { type: 'startSave' },
    )
    expect(s.session.phase).toBe('running')
    s = go(s, { type: 'saveView', name: 'Slow spots' })
    expect(last(s).detected).toBe(true)
    expect(s.views.find((v) => v.name === 'Slow spots')?.cel).toBe('epic == "E-07" && status == "ready" && "perf" in labels')
  })
  it('a wrong filter does not end the scenario', () => {
    let s = begin('2')
    s = go(s, { type: 'toggleFilter', group: 'epic', value: 'E-07' }, { type: 'startSave' }, { type: 'saveView', name: 'Slow spots' })
    expect(s.session.phase).toBe('running')
  })
})

describe('S3 launch a group of three from a view', () => {
  it('open the view, tick three tasks, launch as a group', () => {
    let s = begin('3')
    s = go(s, { type: 'openView', id: 'v-2' })
    for (const id of ['T-03-011', 'T-03-014', 'T-03-019']) s = go(s, { type: 'toggleSelect', id })
    s = go(s, { type: 'openLaunch' }, { type: 'setLaunch', mode: 'group' }, { type: 'confirmLaunch' })
    expect(s.runs).toHaveLength(3)
    expect(last(s).detected).toBe(true)
    expect(last(s).errors).toBe(0)
  })
  it('launching them separately is an automatic error and does not finish', () => {
    let s = begin('3')
    s = go(s, { type: 'openView', id: 'v-2' })
    for (const id of ['T-03-011', 'T-03-014', 'T-03-019']) s = go(s, { type: 'toggleSelect', id })
    s = go(s, { type: 'openLaunch' }, { type: 'confirmLaunch' })
    expect(s.session.phase).toBe('running')
    expect(s.session.errors).toBe(1)
  })
})

describe('S4 clear five pending decisions', () => {
  it('five items are waiting, in five kinds', () => {
    const s = begin('4')
    const kinds = s.queue.filter((q) => q.status === 'pending').map((q) => q.kind).sort()
    expect(kinds).toEqual(['budget', 'edit', 'gate', 'permission', 'review'])
  })
  it('answer each with a', () => {
    let s = begin('4')
    for (const r of ['R-401', 'R-402', 'R-403', 'R-404', 'R-405']) s = answerItem(s, r, 'a')
    expect(last(s).detected).toBe(true)
    expect(last(s).decisions).toHaveLength(5)
  })
  it('the confirm variant needs a second press for consequential kinds', () => {
    let s = begin('4')
    s = go(s, { type: 'setVariant', key: 'keys', value: 'confirm' })
    s = answerItem(s, 'R-405', 'a')
    expect(itemFor(s, 'R-405')).toBeDefined()
    s = go(s, { type: 'answer', key: 'a' })
    expect(itemFor(s, 'R-405')).toBeUndefined()
  })
  it('undo puts the item back', () => {
    let s = begin('4')
    s = answerItem(s, 'R-402', 'a')
    expect(itemFor(s, 'R-402')).toBeUndefined()
    s = go(s, { type: 'undo' })
    expect(itemFor(s, 'R-402')).toBeDefined()
    expect(s.runs.find((r) => r.id === 'R-402')!.status).toBe('waiting')
  })
})

describe('S5 review a finished run and accept it', () => {
  it('from the Queue', () => {
    let s = begin('5')
    const it = s.queue.find((q) => q.runId === 'R-501')!
    s = go(s, { type: 'nav', surface: 'queue' }, { type: 'queueSelect', id: it.id }, { type: 'answer', key: 'a' })
    expect(last(s).detected).toBe(true)
  })
  it('from the Field card, through the review tab', () => {
    let s = begin('5')
    s = go(s, { type: 'selectRun', runId: 'R-501', tab: 'review' }, { type: 'reviewDecide', runId: 'R-501', effect: 'accept' })
    expect(last(s).detected).toBe(true)
  })
  it('accepting the other run does not finish', () => {
    let s = begin('5')
    s = go(s, { type: 'reviewDecide', runId: 'R-502', effect: 'accept' })
    expect(s.session.phase).toBe('running')
  })
})

describe('S6 compare two finished runs', () => {
  it('select two, compare, write where the time went', () => {
    let s = begin('6')
    s = go(s, { type: 'nav', surface: 'tracks' }, { type: 'compareToggle', runId: 'R-601' }, { type: 'compareToggle', runId: 'R-602' }, { type: 'openCompare' })
    s = go(s, { type: 'compareNote', text: 'The fix phase: two long permission waits' }, { type: 'compareSave' })
    expect(last(s).detected).toBe(true)
  })
  it('the second run really is longer, and the gap is waiting in fix', () => {
    const s = begin('6')
    const a = s.runs.find((r) => r.id === 'R-601')!
    const b = s.runs.find((r) => r.id === 'R-602')!
    expect(b.endWall! - b.startWall).toBeGreaterThan(a.endWall! - a.startWall + 1000)
    const wait = b.spans.filter((x) => x.kind === 'wait' && x.phase === 'fix').reduce((acc, x) => acc + (x.end! - x.start), 0)
    expect(wait).toBe(1080)
  })
  it('a wrong finding is an automatic error and does not finish', () => {
    let s = begin('6')
    s = go(s, { type: 'compareToggle', runId: 'R-601' }, { type: 'compareToggle', runId: 'R-602' }, { type: 'openCompare' })
    s = go(s, { type: 'compareNote', text: 'build was slower' }, { type: 'compareSave' })
    expect(s.session.phase).toBe('running')
    expect(s.session.errors).toBe(1)
  })
})

describe('S7 A/B experiment', () => {
  it('draft, start, wait for the result, write the verdict', () => {
    let s = begin('7')
    s = go(s, { type: 'nav', surface: 'experiments' }, { type: 'expDraft', patch: { taskId: 'T-04-012', armA: 'exp/ttl-cache', armB: 'exp/lru-cache' } }, { type: 'expStart' })
    expect(s.experiments[0].status).toBe('running')
    s = go(s, { type: 'advance', seconds: 100 })
    expect(s.experiments[0].status).toBe('running')
    s = go(s, { type: 'advance', seconds: 400 })
    expect(s.experiments[0].status).toBe('complete')
    s = go(s, { type: 'expNote', text: 'exp/lru-cache won, 14 min shorter critical path' }, { type: 'expNoteSave' })
    expect(last(s).detected).toBe(true)
  })
})

describe('S8 live output, then slow tool calls', () => {
  it('the live run is in its fix phase with build history', () => {
    const s = begin('8')
    const r = s.runs.find((x) => x.id === S8_RUN)!
    expect(r.status).toBe('working')
    expect(r.phases.map((p) => p.key)).toContain('fix')
    expect(r.phases.find((p) => p.key === 'build')!.end).not.toBeNull()
  })
  it('the target exists, is unique, and has a slower decoy outside the window', () => {
    const s = begin('8')
    const t = s8Target(s)!
    expect(t).toBeTruthy()
    const run = s.runs.find((x) => x.id === S8_RUN)!
    const spans = run.spans.filter((x) => x.phase === 'build' && x.kind !== 'text' && x.kind !== 'hook' && x.start < t.win[1] && x.end! > t.win[0] && x.end! - x.start >= 2)
    expect(spans.length).toBeGreaterThanOrEqual(4)
    const sorted = spans.map((x) => x.end! - x.start).sort((a, b) => b - a)
    expect(sorted[0] - sorted[1]).toBeGreaterThan(0.4)
    const outsideMax = Math.max(...run.spans.filter((x) => x.phase === 'build' && (x.start > t.win[1] || x.end! < t.win[0])).map((x) => x.end! - x.start))
    expect(outsideMax).toBeGreaterThan(sorted[0])
  })
  it('open the run, set min duration and window, jump to the slowest', () => {
    let s = begin('8')
    const t = s8Target(s)!
    s = go(s, { type: 'selectRun', runId: S8_RUN, tab: 'output' }, { type: 'setTab', tab: 'breakdown' })
    s = go(s, { type: 'setBd', patch: { minDur: 2 } }, { type: 'setBd', patch: { win: t.win } })
    expect(s.session.phase).toBe('running')
    s = go(s, { type: 'focusCall', id: t.id })
    expect(last(s).detected).toBe(true)
  })
  it('jumping to a different call does not finish', () => {
    let s = begin('8')
    const t = s8Target(s)!
    const other = s.runs.find((x) => x.id === S8_RUN)!.spans.find((x) => x.id !== t.id && x.kind === 'tool')!
    s = go(s, { type: 'selectRun', runId: S8_RUN }, { type: 'setBd', patch: { minDur: 2, win: t.win } }, { type: 'focusCall', id: other.id })
    expect(s.session.phase).toBe('running')
  })
  it('the card names clock times inside the build phase', () => {
    const s = begin('8')
    const t = s8Target(s)!
    expect(fmtClock(t.win[0])).toMatch(/^09:4\d:00$/)
  })
})

describe('S9 decide three items from the context view', () => {
  it('three items: command, edit, gate; each context view has facts first and the agent last', () => {
    const s = begin('9')
    const items = s.queue.filter((q) => q.status === 'pending')
    expect(items.map((q) => q.kind).sort()).toEqual(['command', 'edit', 'gate'])
    // the gate's context view deliberately cannot say what retry does or how often it was approved
    expect(items.find((q) => q.kind === 'gate')!.ctx.does).toBeUndefined()
    expect(items.find((q) => q.kind === 'command')!.ctx.does).toBeDefined()
  })
  it('decide each and say what was missing', () => {
    let s = begin('9')
    s = go(s, { type: 'nav', surface: 'queue' })
    const plan: [string, string, string][] = [['R-901', 'd', 'what is in ../shared/build'], ['R-902', 'a', ''], ['R-903', 'r', 'whether node_modules is installed']]
    for (const [run, key, note] of plan) {
      const it = itemFor(s, run)!
      s = go(s, { type: 'queueSelect', id: it.id }, { type: 'toggleCtx' }, { type: 'answer', key })
      s = go(s, { type: 'missingNote', itemId: it.id, text: note || 'nothing missing' })
    }
    expect(last(s).detected).toBe(true)
    expect(last(s).autoFlags).not.toContain('left the context view')
  })
  it('opening the run output is flagged', () => {
    let s = begin('9')
    s = go(s, { type: 'selectRun', runId: 'R-901' })
    expect(s.session.autoFlags).toContain('left the context view')
  })
})

describe('probes', () => {
  it('P1 switch line', () => {
    let s = begin('P1')
    s = go(s, { type: 'setLine', line: 'release-4' })
    expect(last(s).detected).toBe(true)
  })
  it('P2 find the run of a task on Tracks', () => {
    let s = begin('P2')
    s = go(s, { type: 'nav', surface: 'tracks' }, { type: 'selectRun', runId: 'R-701' })
    expect(last(s).detected).toBe(true)
  })
  it('P3 is stopped by the facilitator', () => {
    let s = begin('P3')
    s = go(s, { type: 'queueSelect', id: itemFor(s, 'R-702')!.id }, { type: 'openTrack' }, { type: 'stopTimer', correct: true })
    expect(last(s)).toMatchObject({ stop: 'manual', correct: true })
  })
  it('o with nothing selected opens the first item in the Queue order', () => {
    let s = begin('P3')
    s = go(s, { type: 'nav', surface: 'queue' }, { type: 'openTrack' })
    expect(s.surface).toBe('tracks')
  })
})
