import { renderToString } from 'react-dom/server'
import { describe, expect, test } from 'vitest'
import { FieldPage } from '../src/field/FieldPage.tsx'
import { memoryStore } from '../src/field/folds.ts'
import { type LayoutKind, writeLayout } from '../src/field/layout.ts'
import { loadField, loadViews } from '../src/field/load.ts'
import { NO_FILTERS } from '../src/filters/model.ts'
import type { FieldState } from '../src/field/useField.ts'
import { inlineView } from '../src/field/walk.ts'
import { EPIC_FIELD, EPIC_KNOT, EPIC_LATER, FieldWorld, walkWorld } from './support/field-world.ts'

// The layout switch, the run order and the graph, drawn to text over the walk world (ts-0187-hb). What a click, a
// hover and a key do is in `browser/fake-graph.spec.ts`.

const noop = () => {}
const OWNER = new FieldWorld().owner

interface Shown {
  readonly view?: string | null
  readonly layout?: LayoutKind
  readonly goal?: string | null
  readonly world?: FieldWorld
  readonly over?: Partial<FieldState>
}

/** The Field as the page draws it: what `useField` would have read for the view and the goal, in the layout kept. */
async function page({ view = null, layout, goal = null, world = walkWorld(), over = {} }: Shown = {}) {
  const views = await loadViews(world.owner, 'b-alpha')
  const walk = goal === null ? null : inlineView(views.find((each) => each.view.name === view), goal)
  const data = await loadField(world.owner, 'b-alpha', view, walk)
  const store = memoryStore()
  if (layout !== undefined) writeLayout(store, 'b-alpha', view, layout)
  const field: FieldState = {
    views,
    view,
    select: noop,
    goal,
    chooseGoal: noop,
    filters: NO_FILTERS,
    setFilters: noop,
    shared: false,
    sharedRead: true,
    reload: noop,
    reloadViews: noop,
    saveView: () => Promise.resolve(),
    shareView: () => Promise.resolve(''),
    data,
    problem: null,
    flashed: new Set(),
    updatedAt: null,
    ...over,
  }
  return renderToString(<FieldPage owner={OWNER} boardId="b-alpha" field={field} store={store} />)
}

const pressed = (html: string) =>
  [...html.matchAll(/<button[^>]*aria-pressed="true"[^>]*>([^<]*)</g)].map((match) => match[1])

describe('the layout switch', () => {
  test('has Board, Run order and Graph, in a group named Layout', async () => {
    const html = await page()
    expect(html).toMatch(/role="group"[^>]*aria-label="Layout"|aria-label="Layout"[^>]*role="group"/)
    for (const word of ['Board', 'Run order', 'Graph']) expect(html).toMatch(new RegExp(`<button[^>]*>${word}<`))
  })

  test('presses the board for a view that says nothing of its layout', async () => {
    expect(pressed(await page({ view: 'everything' }))).toEqual(['Board'])
    expect(await page({ view: 'everything' })).toContain('aria-label="Columns"')
  })

  test('presses the layout a view starts with', async () => {
    expect(pressed(await page({ view: 'graph-first' }))).toEqual(['Graph'])
    expect(pressed(await page({ view: 'run-first' }))).toEqual(['Run order'])
  })

  test('presses the one the person chose, over the view`s own', async () => {
    expect(pressed(await page({ view: 'graph-first', layout: 'board' }))).toEqual(['Board'])
    expect(pressed(await page({ view: 'graph-first', layout: 'run-order' }))).toEqual(['Run order'])
  })

  test('draws the columns only in the board layout', async () => {
    expect(await page({ view: 'everything', layout: 'graph' })).not.toContain('aria-label="Columns"')
    expect(await page({ view: 'everything', layout: 'run-order' })).not.toContain('aria-label="Columns"')
  })

  test('keeps the view`s name, its count and what it says of itself in every layout', async () => {
    const html = await page({ view: 'everything', layout: 'graph' })
    expect(html).toMatch(/<h1[^>]*>everything<\/h1>/)
    expect(html).toContain('9 tasks')
  })
})

describe('the goal bar', () => {
  test('names the goal of a view that has one, and offers no pick', async () => {
    const html = await page({ view: 'to-the-wire', layout: 'run-order' })
    expect(html).toContain('Toward')
    expect(html).toContain('Review the wire')
    expect(html).not.toContain('<select')
  })

  test('names a goal that is a collection by its title', async () => {
    const world = walkWorld()
    world.views.find((each) => each.view.name === 'to-the-wire')!.view.closure!.goal = EPIC_FIELD
    expect(await page({ view: 'to-the-wire', layout: 'run-order', world })).toContain('The Field')
  })

  test('offers the collections that have open tasks, for a view with no goal, and marks the goal in use', async () => {
    const html = await page({ view: 'everything', layout: 'run-order', goal: EPIC_LATER })
    const options = [...html.matchAll(/<option[^>]*value="([^"]*)"([^>]*)>([^<]*)</g)]
    expect(options.map((each) => [each[1], each[3]])).toEqual([
      [EPIC_FIELD, 'The Field'],
      [EPIC_LATER, 'Later work'],
      [EPIC_KNOT, 'Tied in a knot'],
    ])
    expect(options.filter((each) => each[2]!.includes('selected')).map((each) => each[1])).toEqual([EPIC_LATER])
  })

  test('says how many open tasks are in how many steps, and what is walked', async () => {
    const html = await page({ view: 'everything', layout: 'run-order', goal: EPIC_FIELD })
    expect(html).toContain('5 open tasks in 4 steps.')
    expect(html).toContain('Only open tasks are walked; finished ones are left out.')
  })

  test('says how the lines read, in the graph', async () => {
    const html = await page({ view: 'everything', layout: 'graph', goal: EPIC_FIELD })
    expect(html).toContain('5 open tasks in 4 steps.')
    expect(html).toContain('A line runs from each blocker to what it holds up. Hover a task to light its lines.')
  })

  test('says why a walk failed, and draws no steps', async () => {
    const html = await page({ view: 'everything', layout: 'run-order', goal: 'plan:ep-000-gone' })
    expect(html).toContain('matches no task')
    expect(html).not.toContain('Step 1')
  })

  test('says there is nothing to put in order when no collection has an open task', async () => {
    const world = walkWorld()
    world.collections = []
    for (const layout of ['run-order', 'graph'] as const) {
      expect(await page({ view: 'everything', layout, world })).toContain('Nothing open to put in order.')
    }
  })
})

