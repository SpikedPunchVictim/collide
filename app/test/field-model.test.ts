import { describe, expect, test } from 'vitest'
import { foldKey, isShut, readFolds, toggleFold } from '../src/field/folds.ts'
import { loadField, loadViews } from '../src/field/load.ts'
import {
  collisionWords,
  columnsOf,
  divergenceWords,
  groupMode,
  lanesOf,
  reportLines,
  shortIdOf,
  stageOf,
  wordsOf,
} from '../src/field/model.ts'
import type { FieldData, Task } from '../src/field/types.ts'
import {
  FieldWorld,
  PLAN,
  SCRATCH,
  eight,
  eightStatuses,
  mixedSources,
  onePlanSource,
  report,
  task,
  view,
} from './support/field-world.ts'

// The Field's columns and cards over the scripted worlds (ts-0181-ft). The worlds are `FakeOwner`s, so `loadField`
// reads them as it reads the daemon; the board is then drawn by plain functions, with no browser.

const BOARD = 'b-alpha'

async function read(world: FieldWorld, view: string | null = null): Promise<FieldData> {
  return loadField(world.owner, BOARD, view)
}

function titlesBy(lanes: ReturnType<typeof lanesOf>['lanes']): Record<string, string[]> {
  return Object.fromEntries(lanes.map((lane) => [lane.column.key, lane.cards.map((card) => card.title)]))
}

describe('loading', () => {
  test('the whole board is listed once, with its sources and its board-wide lists', async () => {
    const world = onePlanSource()
    const data = await read(world)
    expect(data.tasks).toHaveLength(10)
    expect(data.sources.map((source) => source.name)).toEqual(['plan'])
    expect(data.collisions).toHaveLength(1)
    expect(data.divergence).toHaveLength(3)
    expect(data.report).toBeNull()
    expect(world.callsTo('task.list')).toEqual([{ board: BOARD }])
  })

  test('a view lists its own members and its report, and the board is read for the blockers', async () => {
    const world = onePlanSource()
    const data = await read(world, 'ready')
    expect(data.tasks.map((each) => each.key)).toEqual(['plan:ts-0004-dd', 'plan:ts-0006-ff'])
    expect(data.corpus).toHaveLength(10)
    expect(data.report?.name).toBe('ready')
    expect(world.callsTo('task.list')).toContainEqual({ board: BOARD, view: { type: 'named', name: 'ready' } })
  })

  test('the views are those of task.views, the built-in ones first', async () => {
    const world = onePlanSource()
    world.views.reverse()
    const views = await loadViews(world.owner, BOARD)
    expect(views.map((each) => each.view.name)).toEqual([
      'ready',
      'cyclic',
      'by-category',
      'by-status',
      'mine',
      'collide/plan/spikes',
    ])
  })
})

describe('grouping', () => {
  test('a status name reads as words: an underscore is a space, a hyphen stays, and the first letter is raised', () => {
    expect(['in_review', 'on-hold', 'qa', 'Done'].map(wordsOf)).toEqual(['In review', 'On-hold', 'Qa', 'Done'])
  })

  test('tasks of one source are grouped by status', async () => {
    const data = await read(onePlanSource())
    expect(groupMode(data.tasks)).toBe('status')
  })

  test('tasks of two sources are grouped by category', async () => {
    const data = await read(mixedSources(), 'both')
    expect(groupMode(data.tasks)).toBe('category')
  })

  test('a view that lists one source of a mixed board is grouped by that source status', async () => {
    const data = await read(mixedSources(), 'only-eight')
    expect(groupMode(data.tasks)).toBe('status')
  })

  test('a view layout groupBy overrides what the sources say', async () => {
    const data = await read(mixedSources(), 'only-eight')
    expect(groupMode(data.tasks, 'category')).toBe('category')
    const plan = await read(onePlanSource())
    expect(groupMode(plan.tasks, 'status')).toBe('status')
    expect(groupMode(mixedSources().listing, 'status')).toBe('status')
  })

  test('a groupBy the Field does not know is ignored', async () => {
    const data = await read(mixedSources(), 'both')
    expect(groupMode(data.tasks, 'epic')).toBe('category')
  })
})

