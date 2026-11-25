import { describe, expect, test } from 'vitest'
import {
  type ChangesRead,
  ChangesFeed,
  LIVE_REFRESH_MS,
  isChangesEvent,
} from '../src/drawer/changes-feed.ts'
import { LIVE_DEBOUNCE_MS } from '../src/field/live.ts'
import { FakeClock } from './support/fake-clock.ts'
import { changesOf, runEntry } from './support/changes-world.ts'
import type { FieldWorld } from './support/field-world.ts'
import { changesWorld } from './support/changes-world.ts'

// The Changes part follows the daemon (ts-0189-fg; Stage 6a, "an active task shows its edits as they land"). An event
// says that it is time to look, never what to show, so the feed reads `task.changes` again. A fake clock stands in for
// time, so no test waits.

const LIVE = 'plan:ts-0001-aa'
const IDLE = 'plan:ts-0003-cc'

function event(body: Record<string, unknown>) {
  return { method: 'daemon.event', params: { seq: 1, wall_ms: 1, actor: { kind: 'workflow' }, experiment: null, body } }
}

describe('which events are a reason to read the changes', () => {
  test.each(['run.transition', 'phase.transition', 'workspace.transition', 'gate.verdict', 'task.changed'])(
    '%s is',
    (name) => {
      expect(isChangesEvent(event({ event: name }) as never)).toBe(true)
    },
  )

  test.each(['permission.requested', 'permission.answered'])('%s is, since the agent edits between them', (name) => {
    expect(isChangesEvent(event({ event: name }) as never)).toBe(true)
  })

  test('the shell saying the stream is back is, and its saying the stream is lost is not', () => {
    expect(isChangesEvent({ method: 'shell.connection', params: { state: 'back' } })).toBe(true)
    expect(isChangesEvent({ method: 'shell.connection', params: { state: 'lost' } })).toBe(false)
  })

  test('a Queue or budget event is not', () => {
    expect(isChangesEvent(event({ event: 'budget.exceeded' }) as never)).toBe(false)
    expect(isChangesEvent(event({ event: 'metric.reported' }) as never)).toBe(false)
  })
})

async function feedOf(world: FieldWorld, task: string) {
  const clock = new FakeClock()
  const reads: ChangesRead[] = []
  const feed = new ChangesFeed({
    owner: world.owner,
    boardId: 'b-alpha',
    task,
    onRead: (read) => reads.push(read),
    clock,
  })
  await feed.start()
  return { feed, clock, reads, last: () => reads[reads.length - 1]! }
}

