import { renderToString } from 'react-dom/server'
import { describe, expect, test } from 'vitest'
import { EpicView } from '../src/drawer/EpicDrawer.tsx'
import { EpicStrip } from '../src/epics/EpicStrip.tsx'
import { readStripFolded, setStripFolded } from '../src/epics/fold.ts'
import { LitLine } from '../src/epics/LitLine.tsx'
import { cardOf, epicsOf, runOf } from '../src/epics/model.ts'
import { FieldPage } from '../src/field/FieldPage.tsx'
import { type FoldStore, memoryStore } from '../src/field/folds.ts'
import { loadField, loadViews } from '../src/field/load.ts'
import type { FieldState } from '../src/field/useField.ts'
import { type Filters, NO_FILTERS, celOf } from '../src/filters/model.ts'
import {
  type FieldWorld,
  epicsStripWorld,
  onePlanSource,
  walkWorld,
} from './support/field-world.ts'

// The epics strip, the lit line and the epic's panel drawn to text over the scripted worlds (ts-0216-ve). A click, a
// pick and a scroll are in `browser/fake-epics.spec.ts`: the pick is state of the page, so it is not drawn here.

const noop = () => {}
const OWNER = onePlanSource().owner

const IDLE: FieldState = {
  views: [],
  view: null,
  select: noop,
  goal: null,
  chooseGoal: noop,
  filters: NO_FILTERS,
  setFilters: noop,
  shared: false,
  reloadViews: noop,
  saveView: () => Promise.resolve(),
  shareView: () => Promise.resolve(''),
  data: null,
  problem: null,
  flashed: new Set(),
  updatedAt: null,
}

interface Over {
  view?: string | null
  filters?: Filters
  store?: FoldStore
}

async function page(world: FieldWorld, over: Over = {}) {
  const view = over.view ?? null
  const filters = over.filters ?? NO_FILTERS
  const views = await loadViews(world.owner, 'b-alpha')
  const data = await loadField(world.owner, 'b-alpha', view, null, celOf(filters))
  const field: FieldState = { ...IDLE, views, view, filters, data }
  return renderToString(<FieldPage owner={OWNER} boardId="b-alpha" field={field} store={over.store ?? memoryStore()} />)
}

const count = (html: string, text: string) => html.split(text).length - 1

describe('the strip on the Field', () => {
  test('is a region of its own with a card for each of the fifty epics, in the strip\'s order', async () => {
    const html = await page(epicsStripWorld())
    expect(html).toContain('aria-label="Epics"')
    expect(count(html, 'data-epic="')).toBe(50)
    const order = [...html.matchAll(/data-epic="plan:(ep-\d+-aa)"/g)].map((each) => each[1])
    expect(order.slice(0, 3)).toEqual(['ep-001-aa', 'ep-006-aa', 'ep-011-aa'])
    // Finished epics are last, by id.
    expect(order.at(-1)).toBe('ep-049-aa')
  })

  test('sits under the filter bar and over the view\'s own row', async () => {
    const html = await page(epicsStripWorld())
    const bar = html.indexOf('aria-label="Filters"')
    const strip = html.indexOf('aria-label="Epics"')
    const row = html.indexOf('<h1')
    expect(bar).toBeGreaterThan(-1)
    expect(strip).toBeGreaterThan(bar)
    expect(row).toBeGreaterThan(strip)
  })

  test('has a find box over eight epics, named by name or id, and none for fewer', async () => {
    expect(await page(epicsStripWorld())).toContain('placeholder="Find an epic by name or id"')
    expect(await page(walkWorld())).not.toContain('Find an epic by name or id')
  })

  test('is not drawn for a view with no task in any epic', async () => {
    expect(await page(epicsStripWorld(), { view: 'loose' })).not.toContain('aria-label="Epics"')
    expect(await page(onePlanSource())).not.toContain('aria-label="Epics"')
  })

  test('says on each card how many are done, how many are open and under way, and its id', async () => {
    const html = await page(epicsStripWorld())
    expect(html).toContain('0 of 2 done')
    expect(html).toContain('2 open, 1 under way')
    expect(html).toContain('1 of 2 done')
    expect(html).toContain('Finished')
    expect(html).toContain('ep-001-aa')
  })

  test('draws the bar of a card with widths in a style attribute of the page, never a style element', async () => {
    const html = await page(epicsStripWorld())
    expect(html).not.toContain('<style')
    expect(html).toMatch(/style="flex:1"/)
  })

  test('lists every epic when a filter is on, and presses the funnel of the one it chose', async () => {
    const filters = { ...NO_FILTERS, epics: ['plan:ep-003-aa'] }
    const html = await page(epicsStripWorld(), { filters })
    expect(count(html, 'data-epic="')).toBe(50)
    const funnels = [...html.matchAll(/<button[^>]*aria-label="Show only the tasks of ([^"]*)"[^>]*>/g)]
    const pressed = funnels.filter((each) => each[0].includes('aria-pressed="true"')).map((each) => each[1])
    expect(funnels).toHaveLength(50)
    expect(pressed).toEqual(['Epic 003 Field work'])
  })

  test('is folded to its header when its fold is kept for this view, and not for another', async () => {
    const store = memoryStore()
    setStripFolded(store, 'b-alpha', 'loose', true)
    expect(count(await page(epicsStripWorld(), { store }), 'data-epic="')).toBe(50)
    setStripFolded(store, 'b-alpha', null, true)
    const folded = await page(epicsStripWorld(), { store })
    expect(count(folded, 'data-epic="')).toBe(0)
    expect(folded).toMatch(/aria-expanded="false"[^>]*title="Show the epics"/)
    expect(folded).toContain('>50<')
  })
})