describe('status columns, from the declared statuses', () => {
  test('a source with eight statuses shows eight columns, in its order', async () => {
    const data = await read(eightStatuses())
    const { mode, lanes } = lanesOf(data)
    expect(mode).toBe('status')
    expect(lanes.map((lane) => lane.column.key)).toEqual([
      'idea',
      'triage',
      'queued',
      'doing',
      'in_review',
      'qa',
      'done',
      'dropped',
    ])
    expect(lanes.map((lane) => lane.column.label)).toEqual([
      'Idea',
      'Triage',
      'Queued',
      'Doing',
      'In review',
      'Qa',
      'Done',
      'Dropped',
    ])
  })

  test('each card is in the column of its own status', async () => {
    const data = await read(eightStatuses())
    expect(titlesBy(lanesOf(data).lanes)).toEqual({
      idea: ['Think about it'],
      triage: [],
      queued: ['Waits on the unfinished'],
      doing: [],
      in_review: ['Check it once'],
      qa: ['Check it twice'],
      done: ['Finished work'],
      dropped: [],
    })
  })

  test('a column with no task still shows, and says so in its count', async () => {
    const data = await read(eightStatuses())
    const triage = lanesOf(data).lanes.find((lane) => lane.column.key === 'triage')
    expect(triage?.cards).toEqual([])
  })

  test('renaming the declared statuses changes the columns with no code change', async () => {
    const renamed = ['seed', 'sorted', 'next', 'going', 'peer', 'qa-pass', 'shipped', 'binned']
    const data = await read(eightStatuses(renamed))
    expect(lanesOf(data).lanes.map((lane) => lane.column.key)).toEqual(renamed)
    expect(lanesOf(data).lanes.map((lane) => lane.column.label)).toEqual([
      'Seed',
      'Sorted',
      'Next',
      'Going',
      'Peer',
      'Qa-pass',
      'Shipped',
      'Binned',
    ])
  })

  test('a status the source does not declare gets a column of its own after the declared ones', () => {
    const odd = task('eight:9', 'Odd one', 'on-hold', 'backlog')
    const columns = columnsOf('status', [odd], [eight()])
    expect(columns.map((column) => column.key)).toEqual([...eight().statuses.map((s) => s.name), 'on-hold'])
    expect(columns.at(-1)).toMatchObject({ label: 'On-hold', category: 'backlog', sub: 'counts as backlog' })
  })

  test('with no task at all, the columns are the first source statuses', () => {
    const keys = columnsOf('status', [], [eight(), PLAN]).map((column) => column.key)
    expect(keys).toEqual(eight().statuses.map((status) => status.name))
  })

  test('with no task and no source, there is no status column', () => {
    expect(columnsOf('status', [], [])).toEqual([])
  })
})

describe('headers', () => {
  test('a status that is not its category says it counts as one, in words', () => {
    const columns = columnsOf('status', [task('eight:1', 'x', 'qa', 'review')], [eight()])
    const subs = Object.fromEntries(columns.map((column) => [column.key, column.sub]))
    expect(subs.qa).toBe('counts as in review')
    expect(subs.in_review).toBe('counts as in review')
    expect(subs.idea).toBe('counts as backlog')
    expect(subs.done).toBe('finished')
  })

  test('a status named as its category says what the category means', () => {
    const columns = columnsOf('status', [task('plan:a', 'x', 'ready', 'ready')], [PLAN])
    expect(columns.map((column) => [column.label, column.sub])).toEqual([
      ['Ready', 'can start'],
      ['Active', 'being worked on'],
      ['Review', 'waiting on a review'],
      ['Done', 'finished'],
      ['Dropped', 'not doing it'],
    ])
  })

  test('category columns are the six, in order, with the category alone', () => {
    const columns = columnsOf('category', [], [])
    expect(columns.map((column) => [column.key, column.label, column.sub])).toEqual([
      ['backlog', 'Backlog', 'not ready to start'],
      ['ready', 'Ready', 'can start'],
      ['active', 'Active', 'being worked on'],
      ['review', 'In review', 'waiting on a review'],
      ['done', 'Done', 'finished'],
      ['dropped', 'Dropped', 'not doing it'],
    ])
  })
})

