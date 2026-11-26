import { renderToString } from 'react-dom/server'
import { describe, expect, test } from 'vitest'
import { FieldPage } from '../src/field/FieldPage.tsx'
import { memoryStore } from '../src/field/folds.ts'
import { loadField, loadViews } from '../src/field/load.ts'
import type { FieldState } from '../src/field/useField.ts'
import { ViewPickerBody } from '../src/field/ViewPicker.tsx'
import { setBarFolded } from '../src/filters/fold.ts'
import { type Filters, NO_FILTERS, celOf } from '../src/filters/model.ts'
import { type FieldWorld, filtersWorld, manyEpicsWorld, onePlanSource, view } from './support/field-world.ts'

// The filter bar drawn to text over the scripted worlds (ts-0190-tz). What a click, a key and a reload do is in
// `browser/fake-filters.spec.ts`.

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
  sharedRead: true,
  reload: noop,
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
  store?: ReturnType<typeof memoryStore>
}

async function page(world: FieldWorld, over: Over = {}) {
  const view = over.view ?? null
  const filters = over.filters ?? NO_FILTERS
  const views = await loadViews(world.owner, 'b-alpha')
  const data = await loadField(world.owner, 'b-alpha', view, null, celOf(filters))
  const field: FieldState = { ...IDLE, views, view, filters, data }
  return renderToString(<FieldPage owner={OWNER} boardId="b-alpha" field={field} store={over.store ?? memoryStore()} />)
}

describe('the filter bar', () => {
  test('is a region of its own, with its header open', async () => {
    const html = await page(filtersWorld())
    expect(html).toContain('aria-label="Filters"')
    expect(html).toMatch(/aria-expanded="true"[^>]*>(?:(?!<\/button>)[^])*Filters/)
  })

  test('has a chip for each of the six categories, each saying its count in words', async () => {
    const html = await page(filtersWorld())
    for (const [word, count] of [
      ['Backlog', 1],
      ['Ready', 3],
      ['Active', 1],
      ['In review', 1],
      ['Done', 1],
      ['Dropped', 0],
    ] as const) {
      expect(html).toContain(`aria-label="${word}, ${count} task${count === 1 ? '' : 's'}"`)
    }
  })

  test('marks a chip that is chosen as pressed, and no other', async () => {
    const html = await page(filtersWorld(), { filters: { ...NO_FILTERS, categories: ['ready'] } })
    const chips = [...html.matchAll(/<button[^>]*aria-label="([^"]*), \d+ tasks?"[^>]*>/g)]
    const pressed = chips.filter((each) => each[0].includes('aria-pressed="true"')).map((each) => each[1])
    expect(chips.length).toBeGreaterThan(6)
    expect(pressed).toEqual(['Ready'])
  })

  test('has a Comes from group when the tasks come from two sources, and not when from one', async () => {
    expect(await page(filtersWorld())).toContain('aria-label="Comes from"')
    expect(await page(onePlanSource())).not.toContain('aria-label="Comes from"')
  })

  test('has a Labels group when a task carries a label, and not when none does', async () => {
    const html = await page(filtersWorld())
    expect(html).toContain('aria-label="Labels"')
    expect(html).toContain('aria-label="perf, 2 tasks"')
    expect(html).toContain('aria-label="ux, 1 task"')
    expect(await page(onePlanSource())).not.toContain('aria-label="Labels"')
  })

  test('has the Epic box when the tasks are in two epics or more, saying Any, or the epic by its title', async () => {
    expect(await page(filtersWorld())).toContain('aria-label="Epic: Any"')
    const chosen = await page(filtersWorld(), { filters: { ...NO_FILTERS, epics: ['plan:ep-002-bb'] } })
    expect(chosen).toContain('aria-label="Epic: The drawer"')
    expect(await page(onePlanSource())).not.toContain('aria-label="Epic')
  })

  test('has the search box, named, and the expression under Advanced', async () => {
    const html = await page(filtersWorld(), { filters: { ...NO_FILTERS, categories: ['ready'], text: 'parse' } })
    expect(html).toContain('placeholder="Search by name or id"')
    expect(html).toContain('Search tasks')
    expect(html).toContain('Advanced: this view as an expression')
    expect(html).toContain('category in [&quot;ready&quot;]')
  })

  test('says "Showing X of Y" only when a choice is made, over the view and not the board', async () => {
    expect(await page(filtersWorld())).not.toContain('Showing')
    const html = await page(filtersWorld(), { view: 'open-work', filters: { ...NO_FILTERS, categories: ['active'] } })
    expect(html).toContain('Showing 1 of 6')
  })

  test('counts a view before the choices, so the chips of the view are what it holds', async () => {
    const html = await page(filtersWorld(), { view: 'open-work', filters: { ...NO_FILTERS, categories: ['active'] } })
    expect(html).toContain('aria-label="Ready, 3 tasks"')
    expect(html).toContain('aria-label="Done, 0 tasks"')
  })

  test('says why a filter was refused', async () => {
    const world = filtersWorld()
    world.refuseFilter = 'the expression is too long'
    const html = await page(world, { filters: { ...NO_FILTERS, text: 'x' } })
    expect(html).toContain('The filter could not be applied: ')
    expect(html).toContain('the expression is too long')
  })
})

