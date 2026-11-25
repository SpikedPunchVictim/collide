import { describe, expect, test } from 'vitest'
import { loadField } from '../src/field/load.ts'
import { GEOMETRY, goalsOf, graphOf, lightOf, stepsOf, walkWords } from '../src/field/walk.ts'
import {
  EPIC_DONE,
  EPIC_FIELD,
  EPIC_KNOT,
  EPIC_LATER,
  type FieldWorld,
  report,
  walkWorld,
} from './support/field-world.ts'

// What the run order and the graph make of a closure's steps (ts-0187-hb): the goals to pick, the steps as tasks, the
// lines between them and what a hover lights. Nothing here draws; the page is in `field-layouts.test.tsx`.

const SHORT = (key: string) => key.replace('plan:ts-', '')

/** The `view` each `task.list` call asked for. */
function asked(world: FieldWorld) {
  return (world.callsTo('task.list') as { view?: { type?: string } }[]).map((params) => params.view)
}

async function read(goal: string | null = null) {
  const world = walkWorld()
  const view = goal === null ? null : { name: 'x', closure: { goal, follow: 'depends_on' as const } }
  const data = await loadField(world.owner, 'b-alpha', null, view)
  return { world, data }
}

describe('what the Field reads for a walk', () => {
  test('lists the collections on the first page of the listing', async () => {
    const { data } = await read()
    expect(data.collections.map((each) => each.id)).toEqual([EPIC_FIELD, EPIC_LATER, EPIC_DONE, EPIC_KNOT])
  })

  test('asks for no walk when there is no goal, and keeps the board as it is', async () => {
    const { world, data } = await read()
    expect(data.walked).toBeNull()
    expect(asked(world).every((view) => view === undefined)).toBe(true)
  })

  test('asks for the view with the closure added, as an inline view, and keeps what the daemon reports', async () => {
    const { world, data } = await read(EPIC_FIELD)
    const view = { name: 'x', closure: { goal: EPIC_FIELD, follow: 'depends_on' } }
    expect(asked(world)).toContainEqual({ type: 'inline', view })
    expect(data.walked?.steps.map((step) => step.map(SHORT))).toEqual([
      ['0001-aa', '0013-mm'],
      ['0002-bb'],
      ['0003-cc'],
      ['0004-dd'],
    ])
    expect(data.walkProblem).toBeNull()
  })

  test('asks for one task of the walk, since only the report is wanted', async () => {
    const { world } = await read(EPIC_FIELD)
    const calls = world.callsTo('task.list') as { view?: unknown; limit?: number }[]
    expect(calls.filter((params) => params.view !== undefined).map((params) => params.limit)).toEqual([1])
  })

  test('says why a goal could not be walked, and still reads the board', async () => {
    const { data } = await read('plan:ep-000-gone')
    expect(data.walked).toBeNull()
    expect(data.walkProblem).toContain('matches no task')
    expect(data.tasks.length).toBeGreaterThan(0)
  })
})

describe('the goals to pick', () => {
  test('are the collections that have an open task, in the order the source lists them, with how many', async () => {
    const { data } = await read()
    expect(goalsOf(data)).toEqual([
      { id: EPIC_FIELD, title: 'The Field', open: 5 },
      { id: EPIC_LATER, title: 'Later work', open: 1 },
      { id: EPIC_KNOT, title: 'Tied in a knot', open: 3 },
    ])
  })

  test('leave out a collection whose tasks are all finished', async () => {
    const { data } = await read()
    expect(goalsOf(data).map((each) => each.id)).not.toContain(EPIC_DONE)
  })

  test('are none for a board with no collection', async () => {
    const { world } = await read()
    world.collections = []
    expect(goalsOf(await loadField(world.owner, 'b-alpha', null))).toEqual([])
  })
})