describe('a mixed view', () => {
  test('shows the six categories, each card with its own status name', async () => {
    const data = await read(mixedSources(), 'both')
    const { mode, lanes } = lanesOf(data)
    expect(mode).toBe('category')
    expect(lanes.map((lane) => lane.column.key)).toEqual(['backlog', 'ready', 'active', 'review', 'done', 'dropped'])
    expect(titlesBy(lanes)).toEqual({
      backlog: [],
      ready: ['Write the parser'],
      active: ['Build the Field'],
      review: ['Check it twice'],
      done: ['Finished work'],
      dropped: [],
    })
    const review = lanes.find((lane) => lane.column.key === 'review')!
    expect(review.cards[0]!.chips).toContainEqual({ id: 'status', text: 'qa', category: 'review' })
  })

  test('a status column shows no status chip', async () => {
    const data = await read(eightStatuses())
    for (const lane of lanesOf(data).lanes) {
      for (const card of lane.cards) expect(card.chips.map((chip) => chip.id)).not.toContain('status')
    }
  })
})

describe('the chips of a card', () => {
  async function cards() {
    const data = await read(onePlanSource())
    const found = new Map<string, ReturnType<typeof lanesOf>['lanes'][number]['cards'][number][]>()
    for (const lane of lanesOf(data).lanes) {
      for (const card of lane.cards) found.set(card.title, [...(found.get(card.title) ?? []), card])
    }
    return found
  }

  test('a card with no blockers, no differing copy and one source shows no chip', async () => {
    const found = await cards()
    expect(found.get('Write the parser')![0]!.chips).toEqual([])
    expect(found.get('Build the Field')![0]!.chips).toEqual([])
  })

  test('Waits on counts the blockers that are still open, and no others', async () => {
    const found = await cards()
    // Two blockers: ts-0001-aa is ready (open) and ts-0006-ff, named by its bare id, is done.
    expect(found.get('Design the board')![0]!.chips).toEqual([{ id: 'waits', text: 'Waits on 1' }])
  })

  test('a link of a type that does not block does not count', () => {
    const data: FieldData = {
      tasks: [task('plan:a', 'A', 'ready', 'ready', { links: [{ type: 'informs', to: 'plan:b' }] })],
      corpus: [task('plan:a', 'A', 'ready', 'ready'), task('plan:b', 'B', 'backlog', 'backlog')],
      sources: [PLAN],
      collisions: [],
      divergence: [],
      report: null,
      settling: false,
    }
    const cards = lanesOf(data).lanes.flatMap((lane) => lane.cards)
    expect(cards[0]!.chips).toEqual([])
  })

  test('a link marked as blocking counts whatever its type', () => {
    const data: FieldData = {
      tasks: [task('plan:a', 'A', 'ready', 'ready', { links: [{ type: 'needs', to: 'plan:b', blocking: true }] })],
      corpus: [task('plan:b', 'B', 'backlog', 'backlog')],
      sources: [PLAN],
      collisions: [],
      divergence: [],
      report: null,
      settling: false,
    }
    const cards = lanesOf(data).lanes.flatMap((lane) => lane.cards)
    expect(cards[0]!.chips).toEqual([{ id: 'waits', text: 'Waits on 1' }])
  })

  test('a source with no blocking links has no Waits on', () => {
    const data: FieldData = {
      tasks: [task('scratch:1', 'A', 'ready', 'ready', { links: [{ type: 'blocked-by', to: 'scratch:2' }] })],
      corpus: [task('scratch:2', 'B', 'backlog', 'backlog')],
      sources: [SCRATCH],
      collisions: [],
      divergence: [],
      report: null,
      settling: false,
    }
    expect(lanesOf(data).lanes.flatMap((lane) => lane.cards)[0]!.chips).toEqual([])
  })

  test('Unblocks N shows from 3 up, after Waits on, and not below', () => {
    const board = (count: number | undefined, over: Partial<Task> = {}) => {
      const target = task('plan:a', 'A', 'ready', 'ready', {
        links: [{ type: 'blocked-by', to: 'plan:b' }],
        ...(count === undefined ? {} : { unblocks: count }),
        ...over,
      })
      const data: FieldData = {
        tasks: [target],
        corpus: [target, task('plan:b', 'B', 'ready', 'ready')],
        sources: [PLAN],
        collisions: [],
        divergence: [],
        report: null,
        settling: false,
      }
      return lanesOf(data).lanes.flatMap((lane) => lane.cards)[0]!.chips
    }
    expect(board(2)).toEqual([{ id: 'waits', text: 'Waits on 1' }])
    expect(board(undefined)).toEqual([{ id: 'waits', text: 'Waits on 1' }])
    expect(board(3)).toEqual([
      { id: 'waits', text: 'Waits on 1' },
      { id: 'unblocks', text: 'Unblocks 3' },
    ])
    expect(board(12).at(-1)).toEqual({ id: 'unblocks', text: 'Unblocks 12' })
  })

  test('a done task, and a source with no blocking links, show no Unblocks', () => {
    const shown = (key: string, category: 'done' | 'ready', source: typeof PLAN) => {
      const target = task(key, 'A', category, category, { unblocks: 5 })
      const data: FieldData = {
        tasks: [target],
        corpus: [target],
        sources: [source],
        collisions: [],
        divergence: [],
        report: null,
        settling: false,
      }
      return lanesOf(data).lanes.flatMap((lane) => lane.cards)[0]!.chips
    }
    expect(shown('plan:a', 'done', PLAN)).toEqual([])
    expect(shown('scratch:a', 'ready', SCRATCH)).toEqual([])
    expect(shown('plan:a', 'ready', PLAN)).toEqual([{ id: 'unblocks', text: 'Unblocks 5' }])
  })

  test('bottlenecks lists the task first and its card shows Unblocks 5, and a task with 2 shows no chip', async () => {
    const world = new FieldWorld()
    world.sources = [PLAN]
    world.listing = [
      task('plan:ts-0001-aa', 'The hub', 'ready', 'ready', { unblocks: 5 }),
      task('plan:ts-0002-bb', 'The near miss', 'ready', 'ready', { unblocks: 2 }),
    ]
    world.views = [view('ready', { type: 'builtin' }), view('bottlenecks', { type: 'builtin' })]
    world.held.set('bottlenecks', { keys: ['plan:ts-0001-aa'], report: report('bottlenecks', 1) })
    const views = await loadViews(world.owner, BOARD)
    expect(views.map((each) => each.view.name)).toEqual(['ready', 'bottlenecks'])

    const through = lanesOf(await read(world, 'bottlenecks')).lanes.flatMap((lane) => lane.cards)
    expect(through.map((card) => [card.title, card.chips])).toEqual([
      ['The hub', [{ id: 'unblocks', text: 'Unblocks 5' }]],
    ])
    const whole = lanesOf(await read(world)).lanes.flatMap((lane) => lane.cards)
    expect(whole.find((card) => card.title === 'The near miss')!.chips).toEqual([])
  })

  test('a copy that differs is named by its state and the run it is in', async () => {
    const found = await cards()
    expect(found.get('Review the wire')![0]!.chips).toEqual([{ id: 'divergence', text: 'Review in Run r-121' }])
  })

  test('a copy that no run made is named by its checkout', async () => {
    const found = await cards()
    expect(found.get('Plan the drawer')![0]!.chips).toEqual([{ id: 'divergence', text: 'Done in wt-a' }])
  })

  test('copies that differ in several checkouts are counted', () => {
    const words = divergenceWords({
      key: 'plan:a',
      copies: [
        { status: 'done', category: 'done', checkout: '/w/one' },
        { status: 'review', category: 'review', checkout: '/w/two', run: 'r-2' },
      ],
    })
    expect(words).toBe('Differs in 2 copies')
  })

  test('a collided id shows Same id in 2 copies on each card, and never a difference', async () => {
    const found = await cards()
    const both = [...found.get('Same short id, one place')!, ...found.get('Same short id, another place')!]
    expect(both).toHaveLength(2)
    for (const card of both) expect(card.chips).toEqual([{ id: 'collision', text: 'Same id in 2 copies' }])
  })

  test('the words of a collision count its places', () => {
    const locations = [1, 2, 3].map((n) => ({ checkout: `/c${n}`, path: `p${n}.md` }))
    expect(collisionWords({ shortId: 'x', locations })).toBe('Same id in 3 copies')
  })

  test('the chips come in the order of the mock: status, waits, difference, collision', () => {
    const blocked = task('plan:a', 'A', 'review', 'review', { links: [{ type: 'blocked-by', to: 'plan:b' }] })
    const data: FieldData = {
      tasks: [blocked],
      corpus: [blocked, task('plan:b', 'B', 'ready', 'ready')],
      sources: [PLAN, eight()],
      collisions: [],
      divergence: [{ key: 'plan:a', copies: [{ status: 'done', category: 'done', checkout: '/w/x' }] }],
      report: null,
      settling: false,
    }
    const { lanes } = lanesOf(data, 'category')
    const card = lanes.flatMap((lane) => lane.cards)[0]!
    expect(card.chips.map((chip) => chip.id)).toEqual(['status', 'waits', 'divergence'])
  })

  test('a card in the active category is the one being worked on', async () => {
    const data = await read(onePlanSource())
    const cards = lanesOf(data).lanes.flatMap((lane) => lane.cards)
    expect(cards.filter((card) => card.category === 'active').map((card) => card.title)).toEqual(['Build the Field'])
  })
})