describe('the fold of the strip', () => {
  test('is kept for each board and view, and starts open', () => {
    const store = memoryStore()
    expect(readStripFolded(store, 'b-alpha', null)).toBe(false)
    setStripFolded(store, 'b-alpha', null, true)
    expect(readStripFolded(store, 'b-alpha', null)).toBe(true)
    expect(readStripFolded(store, 'b-alpha', 'ready')).toBe(false)
    expect(readStripFolded(store, 'b-beta', null)).toBe(false)
    setStripFolded(store, 'b-alpha', null, false)
    expect(readStripFolded(store, 'b-alpha', null)).toBe(false)
  })

  test('is open when the store is missing or breaks', () => {
    const broken: FoldStore = {
      getItem: () => {
        throw new Error('blocked')
      },
      setItem: () => {
        throw new Error('blocked')
      },
    }
    expect(readStripFolded(null, 'b-alpha', null)).toBe(false)
    expect(readStripFolded(broken, 'b-alpha', null)).toBe(false)
    expect(() => setStripFolded(broken, 'b-alpha', null, true)).not.toThrow()
  })
})

describe('the strip with an epic picked', () => {
  const draw = async (over: { picked: string | null; store?: FoldStore }) => {
    const data = await loadField(epicsStripWorld().owner, 'b-alpha', null)
    return renderToString(
      <EpicStrip
        boardId="b-alpha"
        view={null}
        cards={epicsOf(data)}
        picked={over.picked}
        drawerOpen={false}
        onPick={noop}
        filters={NO_FILTERS}
        onFilters={noop}
        store={over.store ?? memoryStore()}
      />,
    )
  }

  test('presses and rings the card of the pick, and no other', async () => {
    const html = await draw({ picked: 'plan:ep-002-aa' })
    const cards = [...html.matchAll(/<div data-epic="([^"]*)"([^>]*)>/g)]
    const on = cards.filter((each) => each[2]!.includes('data-on="true"')).map((each) => each[1])
    expect(on).toEqual(['plan:ep-002-aa'])
    expect(count(html, 'aria-pressed="true"')).toBe(1)
  })

  test('says the epic is lit on a folded header', async () => {
    const store = memoryStore()
    setStripFolded(store, 'b-alpha', null, true)
    expect(await draw({ picked: 'plan:ep-002-aa', store })).toContain('ep-002-aa is lit')
    expect(await draw({ picked: null, store })).not.toContain('is lit')
  })
})