describe('the steps', () => {
  test('are the report of the walk, as tasks, in the order it gives', async () => {
    const { data } = await read(EPIC_FIELD)
    const steps = stepsOf(data.walked, data)
    expect(steps.map((step) => step.map((each) => SHORT(each.key)))).toEqual([
      ['0001-aa', '0013-mm'],
      ['0002-bb'],
      ['0003-cc'],
      ['0004-dd'],
    ])
    expect(steps[0]![0]!.title).toBe('Write the parser')
  })

  test('leave out a key the board does not list, and a step that is left with nothing', async () => {
    const { data } = await read()
    const said = report('x', 1, { steps: [['plan:ts-9999-zz'], ['plan:ts-0005-ee']] })
    expect(stepsOf(said, data).map((step) => step.map((each) => each.key))).toEqual([['plan:ts-0005-ee']])
  })

  test('are none for no report', async () => {
    const { data } = await read()
    expect(stepsOf(null, data)).toEqual([])
  })

  test('say how many open tasks and how many steps, in the mock words', async () => {
    const { data } = await read(EPIC_FIELD)
    expect(walkWords(stepsOf(data.walked, data))).toBe('5 open tasks in 4 steps.')
    expect(walkWords([])).toBe('0 open tasks in 0 steps.')
  })

  test('say one task in one step in the singular', async () => {
    const { data } = await read(EPIC_LATER)
    expect(walkWords(stepsOf(data.walked, data))).toBe('1 open task in 1 step.')
  })
})

