import { describe, expect, test } from 'vitest'
import { loadField } from '../src/field/load.ts'
import {
  Burst,
  FLASH_MS,
  FieldFeed,
  type FieldRead,
  LIVE_DEBOUNCE_MS,
  agoWords,
  changedCards,
  isBoardEvent,
} from '../src/field/live.ts'
import { FakeClock } from './support/fake-clock.ts'
import { EPIC_FIELD, type FieldWorld, onePlanSource, task, walkWorld } from './support/field-world.ts'
import type { ViewInput } from '../src/owner/generated.ts'

// The Field's live updates (ts-0182-ad): which events are the board's, which cards flash, one read for a burst, and
// what a read keeps. A fake clock stands in for time, so no test waits.

const KEY = 'plan:ts-0001-aa'
const CARD = `${KEY}|`

/** The event `world.announce` pushes, as the feed would hear it. */
function eventFor(source: string, name: 'task.changed' | 'run.transition' = 'task.changed') {
  const task = { key: `${source}:x`, version: 'v', diff: {}, from: null, to: null, cause: 'head' }
  const body =
    name === 'task.changed'
      ? { event: name, source, task }
      : { event: name, run: 'r-1', from: null, to: { state: 'running' } }
  return { method: 'daemon.event', params: { seq: 1, wall_ms: 1, actor: { kind: 'workflow' }, experiment: null, body } }
}

describe('which events are the board', () => {
  test('a task event of a source the board has is the board', () => {
    expect(isBoardEvent(eventFor('plan') as never, ['plan', 'scratch'])).toBe(true)
  })

  test('a task event of a source the board does not have is another board', () => {
    expect(isBoardEvent(eventFor('github') as never, ['plan', 'scratch'])).toBe(false)
  })

  test('an event about a run is not the Field', () => {
    expect(isBoardEvent(eventFor('plan', 'run.transition') as never, ['plan'])).toBe(false)
  })

  test('with nothing read yet, any task event is a reason to read', () => {
    expect(isBoardEvent(eventFor('github') as never, null)).toBe(true)
  })

  test('the shell notice about the connection is not a task event', () => {
    expect(isBoardEvent({ method: 'shell.connection', params: { state: 'back' } }, ['plan'])).toBe(false)
  })
})

describe('events while the daemon is still listing', () => {
  const settling = (world: FieldWorld) => loadField(world.owner, 'b-alpha', null).then((data) => data.settling)

  test('a board that has listed everything is not settling', async () => {
    expect(await settling(onePlanSource())).toBe(false)
  })

  test('a board whose first listing is still running is settling', async () => {
    const world = onePlanSource()
    world.refreshing = true
    expect(await settling(world)).toBe(true)
  })

  test('a board with a source that has not started is settling', async () => {
    const world = onePlanSource()
    world.notStarted = [{ source: 'octo', reason: 'no node' }]
    expect(await settling(world)).toBe(true)
  })

  test('a task event for a source the read did not know is the board while the board is settling', () => {
    expect(isBoardEvent(eventFor('plan') as never, ['scratch'], true)).toBe(true)
    expect(isBoardEvent(eventFor('plan') as never, ['scratch'], false)).toBe(false)
  })
})

describe('which cards flash', () => {
  async function read(world: FieldWorld) {
    return loadField(world.owner, 'b-alpha', null)
  }

  test('only the card whose status moved it to another column', async () => {
    const world = onePlanSource()
    const before = await read(world)
    world.edit(KEY, { state: { category: 'active', status: 'active' } })
    expect([...changedCards(before, await read(world))]).toEqual([CARD])
  })

  test('only the card whose title changed', async () => {
    const world = onePlanSource()
    const before = await read(world)
    world.edit('plan:ts-0003-cc', { title: 'Build the whole Field' })
    expect([...changedCards(before, await read(world))]).toEqual(['plan:ts-0003-cc|'])
  })

  test('the card of a task that came, and not the card of a task that went', async () => {
    const world = onePlanSource()
    const before = await read(world)
    const gone = 'plan:ts-0008-hh'
    const added = task('plan:ts-0010-jj', 'New', 'ready', 'ready')
    world.listing = [...world.listing.filter((each) => each.key !== gone), added]
    expect([...changedCards(before, await read(world))]).toEqual(['plan:ts-0010-jj|'])
  })

  test('the card whose chips changed, when a copy elsewhere now differs', async () => {
    const world = onePlanSource()
    const before = await read(world)
    world.divergence = [
      ...world.divergence,
      { key: KEY, copies: [{ status: 'active', category: 'active', checkout: '/work/wt-z' }] },
    ]
    expect([...changedCards(before, await read(world))]).toEqual([CARD])
  })

  test('none when nothing a card shows changed', async () => {
    const world = onePlanSource()
    const before = await read(world)
    world.edit(KEY, { version: 'v9' })
    expect(changedCards(before, await read(world)).size).toBe(0)
  })
})