describe('the run order', () => {
  test('numbers the steps, and says when each can start', async () => {
    const html = await page({ view: 'to-the-wire', layout: 'run-order' })
    expect(html).toContain('Step 1')
    expect(html).toContain('can start now')
    expect(html).toContain('Step 2')
    expect(html).toContain('after step 1')
    expect(html).toContain('after step 3')
    expect(html).not.toContain('Step 5')
  })

  test('draws the cards of each step under it, in the order of the steps', async () => {
    const html = await page({ view: 'to-the-wire', layout: 'run-order' })
    const at = (title: string) => html.indexOf(title)
    expect(at('Write the parser')).toBeGreaterThan(at('Step 1'))
    expect(at('Draw the tabs')).toBeLessThan(at('Step 2'))
    expect(at('Design the board')).toBeGreaterThan(at('Step 2'))
    expect(at('Design the board')).toBeLessThan(at('Step 3'))
    // The goal bar names the goal first, so the card is the last place the words are.
    expect(html.lastIndexOf('Review the wire')).toBeGreaterThan(at('Step 4'))
  })

  test('draws each card as a card of the board does, so a click opens the task in the drawer', async () => {
    const html = await page({ view: 'to-the-wire', layout: 'run-order' })
    const cards = html.match(/<button[^>]*data-card="[^"]*"/g) ?? []
    expect(cards).toHaveLength(5)
    expect(html).toContain('data-card="plan:ts-0003-cc|"')
  })

  test('draws the tasks of the walk the daemon gave for a picked goal, and no other', async () => {
    const html = await page({ view: 'everything', layout: 'run-order', goal: EPIC_LATER })
    expect(html).toContain('Plan the drawer')
    expect(html).not.toContain('Write the parser')
    expect(html).toContain('1 open task in 1 step.')
  })

  test('leaves out the tasks that are done', async () => {
    const html = await page({ view: 'to-the-wire', layout: 'run-order' })
    expect(html).not.toContain('Ship the shell')
  })
})

describe('the graph', () => {
  const lines = (html: string) => [...html.matchAll(/<path[^>]*data-from="([^"]*)"[^>]*data-to="([^"]*)"/g)]

  test('draws a line for each blocking link between two shown tasks, naming both ends', async () => {
    const html = await page({ view: 'to-the-wire', layout: 'graph' })
    expect(lines(html).map((each) => `${each[1]}>${each[2]}`.replaceAll('plan:ts-', '')).sort()).toEqual([
      '0001-aa>0002-bb',
      '0001-aa>0003-cc',
      '0002-bb>0003-cc',
      '0003-cc>0004-dd',
      '0013-mm>0004-dd',
    ])
  })

  test('draws the lines in an svg that is hidden from a screen reader, and the tasks outside it', async () => {
    const html = await page({ view: 'to-the-wire', layout: 'graph' })
    expect(html).toMatch(/<svg[^>]*aria-hidden="true"/)
    const svg = html.slice(html.indexOf('<svg'), html.indexOf('</svg>'))
    expect(svg).not.toContain('<button')
  })

  test('labels each column with its step, and says the first can start now', async () => {
    const html = await page({ view: 'to-the-wire', layout: 'graph' })
    expect(html).toContain('Step 1 · can start now')
    expect(html).toContain('Step 4')
    expect(html).not.toContain('Step 5')
  })

  test('draws each task as a button that opens it, with its title, short id and status', async () => {
    const html = await page({ view: 'to-the-wire', layout: 'graph' })
    expect(html.match(/<button[^>]*data-card="[^"]*"/g)).toHaveLength(5)
    expect(html).toContain('Build the Field')
    expect(html).toContain('0003-cc · active')
  })

  test('lights nothing until a task is hovered or focused', async () => {
    const html = await page({ view: 'to-the-wire', layout: 'graph' })
    expect(html).not.toContain('data-hot="true"')
    expect(html).not.toContain('data-dim="true"')
  })

  test('names a cycle above the graph, and draws no line among its tasks', async () => {
    const html = await page({ view: 'everything', layout: 'graph', goal: EPIC_KNOT })
    expect(html).toContain('wait on each other')
    expect(html.indexOf('wait on each other')).toBeLessThan(html.indexOf('<svg width'))
    expect(html.indexOf('Chicken')).toBeGreaterThan(-1)
    expect(lines(html).map((each) => `${each[1]}>${each[2]}`)).toEqual(['plan:ts-0011-kk>plan:ts-0012-ll'])
  })

  test('has no cycle note when there is no cycle', async () => {
    expect(await page({ view: 'to-the-wire', layout: 'graph' })).not.toContain('wait on each other')
  })

  test('draws no lines for a closure that is a single task', async () => {
    const html = await page({ view: 'everything', layout: 'graph', goal: EPIC_LATER })
    expect(lines(html)).toEqual([])
    expect(html).toContain('Plan the drawer')
  })
})