describe('what the feed reads', () => {
  test('reads the task once at the start, for the board and the task it was given', async () => {
    const world = changesWorld()
    const { feed, last } = await feedOf(world, LIVE)
    expect(world.callsTo('task.changes')).toEqual([{ board: 'b-alpha', task: LIVE }])
    expect(last().problem).toBeNull()
    expect(last().changes?.runs.map((run) => run.run)).toEqual(['r-12', 'r-9'])
    feed.stop()
  })

  test('reads again once for a burst of events, when the debounce is over', async () => {
    const world = changesWorld()
    const { feed, clock } = await feedOf(world, IDLE)
    for (let i = 0; i < 6; i++) world.announceRun('r-1')
    clock.advance(LIVE_DEBOUNCE_MS - 1)
    expect(world.callsTo('task.changes')).toHaveLength(1)
    clock.advance(1)
    await feed.idle()
    expect(world.callsTo('task.changes')).toHaveLength(2)
    feed.stop()
  })

  test('shows what the daemon then says: a file the agent has since edited', async () => {
    const world = changesWorld()
    const { feed, clock, last } = await feedOf(world, IDLE)
    expect(last().changes?.runs).toEqual([])
    world.changes.set(IDLE, changesOf([runEntry('r-30')]))
    world.announceRun('r-30', 'phase.transition')
    clock.advance(LIVE_DEBOUNCE_MS)
    await feed.idle()
    expect(last().changes?.runs.map((run) => run.run)).toEqual(['r-30'])
    feed.stop()
  })

  test('reads on its own while a run is live, every few seconds, with no event at all', async () => {
    const world = changesWorld()
    const { feed, clock } = await feedOf(world, LIVE)
    clock.advance(LIVE_REFRESH_MS + LIVE_DEBOUNCE_MS)
    await feed.idle()
    expect(world.callsTo('task.changes')).toHaveLength(2)
    clock.advance(LIVE_REFRESH_MS + LIVE_DEBOUNCE_MS)
    await feed.idle()
    expect(world.callsTo('task.changes')).toHaveLength(3)
    feed.stop()
  })

  test('sets no timer for a task with no live run', async () => {
    const world = changesWorld()
    const { feed, clock } = await feedOf(world, 'plan:ts-0002-bb')
    expect(clock.pending()).toBe(0)
    clock.advance(10 * LIVE_REFRESH_MS)
    await feed.idle()
    expect(world.callsTo('task.changes')).toHaveLength(1)
    feed.stop()
  })

  test('stops reading on its own when the run is no longer live', async () => {
    const world = changesWorld()
    const { feed, clock } = await feedOf(world, LIVE)
    const [, done] = world.changes.get(LIVE)!.runs
    world.changes.set(LIVE, changesOf([done!]))
    clock.advance(LIVE_REFRESH_MS + LIVE_DEBOUNCE_MS)
    await feed.idle()
    expect(world.callsTo('task.changes')).toHaveLength(2)
    expect(clock.pending()).toBe(0)
    feed.stop()
  })

  test('an event of a kind it does not follow is ignored: no timer, no read', async () => {
    const world = changesWorld()
    const { feed, clock } = await feedOf(world, IDLE)
    world.owner.push(event({ event: 'budget.exceeded' }) as never)
    expect(clock.pending()).toBe(0)
    expect(world.callsTo('task.changes')).toHaveLength(1)
    feed.stop()
  })

  test('reads again when the stream comes back, since the events between may be gone', async () => {
    const world = changesWorld()
    const { feed, clock } = await feedOf(world, IDLE)
    world.owner.push({ method: 'shell.connection', params: { state: 'back' } })
    clock.advance(LIVE_DEBOUNCE_MS)
    await feed.idle()
    expect(world.callsTo('task.changes')).toHaveLength(2)
    feed.stop()
  })

  test('a read that fails after a good one keeps what was read, and says why', async () => {
    const world = changesWorld()
    const { feed, clock, last } = await feedOf(world, LIVE)
    world.refuseChanges = 'git is busy'
    world.announceRun('r-12')
    clock.advance(LIVE_DEBOUNCE_MS)
    await feed.idle()
    expect(last().changes?.runs).toHaveLength(2)
    expect(last().problem).toBe('git is busy')
    world.refuseChanges = null
    world.announceRun('r-12')
    clock.advance(LIVE_DEBOUNCE_MS)
    await feed.idle()
    expect(last().problem).toBeNull()
    feed.stop()
  })

  test('a first read that fails has no changes and says why', async () => {
    const world = changesWorld()
    world.refuseChanges = 'no such board'
    const { feed, last } = await feedOf(world, LIVE)
    expect(last().changes).toBeNull()
    expect(last().problem).toBe('no such board')
    feed.stop()
  })

  test('after stop it reads nothing, hears nothing and tells nobody', async () => {
    const world = changesWorld()
    const { feed, clock, reads } = await feedOf(world, LIVE)
    feed.stop()
    const told = reads.length
    world.announceRun('r-12')
    clock.advance(10 * LIVE_REFRESH_MS)
    await feed.idle()
    expect(world.callsTo('task.changes')).toHaveLength(1)
    expect(reads).toHaveLength(told)
    expect(clock.pending()).toBe(0)
  })
})