describe('the title, short id and stage of a card', () => {
  test('a card names its title, then its short id from the source pattern, then its stage', async () => {
    const data = await read(onePlanSource())
    const card = lanesOf(data)
      .lanes.flatMap((lane) => lane.cards)
      .find((each) => each.title === 'Design the board')!
    expect(card.shortId).toBe('0002-bb')
    expect(card.stage).toBe('ep-006-wq')
  })

  test('the short id is the whole key after the source when the pattern fits it', () => {
    expect(shortIdOf(task('eight:12', 'x', 'idea', 'backlog'), [eight()])).toBe('12')
  })

  test('the short id is what the pattern finds in a longer key', () => {
    const github = { ...eight(), name: 'github', short_id_pattern: '^#[0-9]+$' }
    expect(shortIdOf(task('github:acme/edge#212', 'x', 'idea', 'backlog'), [github])).toBe('#212')
  })

  test('a source that is not declared, or a pattern that finds nothing, leaves the key after the source', () => {
    expect(shortIdOf(task('mystery:abc', 'x', 'idea', 'backlog'), [PLAN])).toBe('abc')
    expect(shortIdOf(task('eight:abc', 'x', 'idea', 'backlog'), [eight()])).toBe('abc')
  })

  test('a pattern that is not a regular expression leaves the key after the source', () => {
    const broken = { ...eight(), short_id_pattern: '([' }
    expect(shortIdOf(task('eight:12', 'x', 'idea', 'backlog'), [broken])).toBe('12')
  })

  test('the stage is the epic the task is part of, and nothing when it has none', () => {
    const partOf = task('plan:a', 'A', 'ready', 'ready', { links: [{ type: 'part-of', to: 'ep-002-va' }] })
    expect(stageOf(partOf)).toBe('ep-002-va')
    expect(stageOf(task('plan:b', 'B', 'ready', 'ready'))).toBeNull()
  })
})

