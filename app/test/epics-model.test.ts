import { describe, expect, test } from 'vitest'
import {
  BAR_ORDER,
  boardLit,
  boardWords,
  epicsOf,
  findEpics,
  litIds,
  outsideView,
  progressWords,
  rankOf,
  walkWords,
} from '../src/epics/model.ts'
import { loadField } from '../src/field/load.ts'
import { lanesOf } from '../src/field/model.ts'
import { stepsOf } from '../src/field/walk.ts'
import { NO_FILTERS, celOf } from '../src/filters/model.ts'
import { type FieldWorld, epicsStripWorld, walkWorld } from './support/field-world.ts'

// The strip's rules over the scripted worlds (ts-0216-ve): which epics show, in what order, what a card counts, and
// what the lit line says. What a click does is in `epics-page.test.tsx` and `browser/fake-epics.spec.ts`.

const read = (world: FieldWorld, view: string | null = null, filter: string | null = null) =>
  loadField(world.owner, 'b-alpha', view, null, filter)

const ids = (cards: readonly { id: string }[]) => cards.map((each) => each.id.slice('plan:ep-'.length, -3))

describe('the epics the strip lists', () => {
  test('are the collections that have a task in the view, whatever the filter bar chose', async () => {
    const world = epicsStripWorld()
    const filter = celOf({ ...NO_FILTERS, epics: ['plan:ep-003-aa'] })
    const data = await read(world, null, filter)
    expect(data.tasks).toHaveLength(1)
    expect(epicsOf(data)).toHaveLength(50)
  })

  test('leave out an epic none of whose tasks is in the view', async () => {
    const data = await read(walkWorld(), 'to-the-wire')
    expect(epicsOf(data).map((each) => each.title)).toEqual(['The Field'])
  })

  test('are none for a view whose tasks are in no epic', async () => {
    expect(epicsOf(await read(epicsStripWorld(), 'loose'))).toEqual([])
  })

  test('are listed as the board lists them for the view, so the strip and the Epic box offer the same', async () => {
    const data = await read(walkWorld())
    expect(epicsOf(data).map((each) => each.id).sort()).toEqual(
      data.collections.map((each) => each.id).sort(),
    )
  })
})

describe('the order of the strip', () => {
  test('is under way, then ready, then the rest, and finished epics last', async () => {
    const cards = epicsOf(await read(epicsStripWorld()))
    expect(cards.map(rankOf)).toEqual([...cards.map(rankOf)].sort())
    const kinds = new Set(cards.map(rankOf))
    expect([...kinds]).toEqual([0, 1, 2, 3])
  })

  test('puts the epics of one rank in the order of their ids', async () => {
    const cards = epicsOf(await read(epicsStripWorld()))
    const underWay = cards.filter((each) => rankOf(each) === 0)
    expect(ids(underWay)).toEqual(['001', '006', '011', '016', '021', '026', '031', '036', '041', '046'])
    const finished = cards.filter((each) => rankOf(each) === 3)
    expect(ids(finished).slice(0, 3)).toEqual(['004', '009', '014'])
  })

  test('counts a task in review as under way, and reads no status name', async () => {
    const world = epicsStripWorld()
    // Epic 002 has one ready task; a status of its own name in the category `review` moves the epic up.
    world.edit('plan:ts-002-aa', { state: { category: 'review', status: 'sign-off' } })
    const cards = epicsOf(await read(world))
    const two = cards.find((each) => each.id === 'plan:ep-002-aa')!
    expect(rankOf(two)).toBe(0)
    expect(ids(cards).slice(0, 2)).toEqual(['001', '002'])
  })
})

describe('a card', () => {
  test('counts the epic over the whole board, not over the view', async () => {
    const world = epicsStripWorld()
    const whole = epicsOf(await read(world))
    const four = whole.find((each) => each.id === 'plan:ep-004-aa')!
    expect(four).toMatchObject({ total: 2, done: 1, open: 0, busy: 0 })
    expect(four.counts).toMatchObject({ done: 1, dropped: 1, ready: 0 })
  })

  test('counts a view that lists only some of an epic by the epic whole', async () => {
    const data = await read(walkWorld(), 'to-the-wire')
    const [card] = epicsOf(data)
    expect(card).toMatchObject({ title: 'The Field', total: 6, done: 1, open: 5, busy: 1 })
  })

  test('says how many are done of how many, how many are open and under way, or that it is finished', async () => {
    const cards = epicsOf(await read(epicsStripWorld()))
    const by = (id: string) => progressWords(cards.find((each) => each.id === `plan:${id}`)!)
    expect(by('ep-001-aa')).toEqual({ done: '0 of 2 done', open: '2 open, 1 under way' })
    expect(by('ep-002-aa')).toEqual({ done: '0 of 1 done', open: '1 open' })
    expect(by('ep-004-aa')).toEqual({ done: '1 of 2 done', open: 'Finished' })
  })

  test('has the short id of the epic and the file its source keeps it in', async () => {
    const cards = epicsOf(await read(epicsStripWorld()))
    expect(cards[0]).toMatchObject({ short: 'ep-001-aa', path: '.collide/planning/epics/ep-001-aa/epic.md' })
    const bare = epicsOf(await read(walkWorld()))
    expect(bare.every((each) => each.path === null)).toBe(true)
  })

  test('has a bar of the categories from finished to waiting', () => {
    expect(BAR_ORDER).toEqual(['done', 'dropped', 'review', 'active', 'ready', 'backlog'])
  })
})

