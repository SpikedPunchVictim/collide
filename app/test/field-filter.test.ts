import { describe, expect, test } from 'vitest'
import { loadField } from '../src/field/load.ts'
import { EPIC_FIELD, EPIC_LATER, type FieldWorld, walkWorld } from './support/field-world.ts'

// What the Field reads when the filter bar has made a choice (ts-0190-tz): the board is the view narrowed by the bar's
// expression, and the counts of the groups come from the view as it was before it.

const IN_FIELD = `"${EPIC_FIELD}" in collections`
const DONE = 'category in ["done"]'

/** The `view` and `filter` of each `task.list` call, in order. */
function asked(world: FieldWorld) {
  const calls = world.callsTo('task.list') as { view?: { name?: string }; filter?: string; cursor?: string }[]
  return calls.map((params) => [params.view?.name ?? null, params.filter ?? null])
}

const short = (tasks: readonly { key: string }[]) => tasks.map((each) => each.key.replace('plan:ts-', ''))

describe('what the Field reads under a filter', () => {
  test('reads the view with the filter, the view without it for the counts, and the board for the cards', async () => {
    const world = walkWorld()
    const data = await loadField(world.owner, 'b-alpha', 'everything', null, IN_FIELD)
    expect(short(data.tasks)).toEqual(['0001-aa', '0002-bb', '0003-cc', '0004-dd', '0013-mm'])
    expect(short(data.inView)).toHaveLength(9)
    expect(short(data.corpus)).toHaveLength(11)
    expect(asked(world)).toEqual([
      [null, null],
      ['everything', null],
      ['everything', IN_FIELD],
    ])
  })

  test('reads the board once more, not twice, for a filter over the whole board', async () => {
    const world = walkWorld()
    const data = await loadField(world.owner, 'b-alpha', null, null, DONE)
    expect(short(data.tasks)).toEqual(['0006-ff', '0007-gg'])
    expect(data.inView).toHaveLength(11)
    expect(asked(world)).toEqual([
      [null, null],
      [null, DONE],
    ])
  })

  test('reads nothing more with no filter, and the counts are over what is shown', async () => {
    const world = walkWorld()
    const data = await loadField(world.owner, 'b-alpha', 'everything', null, null)
    expect(data.inView).toEqual(data.tasks)
    expect(asked(world)).toEqual([
      [null, null],
      ['everything', null],
    ])
  })

  test('applies the filter to a view that is a rule or walks to a goal, as it does to any other', async () => {
    const world = walkWorld()
    const data = await loadField(world.owner, 'b-alpha', 'to-the-wire', null, `"${EPIC_LATER}" in collections`)
    expect(data.tasks).toEqual([])
    expect(short(data.inView)).toEqual(['0001-aa', '0002-bb', '0003-cc', '0004-dd', '0013-mm'])
  })

  test('asks for the walk with the same filter, so the run order is of the tasks the board shows', async () => {
    const world = walkWorld()
    const goal = { name: 'x', closure: { goal: EPIC_FIELD, follow: 'depends_on' as const } }
    const data = await loadField(world.owner, 'b-alpha', null, goal, 'category in ["ready"]')
    expect(data.walked?.steps.map((step) => step.map((key) => key.replace('plan:ts-', '')))).toEqual([
      ['0001-aa', '0013-mm'],
      ['0002-bb'],
      ['0004-dd'],
    ])
    const walks = (world.callsTo('task.list') as { view?: { type?: string }; filter?: string }[]).filter(
      (params) => params.view?.type === 'inline',
    )
    expect(walks.map((params) => params.filter)).toEqual(['category in ["ready"]'])
  })

  test('says why a filter was refused, shows no task, and still has the board', async () => {
    const world = walkWorld()
    world.refuseFilter = 'the expression is too long'
    const data = await loadField(world.owner, 'b-alpha', 'everything', null, IN_FIELD)
    expect(data.filterProblem).toContain('the expression is too long')
    expect(data.tasks).toEqual([])
    expect(data.inView).toHaveLength(9)
    expect(data.corpus).toHaveLength(11)
  })

  test('has no problem to say when the filter was taken', async () => {
    const data = await loadField(walkWorld().owner, 'b-alpha', 'everything', null, IN_FIELD)
    expect(data.filterProblem).toBeNull()
  })
})