describe('what a view says of itself', () => {
  test('a clean view says nothing', () => {
    expect(reportLines(report('mine', 3))).toEqual([])
    expect(reportLines(null)).toEqual([])
  })

  test('a view error says why the view has no members', () => {
    expect(reportLines(report('x', 0, { error: 'where: no such field `colour`' }))).toEqual([
      'This view has an error: where: no such field `colour`',
    ])
  })

  test('a cycle of the ready view says that none of its tasks is ready', () => {
    const lines = reportLines(report('ready', 0, { cycle: ['plan:a', 'plan:b'] }))
    expect(lines).toEqual(['Tasks that wait on each other, so none of them is ready: plan:a, plan:b'])
  })

  test('a cycle of another view says only that they wait on each other', () => {
    const lines = reportLines(report('goal', 0, { cycle: ['plan:a', 'plan:b'] }))
    expect(lines).toEqual(['Tasks that wait on each other: plan:a, plan:b'])
  })

  test('an unknown blocker names the task and what it waits on', () => {
    const lines = reportLines(report('ready', 0, { unknown_blockers: [{ from: 'plan:c', to: 'ts-9999-zz' }] }))
    expect(lines).toEqual(['plan:c waits on ts-9999-zz, which is not a task, so it is not ready'])
  })

  test('an include that matches no task is named', () => {
    const lines = reportLines(report('MVP', 0, { unmatched_include: ['github:acme/edge#212', 'plan:T-01'] }))
    expect(lines).toEqual(['No task matches what the view includes: github:acme/edge#212, plan:T-01'])
  })

  test('a goal that matches no task is named', () => {
    expect(reportLines(report('MVP', 0, { goal_unmatched: 'plan:T-01-042' }))).toEqual([
      'The goal of the view, plan:T-01-042, matches no task',
    ])
  })

  test('tasks the view could not check, and tasks that ran out of steps, are counted', () => {
    const lines = reportLines(report('x', 0, { errors: { 'no such field': 2 }, over_task_budget: 1 }))
    expect(lines).toEqual([
      '2 tasks could not be checked: no such field',
      '1 task ran out of steps while the view checked it',
    ])
  })

  test('a pass budget that ran out is a stop, with the tasks it did not reach', () => {
    const lines = reportLines(report('x', 0, { pass_budget_exhausted: true, not_reached: 4 }))
    expect(lines).toEqual(['The view stopped before it had checked every task: 4 not reached'])
  })
})