describe('the words of the Live stamp', () => {
  test.each([
    [0, 'just now'],
    [4_400, 'just now'],
    [5_000, '5 s ago'],
    [59_000, '59 s ago'],
    [60_000, '1 min ago'],
    [190_000, '3 min ago'],
    [-5_000, 'just now'],
  ])('%i ms after the change says %s', (ms, words) => {
    expect(agoWords(1_000, 1_000 + ms)).toBe(words)
  })
})

describe('one read for a burst', () => {
  test('ten pokes inside the debounce are one job, run after the first of them', async () => {
    const clock = new FakeClock()
    let runs = 0
    const burst = new Burst(clock, LIVE_DEBOUNCE_MS, async () => void runs++)
    for (let i = 0; i < 10; i++) {
      burst.poke()
      clock.advance(10)
    }
    expect(runs).toBe(0)
    clock.advance(LIVE_DEBOUNCE_MS)
    await burst.idle()
    expect(runs).toBe(1)
  })

  test('pokes while the job runs are one more job after it, not several', async () => {
    const clock = new FakeClock()
    let runs = 0
    let release = () => {}
    const burst = new Burst(clock, LIVE_DEBOUNCE_MS, async () => {
      runs++
      if (runs === 1) await new Promise<void>((resolve) => (release = resolve))
    })
    burst.poke()
    clock.advance(LIVE_DEBOUNCE_MS)
    for (let i = 0; i < 5; i++) burst.poke()
    release()
    await burst.idle()
    clock.advance(LIVE_DEBOUNCE_MS)
    await burst.idle()
    expect(runs).toBe(2)
  })

  test('a stopped burst runs nothing, and leaves no timer', () => {
    const clock = new FakeClock()
    let runs = 0
    const burst = new Burst(clock, LIVE_DEBOUNCE_MS, async () => void runs++)
    burst.poke()
    burst.stop()
    expect(clock.pending()).toBe(0)
    burst.poke()
    clock.advance(LIVE_DEBOUNCE_MS)
    expect(runs).toBe(0)
  })
})