describe('find', () => {
  test('narrows by name or by id, in any case, and no text keeps every epic', async () => {
    const cards = epicsOf(await read(epicsStripWorld()))
    expect(findEpics(cards, '')).toHaveLength(50)
    expect(findEpics(cards, '  ')).toHaveLength(50)
    expect(findEpics(cards, 'DAEMON')).toHaveLength(25)
    expect(ids(findEpics(cards, 'ep-007'))).toEqual(['007'])
    expect(findEpics(cards, 'no such epic')).toEqual([])
  })
})

describe('a pick, on the board', () => {
  const FIELD = 'plan:ep-006-wq'

  test('lights the cards of the epic, one for each place its tasks are at', async () => {
    const data = await read(walkWorld())
    const lit = litIds(data, FIELD)
    const keys = [...lit].map((id) => id.split('|')[0])
    const want = ['0001-aa', '0002-bb', '0003-cc', '0004-dd', '0006-ff', '0013-mm'].map((tail) => `plan:ts-${tail}`)
    expect(keys.sort()).toEqual(want.sort())
    expect(litIds(data, null).size).toBe(0)
  })

  test('counts the lit cards in columns that show and those in folded columns', async () => {
    const data = await read(walkWorld())
    const { lanes } = lanesOf(data)
    const lit = boardLit(lanes, { open: [], shut: [] }, litIds(data, FIELD))
    // Done starts folded: the finished task is in it, and the five open ones show.
    expect(lit).toMatchObject({ shown: 5, folded: 1 })
    expect(lit.byFolded.get('done')).toBe(1)
    expect(outsideView(data, FIELD)).toBe(0)
    expect(boardWords(lit, 0)).toBe('5 cards lit · 1 in folded columns')
  })

  test('counts an epic task the view leaves out as outside the view', async () => {
    // The view `everything` holds the open tasks, so the finished task of the epic is outside it.
    const data = await read(walkWorld(), 'everything')
    expect(outsideView(data, FIELD)).toBe(1)
    const { lanes } = lanesOf(data)
    const lit = boardLit(lanes, { open: [], shut: [] }, litIds(data, FIELD))
    expect(boardWords(lit, outsideView(data, FIELD))).toBe('5 cards lit · 1 outside the view')
  })

  test('counts a filter as leaving tasks outside the view too', async () => {
    const world = epicsStripWorld()
    const filter = celOf({ ...NO_FILTERS, categories: ['ready'] })
    const data = await read(world, null, filter)
    // Epic 001 has a ready task and a task under way: the filter leaves the second out.
    expect(outsideView(data, 'plan:ep-001-aa')).toBe(1)
  })

  test('says one card lit in the singular', () => {
    expect(boardWords({ shown: 1, folded: 0, byFolded: new Map() }, 0)).toBe('1 card lit')
  })

  test('a folded column holds the lit cards of the columns that start open as none', async () => {
    const data = await read(walkWorld())
    const { lanes } = lanesOf(data)
    const open = boardLit(lanes, { open: ['done'], shut: [] }, litIds(data, FIELD))
    expect(open.folded).toBe(0)
    expect(open.byFolded.size).toBe(0)
  })
})

describe('a pick, in the run order and the graph', () => {
  test('says how many open tasks of the epic are drawn and how many of other epics they wait on', async () => {
    const world = epicsStripWorld()
    const data = await read(world)
    const walked = world.walks.get('plan:ep-001-aa')!
    const steps = stepsOf(walked.report, data)
    expect(walkWords(steps, 'plan:ep-001-aa')).toBe('2 open tasks lit · 1 of other epics they wait on')
  })

  test('has no other epics to name when the epic waits on none', async () => {
    const world = epicsStripWorld()
    const data = await read(world)
    const steps = stepsOf(world.walks.get('plan:ep-002-aa')!.report, data)
    expect(walkWords(steps, 'plan:ep-002-aa')).toBe('1 open task lit')
  })

  test('says nothing is open in an epic with nothing to draw', () => {
    expect(walkWords([], 'plan:ep-004-aa')).toBe('Nothing open in it to draw')
  })
})