describe('the fold of a column', () => {
  class Memory {
    readonly items = new Map<string, string>()
    getItem(key: string) {
      return this.items.get(key) ?? null
    }
    setItem(key: string, value: string) {
      this.items.set(key, value)
    }
  }
  const done = { key: 'done', label: 'Done', category: 'done' as const, sub: 'finished' }
  const dropped = { key: 'dropped', label: 'Dropped', category: 'dropped' as const, sub: 'not doing it' }
  const ready = { key: 'ready', label: 'Ready', category: 'ready' as const, sub: 'can start' }
  const qa = { key: 'qa', label: 'Qa', category: 'review' as const, sub: 'counts as in review' }
  const noFolds = { open: [], shut: [] }

  test('Done and Dropped start folded, and no other column', () => {
    expect([done, dropped, ready, qa].map((column) => isShut(column, noFolds))).toEqual([true, true, false, false])
  })

  test('a status of the done category starts folded whatever it is called', () => {
    const shipped = { key: 'shipped', label: 'Shipped', category: 'done' as const, sub: 'counts as done' }
    expect(isShut(shipped, noFolds)).toBe(true)
  })

  test('the key holds the board and the view, so each view has its own folds', () => {
    const keys = [
      foldKey('b-alpha', 'ready'),
      foldKey('b-alpha', 'mine'),
      foldKey('b-beta', 'ready'),
      foldKey('b-alpha', null),
    ]
    expect(new Set(keys).size).toBe(4)
    expect(keys.every((key) => key.includes('b-') && key.length > 4)).toBe(true)
  })

  test('opening a folded column is kept for that board and view, and read back', () => {
    const store = new Memory()
    const after = toggleFold(store, 'b-alpha', 'ready', done)
    expect(isShut(done, after)).toBe(false)
    expect(isShut(done, readFolds(store, 'b-alpha', 'ready'))).toBe(false)
    // Another view of the same board, and another board, keep their own.
    expect(isShut(done, readFolds(store, 'b-alpha', 'mine'))).toBe(true)
    expect(isShut(done, readFolds(store, 'b-beta', 'ready'))).toBe(true)
    // The other folded column is as it was.
    expect(isShut(dropped, readFolds(store, 'b-alpha', 'ready'))).toBe(true)
  })

  test('folding an open column is kept, and folding it again opens it', () => {
    const store = new Memory()
    toggleFold(store, 'b-alpha', null, ready)
    expect(isShut(ready, readFolds(store, 'b-alpha', null))).toBe(true)
    toggleFold(store, 'b-alpha', null, ready)
    expect(isShut(ready, readFolds(store, 'b-alpha', null))).toBe(false)
  })

  test('storage that is not there, or is not readable, is a board with the starting folds', () => {
    expect(isShut(done, readFolds(null, 'b-alpha', 'ready'))).toBe(true)
    const store = new Memory()
    store.setItem(foldKey('b-alpha', 'ready'), 'not json')
    expect(readFolds(store, 'b-alpha', 'ready')).toEqual(noFolds)
    expect(isShut(done, toggleFold(null, 'b-alpha', 'ready', done))).toBe(false)
  })
})