describe('the folded bar', () => {
  test('is the header and a line of words, with no chips', async () => {
    const store = memoryStore()
    setBarFolded(store, 'b-alpha', true)
    const html = await page(filtersWorld(), { store })
    expect(html).toMatch(/aria-expanded="false"/)
    expect(html).toContain('No filters · 7 tasks')
    expect(html).not.toContain('Ready, 3 tasks')
    expect(html).not.toContain('Search by name or id')
  })

  test('says what is chosen and how much it shows, and has Clear filters', async () => {
    const store = memoryStore()
    setBarFolded(store, 'b-alpha', true)
    const filters = { ...NO_FILTERS, categories: ['ready' as const], epics: ['plan:ep-001-aa'] }
    const html = await page(filtersWorld(), { store, filters })
    expect(html).toContain('Epic: The Field · Where it stands: Ready · showing 2 of 7')
    expect(html).toContain('Clear filters')
  })

  test('is kept for the board it was folded on, and no other', async () => {
    const store = memoryStore()
    setBarFolded(store, 'b-gamma', true)
    expect(await page(filtersWorld(), { store })).toContain('aria-expanded="true"')
  })
})

describe('Save as a view', () => {
  test('is off with no choice made', async () => {
    expect(await page(filtersWorld())).toMatch(/<button[^>]*disabled=""[^>]*>Save as a view…/)
  })

  test('is on over the whole board, and over a view that has only a `where`', async () => {
    const filters = { ...NO_FILTERS, categories: ['ready' as const] }
    expect(await page(filtersWorld(), { filters })).not.toMatch(/<button[^>]*disabled=""[^>]*>Save as a view…/)
    expect(await page(filtersWorld(), { view: 'open-work', filters })).not.toContain('Save is off')
  })

  test('is off over a built-in view, with the words that say why', async () => {
    const html = await page(filtersWorld(), { view: 'ready', filters: { ...NO_FILTERS, text: 'x' } })
    expect(html).toContain('Save is off: ready is a built-in view')
    expect(html).toMatch(/<button[^>]*disabled=""[^>]*>Save as a view…/)
  })

  test('is off over a view that names tasks, with the words that say why', async () => {
    const html = await page(filtersWorld(), { view: 'picked', filters: { ...NO_FILTERS, text: 'x' } })
    expect(html).toContain('Save is off: This view names tasks')
  })
})

describe('the Epic box over many epics', () => {
  test('says Any, and has no more than the box itself drawn until it is opened', async () => {
    const html = await page(manyEpicsWorld())
    expect(html).toContain('aria-label="Epic: Any"')
    // The strip has a find box of its own, named otherwise (`epics-page.test.tsx`); the box's is drawn when it opens.
    expect(html).not.toContain('id="epic-find"')
    expect(html).not.toContain('Epics to show')
  })

  test('says how many are chosen', async () => {
    const epics = ['plan:ep-001-aa', 'plan:ep-002-aa', 'plan:ep-003-aa']
    expect(await page(manyEpicsWorld(), { filters: { ...NO_FILTERS, epics } })).toContain('aria-label="Epic: 3 epics"')
  })
})

describe('Share with team in the picker', () => {
  const views = [
    view('ready', { type: 'builtin' }),
    view('mine', { type: 'personal' }, { where: 'true' }),
    view('team', { type: 'repo' }),
  ]
  const draw = (shared: boolean) =>
    renderToString(
      <ViewPickerBody
        boardName="alpha"
        views={views}
        view={null}
        onChoose={noop}
        shared={shared}
        onShare={() => Promise.resolve('')}
      />,
    )

  test('is on the row of a personal view once the line has a Collide folder, and on no other row', () => {
    const html = draw(true)
    expect(html.match(/>Share with team</g)).toHaveLength(1)
    expect(html).toContain('aria-label="Share mine with team"')
    expect(html).not.toContain('Share ready with team')
    expect(html).not.toContain('Share team with team')
  })

  test('is not there while the line has no Collide folder', () => {
    expect(draw(false)).not.toContain('Share with team')
  })

  test('is gone from a personal view once the repo has the same view, which is then one row', () => {
    const same = [...views, view('mine', { type: 'repo' }, { where: 'true' })]
    const html = renderToString(
      <ViewPickerBody
        boardName="alpha"
        views={same}
        view={null}
        onChoose={noop}
        shared
        onShare={() => Promise.resolve('')}
      />,
    )
    expect(html).not.toContain('Share with team')
    expect(html.match(/in this repo/g)).toHaveLength(2)
  })
})