describe('the graph', () => {
  async function graph(goal: string) {
    const { data } = await read(goal)
    const steps = stepsOf(data.walked, data)
    return graphOf(steps, data.walked?.cycle ?? [])
  }
  const pairs = (lines: readonly { from: string; to: string }[]) =>
    lines.map((each) => `${SHORT(each.from)}>${SHORT(each.to)}`).sort()

  test('puts each step in a column of its own, and the tasks of a step one under another', async () => {
    const { nodes } = await graph(EPIC_FIELD)
    const at = (short: string) => nodes.find((each) => each.task.key === `plan:ts-${short}`)!
    const { W, H, top, left } = GEOMETRY
    expect([at('0001-aa').x, at('0002-bb').x, at('0003-cc').x, at('0004-dd').x]).toEqual([
      left,
      left + W,
      left + 2 * W,
      left + 3 * W,
    ])
    expect([at('0001-aa').y, at('0013-mm').y].sort((a, b) => a - b)).toEqual([top, top + H])
  })

  test('sizes the drawing to the columns and the longest step', async () => {
    const { width, height } = await graph(EPIC_FIELD)
    const { W, H, top, left, nw } = GEOMETRY
    expect(width).toBe(left * 2 + 3 * W + nw)
    expect(height).toBe(top + 2 * H + 6)
  })

  test('draws one line for each blocking link between two shown tasks, and no more', async () => {
    const { edges } = await graph(EPIC_FIELD)
    expect(pairs(edges)).toEqual([
      '0001-aa>0002-bb',
      '0001-aa>0003-cc',
      '0002-bb>0003-cc',
      '0003-cc>0004-dd',
      '0013-mm>0004-dd',
    ])
  })

  test('draws no line to a finished blocker, to what is not a task, or for a link that does not block', async () => {
    const { edges } = await graph(EPIC_FIELD)
    const keys = edges.flatMap((each) => [each.from, each.to])
    // The parser waits on a finished task, the build on a key no task has, and the review is part of its epic.
    expect(keys).not.toContain('plan:ts-0006-ff')
    expect(keys).not.toContain('plan:ts-0099-zz')
    expect(edges.filter((each) => each.to === 'plan:ts-0004-dd')).toHaveLength(2)
  })

  test('draws no line for a link that says it does not block', async () => {
    const { data } = await read(EPIC_FIELD)
    const link = { type: 'blocked-by', to: 'plan:ts-0001-aa', blocking: false }
    const steps = stepsOf(data.walked, data).map((step) =>
      step.map((each) => (each.key === 'plan:ts-0002-bb' ? { ...each, links: [link] } : each)),
    )
    expect(pairs(graphOf(steps, []).edges)).not.toContain('0001-aa>0002-bb')
  })

  test('finds a blocker by another name of it, and a bare one in the source of the task', async () => {
    const { data } = await read(EPIC_FIELD)
    const steps = stepsOf(data.walked, data).map((step) =>
      step.map((each) => {
        if (each.key === 'plan:ts-0001-aa') return { ...each, aliases: ['old-1'] }
        if (each.key === 'plan:ts-0002-bb') return { ...each, links: [{ type: 'blocked-by', to: 'old-1' }] }
        if (each.key === 'plan:ts-0004-dd') return { ...each, links: [{ type: 'depends_on', to: 'ts-0003-cc' }] }
        return each
      }),
    )
    expect(pairs(graphOf(steps, []).edges)).toEqual([
      '0001-aa>0002-bb',
      '0001-aa>0003-cc',
      '0002-bb>0003-cc',
      '0003-cc>0004-dd',
    ])
  })

  test('draws a line once however many links name the same task', async () => {
    const { data } = await read(EPIC_FIELD)
    const steps = stepsOf(data.walked, data).map((step) =>
      step.map((each) =>
        each.key === 'plan:ts-0002-bb'
          ? { ...each, links: [...each.links, { type: 'blocked-by', to: 'plan:ts-0001-aa' }] }
          : each,
      ),
    )
    expect(pairs(graphOf(steps, []).edges).filter((each) => each === '0001-aa>0002-bb')).toHaveLength(1)
  })

  test('draws a line straight across for tasks in one row, and with one bend for tasks that are not', async () => {
    const { nodes, edges } = await graph(EPIC_FIELD)
    const row = (key: string) => nodes.find((each) => each.task.key === key)!.y
    for (const edge of edges) {
      const bends = (edge.d.match(/L/g) ?? []).length
      expect(bends, `${edge.from} to ${edge.to}`).toBe(row(edge.from) === row(edge.to) ? 1 : 2)
    }
  })

  test('starts a line at the right side of its blocker and ends it at the left side of what it holds up', async () => {
    const { edges, nodes } = await graph(EPIC_LATER)
    expect(edges).toEqual([])
    expect(nodes).toHaveLength(1)
    const { nw, nh } = GEOMETRY
    const field = await graph(EPIC_FIELD)
    const from = field.nodes.find((each) => each.task.key === 'plan:ts-0003-cc')!
    const to = field.nodes.find((each) => each.task.key === 'plan:ts-0004-dd')!
    const line = field.edges.find((each) => each.from === from.task.key && each.to === to.task.key)!
    expect(line.d.startsWith(`M${from.x + nw} ${from.y + nh / 2} `)).toBe(true)
    expect(line.d.endsWith(`L${to.x} ${to.y + nh / 2}`)).toBe(true)
  })

  test('draws no line among the tasks of a cycle, but keeps the line to a task that waits on one', async () => {
    const { edges, nodes } = await graph(EPIC_KNOT)
    expect(nodes).toHaveLength(3)
    expect(pairs(edges)).toEqual(['0011-kk>0012-ll'])
  })

  test('has no node and no size for no steps', () => {
    expect(graphOf([], [])).toMatchObject({ nodes: [], edges: [] })
  })
})

describe('what a hover lights', () => {
  async function lit(key: string | null) {
    const { data } = await read(EPIC_FIELD)
    const graph = graphOf(stepsOf(data.walked, data), [])
    return lightOf(graph, key)
  }

  test('lights the lines into and out of the task, and keeps the task and its neighbours clear', async () => {
    const { hot, near } = await lit('plan:ts-0003-cc')
    expect([...hot].map((each) => each.replaceAll('plan:ts-', '')).sort()).toEqual([
      '0001-aa>0003-cc',
      '0002-bb>0003-cc',
      '0003-cc>0004-dd',
    ])
    expect([...near].map(SHORT).sort()).toEqual(['0001-aa', '0002-bb', '0003-cc', '0004-dd'])
  })

  test('lights nothing for no task', async () => {
    const { hot, near } = await lit(null)
    expect(hot.size).toBe(0)
    expect(near.size).toBe(0)
  })

  test('keeps a task with no line only to itself', async () => {
    const { data } = await read(EPIC_LATER)
    const { hot, near } = lightOf(graphOf(stepsOf(data.walked, data), []), 'plan:ts-0005-ee')
    expect(hot.size).toBe(0)
    expect([...near]).toEqual(['plan:ts-0005-ee'])
  })
})