describe('the feed', () => {
  async function feedOf(
    world: FieldWorld,
    over: { view?: string | null; groupBy?: string | null; walk?: () => ViewInput | null; filter?: string } = {},
  ) {
    const clock = new FakeClock()
    const reads: FieldRead[] = []
    let now = 1_000
    const feed = new FieldFeed({
      owner: world.owner,
      boardId: 'b-alpha',
      view: over.view ?? null,
      groupBy: () => over.groupBy ?? null,
      ...(over.walk === undefined ? {} : { walk: over.walk }),
      ...(over.filter === undefined ? {} : { filter: over.filter }),
      onRead: (read) => reads.push(read),
      clock,
      now: () => now,
    })
    await feed.start()
    return {
      feed,
      clock,
      reads,
      last: () => reads[reads.length - 1]!,
      at: (ms: number) => void (now = ms),
    }
  }

  const WALK: ViewInput = { name: 'x', closure: { goal: EPIC_FIELD, follow: 'depends_on' } }

  test('reads the walk of a goal with the board, so the steps come with the cards', async () => {
    const world = walkWorld()
    const { last, feed } = await feedOf(world, { walk: () => WALK })
    expect(last().data?.walked?.steps).toHaveLength(4)
    expect(last().data?.walkProblem).toBeNull()
    feed.stop()
  })

  test('reads with the filter at the start and at every read after, and a card that comes in flashes', async () => {
    const world = walkWorld()
    const filter = 'category in ["active"]'
    const { feed, clock, last } = await feedOf(world, { filter })
    expect(last().data?.tasks.map((each) => each.key)).toEqual(['plan:ts-0003-cc'])
    world.edit('plan:ts-0001-aa', { state: { category: 'active', status: 'active' } })
    world.announce('plan')
    clock.advance(LIVE_DEBOUNCE_MS)
    await feed.idle()
    expect(last().data?.tasks.map((each) => each.key)).toEqual(['plan:ts-0001-aa', 'plan:ts-0003-cc'])
    expect(last().flashed.has(`${KEY}|`)).toBe(true)
    const calls = world.callsTo('task.list') as { filter?: string }[]
    expect(calls.filter((params) => params.filter === filter)).toHaveLength(2)
    feed.stop()
  })

  test('asks for no walk while there is no goal', async () => {
    const world = walkWorld()
    const { last, feed } = await feedOf(world, { walk: () => null })
    expect(last().data?.walked).toBeNull()
    expect((world.callsTo('task.list') as { view?: unknown }[]).every((params) => params.view === undefined)).toBe(true)
    feed.stop()
  })

  test('reads the walk again when a task changes, and publishes the steps it finds', async () => {
    const world = walkWorld()
    const { feed, clock, last } = await feedOf(world, { walk: () => WALK })
    const held = world.walks.get(EPIC_FIELD)!
    const steps = held.report.steps.slice(0, 2)
    world.walks.set(EPIC_FIELD, { ...held, report: { ...held.report, steps } })
    world.announce('plan')
    clock.advance(LIVE_DEBOUNCE_MS)
    await feed.idle()
    expect(last().data?.walked?.steps).toEqual(steps)
    feed.stop()
  })

  test('a goal the daemon refuses is words in the read, and the board is read as it was', async () => {
    const world = walkWorld()
    const gone: ViewInput = { name: 'x', closure: { goal: 'plan:gone', follow: 'depends_on' } }
    const { last, feed } = await feedOf(world, { walk: () => gone })
    expect(last().problem).toBeNull()
    expect(last().data?.walkProblem).toContain('matches no task')
    expect(last().data?.tasks.length).toBeGreaterThan(0)
    feed.stop()
  })

  test('reads the board once at the start, and nothing flashes', async () => {
    const world = onePlanSource()
    const { last, feed } = await feedOf(world)
    expect(world.lists()).toBe(1)
    expect(last().data?.tasks).toHaveLength(world.listing.length)
    expect(last().flashed.size).toBe(0)
    expect(last().updatedAt).toBe(1_000)
    feed.stop()
  })

  test('a burst of events for the board is one more read, once the debounce is over', async () => {
    const world = onePlanSource()
    const { feed, clock } = await feedOf(world)
    for (let i = 0; i < 8; i++) world.announce('plan')
    clock.advance(LIVE_DEBOUNCE_MS - 1)
    expect(world.lists()).toBe(1)
    clock.advance(1)
    await feed.idle()
    expect(world.lists()).toBe(2)
    feed.stop()
  })

  test('an event for another board is ignored: no timer, no read', async () => {
    const world = onePlanSource()
    const { feed, clock } = await feedOf(world)
    world.announce('github', 'github:acme/edge#1')
    expect(clock.pending()).toBe(0)
    clock.advance(10 * LIVE_DEBOUNCE_MS)
    await feed.idle()
    expect(world.lists()).toBe(1)
    feed.stop()
  })

  test('the card that changed flashes, once, and the stamp moves to the time of the change', async () => {
    const world = onePlanSource()
    const { feed, clock, last, at } = await feedOf(world)
    world.edit(KEY, { state: { category: 'active', status: 'active' } })
    world.announce('plan')
    at(9_000)
    clock.advance(LIVE_DEBOUNCE_MS)
    await feed.idle()
    expect([...last().flashed]).toEqual([CARD])
    expect(last().updatedAt).toBe(9_000)
    clock.advance(FLASH_MS)
    expect(last().flashed.size).toBe(0)
    expect(last().updatedAt).toBe(9_000)
    feed.stop()
  })

  test('nothing flashes while the first listing of the daemon is still coming in', async () => {
    const world = onePlanSource()
    const keep = world.listing
    world.listing = []
    world.refreshing = true
    const { feed, clock, last } = await feedOf(world)
    expect(last().data?.settling).toBe(true)
    world.listing = keep
    world.refreshing = false
    world.announce('plan')
    clock.advance(LIVE_DEBOUNCE_MS)
    await feed.idle()
    expect(last().data?.tasks).toHaveLength(keep.length)
    expect(last().flashed.size).toBe(0)
    feed.stop()
  })

  test('a read that finds the board as it was publishes nothing and leaves the stamp', async () => {
    const world = onePlanSource()
    const { feed, clock, reads, last, at } = await feedOf(world)
    const count = reads.length
    world.announce('plan')
    at(9_000)
    clock.advance(LIVE_DEBOUNCE_MS)
    await feed.idle()
    expect(world.lists()).toBe(2)
    expect(reads).toHaveLength(count)
    expect(last().updatedAt).toBe(1_000)
    feed.stop()
  })

  test('a read that fails after a good one keeps the board, and the next event reads again', async () => {
    const world = onePlanSource()
    const { feed, clock, reads, last } = await feedOf(world)
    const count = reads.length
    world.refuseList = 'the daemon is busy'
    world.announce('plan')
    clock.advance(LIVE_DEBOUNCE_MS)
    await feed.idle()
    expect(reads).toHaveLength(count)
    expect(last().problem).toBeNull()
    expect(last().data).not.toBeNull()
    world.refuseList = null
    world.edit(KEY, { title: 'Write the parser, again' })
    world.announce('plan')
    clock.advance(LIVE_DEBOUNCE_MS)
    await feed.idle()
    expect(last().data?.tasks.find((each) => each.key === KEY)?.title).toBe('Write the parser, again')
    feed.stop()
  })

  test('a first read that fails says why, and an event after it reads again', async () => {
    const world = onePlanSource()
    world.refuseList = 'no daemon'
    const { feed, clock, last } = await feedOf(world)
    expect(last().problem).toContain('no daemon')
    expect(last().data).toBeNull()
    world.refuseList = null
    world.announce('github')
    clock.advance(LIVE_DEBOUNCE_MS)
    await feed.idle()
    expect(last().problem).toBeNull()
    expect(last().data).not.toBeNull()
    feed.stop()
  })

  test('a channel that cannot be listened to is said, and nothing is read', async () => {
    const world = onePlanSource()
    world.owner.subscribe = async () => {
      throw new Error('no channel')
    }
    const { last } = await feedOf(world)
    expect(last().problem).toContain('no channel')
    expect(world.lists()).toBe(0)
  })

  test('the connection coming back reads the board again', async () => {
    const world = onePlanSource()
    const { feed, clock } = await feedOf(world)
    world.owner.push({ method: 'shell.connection', params: { state: 'lost' } })
    expect(clock.pending()).toBe(0)
    world.owner.push({ method: 'shell.connection', params: { state: 'back' } })
    clock.advance(LIVE_DEBOUNCE_MS)
    await feed.idle()
    expect(world.lists()).toBe(2)
    feed.stop()
  })

  test('a view is read through that view, at each read', async () => {
    const world = onePlanSource()
    const { feed, clock, last } = await feedOf(world, { view: 'ready' })
    expect(last().data?.tasks.map((each) => each.key)).toEqual(['plan:ts-0004-dd', 'plan:ts-0006-ff'])
    world.edit('plan:ts-0004-dd', { title: 'Review the wire, twice' })
    world.announce('plan')
    clock.advance(LIVE_DEBOUNCE_MS)
    await feed.idle()
    expect(last().data?.tasks.find((each) => each.key === 'plan:ts-0004-dd')?.title).toBe('Review the wire, twice')
    expect(world.callsTo('task.list').some((params) => JSON.stringify(params).includes('"ready"'))).toBe(true)
    feed.stop()
  })

  test('after stop, an event reads nothing and the listener is gone', async () => {
    const world = onePlanSource()
    const { feed, clock, reads } = await feedOf(world)
    const count = reads.length
    feed.stop()
    world.announce('plan')
    expect(clock.pending()).toBe(0)
    clock.advance(LIVE_DEBOUNCE_MS)
    await feed.idle()
    expect(world.lists()).toBe(1)
    expect(reads).toHaveLength(count)
  })
})