describe('the lit line', () => {
  const line = (drawerOpen: boolean) =>
    renderToString(
      <LitLine
        title="The Field"
        words="5 cards lit · 1 outside the view"
        drawerOpen={drawerOpen}
        onOpen={noop}
        onClear={noop}
      />,
    )

  test('names the epic, says what is lit and has Clear, as a status', () => {
    const html = line(true)
    expect(html).toContain('role="status"')
    expect(html).toContain('The Field')
    expect(html).toContain('5 cards lit · 1 outside the view')
    expect(html).toContain('>Clear<')
  })

  test('has Open the epic only while the drawer is not showing it', () => {
    expect(line(false)).toContain('Open the epic')
    expect(line(true)).not.toContain('Open the epic')
  })
})

describe('the epic in the drawer', () => {
  const FIRST = 'plan:ep-001-aa'

  const draw = async (id: string, over: { read?: boolean; problem?: string | null; shut?: string[] } = {}) => {
    const world = epicsStripWorld()
    const data = await loadField(world.owner, 'b-alpha', null)
    const collection = data.collections.find((each) => each.id === id)!
    const run = runOf(id, data, world.walks.get(id)!.report)
    return renderToString(
      <EpicView
        card={cardOf(collection, data.corpus)}
        data={data}
        run={run}
        read={over.read ?? true}
        problem={over.problem ?? null}
        shut={over.shut ?? ['details']}
        onFlip={noop}
        onOpenTask={noop}
        onLayout={noop}
        onClose={noop}
      />,
    )
  }

  test('says what it is, that it is read only, and how much of it is open', async () => {
    const html = await draw(FIRST)
    expect(html).toContain('aria-label="Epic drawer"')
    expect(html).toContain('Epic 001 Field work')
    expect(html).toContain('2 open of 2')
    expect(html).toContain('Read only.')
    expect(html).toContain('ep-001-aa')
  })

  test('has Add to Up next off, with Stage 11 in its words, and the two ways to see the run', async () => {
    const html = await draw(FIRST)
    expect(html).toMatch(/<button[^>]*disabled=""[^>]*>Add to Up next<\/button>/)
    expect(html).toContain('comes in Stage 11')
    expect(html).toContain('See the run order')
    expect(html).toContain('See the graph')
  })

  test('lists the open tasks by step, with the tasks of other epics they wait on named by epic', async () => {
    const html = await draw(FIRST)
    expect(html).toContain('Step 1, 2 that can go side by side')
    expect(html).toContain('Step 2')
    expect(html).toContain('Task 1 of epic 001')
    // Task 1 of epic 002 holds up task 1 of epic 001, and is in step 1 under the name of its own epic.
    expect(html).toContain('Task 1 of epic 002')
    expect(html).toContain('Epic 002 Daemon work')
    expect(html).toContain('and the 1 of other epics they wait on')
  })

  test('folds the finished ones under a count, and says there is nothing open in a finished epic', async () => {
    const html = await draw('plan:ep-004-aa')
    expect(html).toContain('Nothing is open in it.')
    expect(html).toContain('No open tasks.')
    expect(html).toContain('2 done or dropped')
    expect(html).toContain('<details>')
  })

  test('has Details for engineers with its file and how many tasks it has, folded until it is opened', async () => {
    const shut = await draw(FIRST)
    expect(shut).not.toContain('.collide/planning/epics/ep-001-aa/epic.md')
    const open = await draw(FIRST, { shut: [] })
    expect(open).toContain('File')
    expect(open).toContain('.collide/planning/epics/ep-001-aa/epic.md')
    expect(open).toContain('Tasks')
  })

  test('says nothing of a file when the source names none', async () => {
    const world = walkWorld()
    const data = await loadField(world.owner, 'b-alpha', null)
    const collection = data.collections[0]!
    const html = renderToString(
      <EpicView
        card={cardOf(collection, data.corpus)}
        data={data}
        run={runOf(collection.id, data, null)}
        read
        problem={null}
        shut={[]}
        onFlip={noop}
        onOpenTask={noop}
        onLayout={noop}
        onClose={noop}
      />,
    )
    expect(html).not.toContain('>File<')
    expect(html).toContain('>Tasks<')
  })

  test('says while the run order is on its way, and why when the daemon refused it', async () => {
    expect(await draw(FIRST, { read: false })).toContain('Reading the run order…')
    const refused = await draw(FIRST, { problem: 'no such goal' })
    expect(refused).toContain('role="alert"')
    expect(refused).toContain('The run order could not be read: no such goal')
  })
})
