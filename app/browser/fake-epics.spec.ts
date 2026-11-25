// The epics strip in Chrome over a fake owner (ts-0216-ve): one row of fifty cards in every layout, open work
// first, the find box, the fold kept across a reload, a pick that lights its cards and leaves the row where it was,
// the lit line, the epic in the drawer, run order and graph following the pick, and the funnel with the Epic box.
// The page plays the 50-epic world (`test/support/field-world.ts`, `epicsStripWorld`) under the built content
// security policy.
import { type Locator, type Page } from '@playwright/test'
import { expect, test } from './support/fixtures.ts'

type Open = (world: string, watch: (page: Page) => void) => Promise<Page>

/** Opens the world. The page watches for what the content security policy refuses. */
async function open(openFake: Open) {
  const refused: string[] = []
  const page = await openFake('field-epics-strip', (made) =>
    made.on('console', (message) => {
      if (/Content Security Policy/i.test(message.text())) refused.push(message.text())
    }),
  )
  await expect(strip(page).locator('[data-epic]')).toHaveCount(50)
  return { page, refused }
}

const strip = (page: Page) => page.getByRole('region', { name: 'Epics', exact: true })
const row = (page: Page) => strip(page).getByRole('group', { name: 'Epic cards' })
const epic = (page: Page, id: string) => strip(page).locator(`[data-epic="plan:${id}"]`)
const pick = (page: Page, id: string) => epic(page, id).getByRole('button', { pressed: undefined }).first()
const funnel = (page: Page, id: string) => epic(page, id).getByRole('button', { name: /^Show only the tasks of/ })
const litLine = (page: Page) => page.getByRole('status', { name: 'Picked epic' })
const epicDrawer = (page: Page) => page.getByRole('complementary', { name: 'Epic drawer' })
const taskDrawer = (page: Page) => page.getByRole('complementary', { name: 'Task drawer' })
const bar = (page: Page) => page.getByRole('region', { name: 'Filters', exact: true })
const lit = (page: Page) => page.locator('[data-card][data-lit="true"]')
/** The folded Done column, which says how many of its cards are lit. */
const doneColumn = (page: Page) => page.locator('button[data-column][title="Show Done"]')
const cards = (page: Page) => page.locator('[data-card]')
const scrollOf = (where: Locator) => where.evaluate((el) => el.scrollLeft)

function layout(page: Page, word: string) {
  return page.getByRole('group', { name: 'Layout' }).getByRole('button', { name: word, exact: true })
}

const NUMBERS = Array.from({ length: 50 }, (_, index) => index + 1)
const KIND = [0, 1, 2, 3, 1]
const rank = (number: number) => KIND[(number - 1) % 5]!
const named = (number: number) => `ep-${String(number).padStart(3, '0')}-aa`

test('the strip is one row of fifty cards in the board, the run order and the graph, and it keeps its height', async ({
  openFake,
}) => {
  const { page, refused } = await open(openFake)
  const heights: number[] = []
  const tops: number[] = []
  for (const word of ['Board', 'Run order', 'Graph']) {
    await layout(page, word).click()
    await expect(layout(page, word)).toHaveAttribute('aria-pressed', 'true')
    const box = (await row(page).boundingBox())!
    heights.push(box.height)
    tops.push(box.y)
    // One row: the cards are side by side, the row is wider than it shows, and no card is below another.
    const rows = await row(page).evaluate((el) => ({
      wide: el.scrollWidth > el.clientWidth,
      tops: new Set([...el.querySelectorAll('[data-epic]')].map((card) => card.getBoundingClientRect().top)).size,
    }))
    expect(rows).toEqual({ wide: true, tops: 1 })
  }
  expect(new Set(heights).size).toBe(1)
  expect(new Set(tops).size).toBe(1)
  expect(refused).toEqual([])
})

test('open work comes first: under way, then ready, then the rest, and finished epics last', async ({ openFake }) => {
  const { page } = await open(openFake)
  const ids = await row(page).evaluate((el) =>
    [...el.querySelectorAll('[data-epic]')].map((card) => card.getAttribute('data-epic')),
  )
  const expected = [...NUMBERS].sort((a, b) => rank(a) - rank(b) || a - b).map((number) => `plan:${named(number)}`)
  expect(ids).toEqual(expected)
  await expect(epic(page, 'ep-001-aa')).toContainText('Epic 001 Field work')
  await expect(epic(page, 'ep-001-aa')).toContainText('0 of 2 done')
  await expect(epic(page, 'ep-001-aa')).toContainText('2 open, 1 under way')
  await expect(epic(page, 'ep-004-aa')).toContainText('1 of 2 done')
  await expect(epic(page, 'ep-004-aa')).toContainText('Finished')
})

test('the find box narrows the row by name and by id, and says how many it found', async ({ openFake }) => {
  const { page } = await open(openFake)
  const find = strip(page).getByRole('searchbox', { name: 'Find an epic by name or id' })
  await expect(strip(page)).toContainText('Open work first')
  await find.fill('daemon')
  await expect(strip(page).locator('[data-epic]')).toHaveCount(25)
  await expect(strip(page)).toContainText('25 of 50')
  await find.fill('ep-007')
  await expect(strip(page).locator('[data-epic]')).toHaveCount(1)
  await expect(epic(page, 'ep-007-aa')).toBeVisible()
  await find.fill('EPIC 012')
  await expect(epic(page, 'ep-012-aa')).toBeVisible()
  await find.fill('no such epic')
  await expect(strip(page)).toContainText('No epic matches.')
  await find.fill('')
  await expect(strip(page).locator('[data-epic]')).toHaveCount(50)
})

test('the header folds the strip, and the fold holds for the view across a reload', async ({ openFake }) => {
  const { page } = await open(openFake)
  const header = strip(page).getByRole('button', { name: /^Epics/ })
  await expect(header).toHaveAttribute('aria-expanded', 'true')
  await header.click()
  await expect(header).toHaveAttribute('aria-expanded', 'false')
  await expect(strip(page).locator('[data-epic]')).toHaveCount(0)
  await expect(header).toContainText('50')
  await page.reload()
  await expect(strip(page).getByRole('button', { name: /^Epics/ })).toHaveAttribute('aria-expanded', 'false')
  await expect(strip(page).locator('[data-epic]')).toHaveCount(0)
  await strip(page).getByRole('button', { name: /^Epics/ }).click()
  await expect(strip(page).locator('[data-epic]')).toHaveCount(50)
})

test('a view with no task in an epic has no strip', async ({ openFake }) => {
  const { page } = await open(openFake)
  await page.getByRole('button', { name: 'All tasks' }).click()
  await page.getByRole('button', { name: /^loose/ }).click()
  await expect(page.getByRole('heading', { name: 'loose' })).toBeVisible()
  await expect(strip(page)).toHaveCount(0)
})

test('the strip sits under the filters and over the view\'s own row', async ({ openFake }) => {
  const { page } = await open(openFake)
  const y = async (where: Locator) => (await where.boundingBox())!.y
  const heading = page.getByRole('heading', { name: 'All tasks' })
  expect(await y(bar(page))).toBeLessThan(await y(strip(page)))
  expect(await y(strip(page))).toBeLessThan(await y(heading))
})

test('a pick lights exactly its cards and opens the epic in the drawer, and a second click ends it', async ({
  openFake,
}) => {
  const { page, refused } = await open(openFake)
  await expect(lit(page)).toHaveCount(0)
  await pick(page, 'ep-001-aa').click()
  await expect(pick(page, 'ep-001-aa')).toHaveAttribute('aria-pressed', 'true')
  await expect(lit(page)).toHaveCount(2)
  await expect(lit(page).filter({ hasText: 'Task 1 of epic 001' })).toHaveCount(1)
  await expect(lit(page).filter({ hasText: 'Task 2 of epic 001' })).toHaveCount(1)
  await expect(litLine(page)).toContainText('Epic 001 Field work')
  await expect(litLine(page)).toContainText('2 cards lit')
  await expect(epicDrawer(page).getByRole('heading', { name: 'Epic 001 Field work' })).toBeVisible()
  await pick(page, 'ep-001-aa').click()
  await expect(pick(page, 'ep-001-aa')).toHaveAttribute('aria-pressed', 'false')
  await expect(litLine(page)).toHaveCount(0)
  await expect(lit(page)).toHaveCount(0)
  await expect(epicDrawer(page)).toHaveCount(0)
  expect(refused).toEqual([])
})

test('Clear ends the pick, and Close leaves the epic picked with a way to open it again', async ({ openFake }) => {
  const { page } = await open(openFake)
  await pick(page, 'ep-001-aa').click()
  await epicDrawer(page).getByRole('button', { name: 'Close' }).click()
  await expect(epicDrawer(page)).toHaveCount(0)
  await expect(lit(page)).toHaveCount(2)
  await litLine(page).getByRole('button', { name: 'Open the epic' }).click()
  await expect(epicDrawer(page)).toBeVisible()
  await expect(litLine(page).getByRole('button', { name: 'Open the epic' })).toHaveCount(0)
  await litLine(page).getByRole('button', { name: 'Clear' }).click()
  await expect(litLine(page)).toHaveCount(0)
  await expect(epicDrawer(page)).toHaveCount(0)
  await expect(lit(page)).toHaveCount(0)
})

test('a pick leaves the row where it was', async ({ openFake }) => {
  const { page } = await open(openFake)
  await row(page).evaluate((el) => {
    el.scrollLeft = 1500
  })
  const before = await scrollOf(row(page))
  expect(before).toBeGreaterThan(1000)
  // A card that shows in the row now.
  const showing = await row(page).evaluate((el) => {
    const edge = el.getBoundingClientRect()
    const found = [...el.querySelectorAll('[data-epic]')].find((card) => {
      const at = card.getBoundingClientRect()
      return at.left >= edge.left + 4 && at.right <= edge.right - 4
    })
    return found?.getAttribute('data-epic') ?? ''
  })
  expect(showing).not.toBe('')
  await strip(page).locator(`[data-epic="${showing}"]`).getByRole('button').first().click()
  await expect(litLine(page)).toBeVisible()
  await expect(epicDrawer(page)).toBeVisible()
  expect(await scrollOf(row(page))).toBe(before)
})

test('the row moves only to keep the picked card in sight, when the pick is made elsewhere', async ({ openFake }) => {
  const { page } = await open(openFake)
  await layout(page, 'Graph').click()
  await expect(page.getByRole('group', { name: 'Goal' })).toBeVisible()
  expect(await scrollOf(row(page))).toBe(0)
  // The goal bar picks the epic; its card is far along the row, and the row goes to it.
  await page.getByLabel('Toward').selectOption('plan:ep-047-aa')
  await expect(pick(page, 'ep-047-aa')).toHaveAttribute('aria-pressed', 'true')
  const shown = await row(page).evaluate((el) => {
    const edge = el.getBoundingClientRect()
    const at = el.querySelector('[data-on="true"]')!.getBoundingClientRect()
    return { in: at.left >= edge.left && at.right <= edge.right, moved: el.scrollLeft }
  })
  // A goal picked in the bar does not open the drawer, so the row stays where it is.
  expect(shown.moved).toBe(0)
  await litLine(page).getByRole('button', { name: 'Open the epic' }).click()
  await expect(epicDrawer(page)).toBeVisible()
  await expect
    .poll(() =>
      row(page).evaluate((el) => {
        const edge = el.getBoundingClientRect()
        const at = el.querySelector('[data-on="true"]')!.getBoundingClientRect()
        return at.left >= edge.left && at.right <= edge.right
      }),
    )
    .toBe(true)
})

test('the lit line counts the lit cards, those in folded columns, and those outside the view', async ({ openFake }) => {
  const { page } = await open(openFake)
  const find = strip(page).getByRole('searchbox', { name: 'Find an epic by name or id' })
  await find.fill('ep-005')
  await pick(page, 'ep-005-aa').click()
  // Epic 005 has a ready task, which shows, and a done one, which is in the folded Done column.
  await expect(litLine(page)).toContainText('1 card lit · 1 in folded columns')
  await expect(doneColumn(page)).toContainText('1 lit')
  await expect(lit(page)).toHaveCount(1)
  await bar(page).getByRole('button', { name: /^Ready, / }).click()
  // The filter leaves the done task out of the board.
  await expect(litLine(page)).toContainText('1 card lit · 1 outside the view')
  await expect(doneColumn(page)).not.toContainText('lit')
  // The strip still lists the epic whatever the filter says.
  await expect(epic(page, 'ep-005-aa')).toBeVisible()
})

test('the drawer shows the open tasks in the daemon\'s steps, with Stage 11 for Up next', async ({
  openFake,
}) => {
  const { page } = await open(openFake)
  await pick(page, 'ep-001-aa').click()
  const drawer = epicDrawer(page)
  await expect(drawer).toContainText('2 open of 2')
  await expect(drawer).toContainText('Read only.')
  await expect(drawer.getByRole('button', { name: 'Add to Up next' })).toBeDisabled()
  await expect(drawer).toContainText('comes in Stage 11')
  await expect(drawer.getByRole('button', { name: 'See the run order' })).toBeEnabled()
  await expect(drawer.getByRole('button', { name: 'See the graph' })).toBeEnabled()
  const steps = drawer.locator('[data-step]')
  await expect(steps).toHaveCount(2)
  await expect(steps.nth(0)).toContainText('Step 1, 2 that can go side by side')
  await expect(steps.nth(0)).toContainText('Task 1 of epic 002')
  await expect(steps.nth(0)).toContainText('Epic 002 Daemon work')
  await expect(steps.nth(0)).toContainText('Task 2 of epic 001')
  await expect(steps.nth(1)).toContainText('Task 1 of epic 001')
  await expect(drawer).toContainText('and the 1 of other epics they wait on')
  // Details for engineers starts folded, and has the file when it is opened.
  await expect(drawer).not.toContainText('epic.md')
  await drawer.getByRole('button', { name: 'Details for engineers' }).click()
  await expect(drawer).toContainText('.collide/planning/epics/ep-001-aa/epic.md')
})

test('a task opened from the epic leaves the epic picked, and the epic can be opened again', async ({ openFake }) => {
  const { page } = await open(openFake)
  await pick(page, 'ep-001-aa').click()
  await epicDrawer(page).getByRole('button', { name: /Task 2 of epic 001/ }).click()
  await expect(taskDrawer(page).getByRole('heading', { name: 'Task 2 of epic 001' })).toBeVisible()
  await expect(epicDrawer(page)).toHaveCount(0)
  await expect(pick(page, 'ep-001-aa')).toHaveAttribute('aria-pressed', 'true')
  await expect(lit(page)).toHaveCount(2)
  await litLine(page).getByRole('button', { name: 'Open the epic' }).click()
  await expect(epicDrawer(page)).toBeVisible()
  await expect(taskDrawer(page)).toHaveCount(0)
})

test('a task of another epic that the pick waits on opens in the drawer too', async ({ openFake }) => {
  const { page } = await open(openFake)
  await pick(page, 'ep-001-aa').click()
  await epicDrawer(page).getByRole('button', { name: /Task 1 of epic 002/ }).click()
  await expect(taskDrawer(page).getByRole('heading', { name: 'Task 1 of epic 002' })).toBeVisible()
  await expect(pick(page, 'ep-001-aa')).toHaveAttribute('aria-pressed', 'true')
})

test('a pick makes the epic the goal of the run order and the graph, and lights its tasks', async ({ openFake }) => {
  const { page } = await open(openFake)
  await pick(page, 'ep-001-aa').click()
  await layout(page, 'Run order').click()
  await expect(page.getByLabel('Toward')).toHaveValue('plan:ep-001-aa')
  await expect(litLine(page)).toContainText('2 open tasks lit · 1 of other epics they wait on')
  await expect(page.getByRole('list', { name: 'Run order' }).locator('[data-card]')).toHaveCount(3)
  await expect(lit(page)).toHaveCount(2)
  await layout(page, 'Graph').click()
  await expect(page.getByLabel('Toward')).toHaveValue('plan:ep-001-aa')
  await expect(page.getByLabel('Graph', { exact: true }).locator('[data-card]')).toHaveCount(3)
  await expect(lit(page)).toHaveCount(2)
  await expect(litLine(page)).toContainText('2 open tasks lit')
})

test('choosing another goal in the goal bar picks that epic', async ({ openFake }) => {
  const { page } = await open(openFake)
  await layout(page, 'Graph').click()
  await expect(litLine(page)).toHaveCount(0)
  await page.getByLabel('Toward').selectOption('plan:ep-002-aa')
  await expect(pick(page, 'ep-002-aa')).toHaveAttribute('aria-pressed', 'true')
  await expect(litLine(page)).toContainText('Epic 002 Daemon work')
  await expect(litLine(page)).toContainText('1 open task lit')
  await expect(lit(page)).toHaveCount(1)
  await page.getByLabel('Toward').selectOption('plan:ep-001-aa')
  await expect(pick(page, 'ep-001-aa')).toHaveAttribute('aria-pressed', 'true')
  await expect(pick(page, 'ep-002-aa')).toHaveAttribute('aria-pressed', 'false')
  await expect(litLine(page)).toContainText('Epic 001 Field work')
  // Back on the board the pick is still the epic.
  await layout(page, 'Board').click()
  await expect(lit(page)).toHaveCount(2)
})

test('See the run order and See the graph in the epic\'s panel change the layout, keeping the goal', async ({
  openFake,
}) => {
  const { page } = await open(openFake)
  await pick(page, 'ep-001-aa').click()
  await epicDrawer(page).getByRole('button', { name: 'See the run order' }).click()
  await expect(layout(page, 'Run order')).toHaveAttribute('aria-pressed', 'true')
  await expect(page.getByLabel('Toward')).toHaveValue('plan:ep-001-aa')
  await epicDrawer(page).getByRole('button', { name: 'See the graph' }).click()
  await expect(layout(page, 'Graph')).toHaveAttribute('aria-pressed', 'true')
  await expect(page.getByLabel('Toward')).toHaveValue('plan:ep-001-aa')
})

test('the funnel shows only its epic\'s tasks, the Epic box agrees, and the strip keeps all fifty', async ({
  openFake,
}) => {
  const { page } = await open(openFake)
  await expect(funnel(page, 'ep-001-aa')).toHaveAttribute('aria-pressed', 'false')
  await funnel(page, 'ep-001-aa').click()
  await expect(funnel(page, 'ep-001-aa')).toHaveAttribute('aria-pressed', 'true')
  await expect(cards(page)).toHaveCount(2)
  await expect(strip(page).locator('[data-epic]')).toHaveCount(50)
  await expect(bar(page).getByRole('button', { name: 'Epic: Epic 001 Field work' })).toBeVisible()
  // A second epic's funnel adds it.
  await funnel(page, 'ep-002-aa').click()
  await expect(funnel(page, 'ep-002-aa')).toHaveAttribute('aria-pressed', 'true')
  await expect(funnel(page, 'ep-001-aa')).toHaveAttribute('aria-pressed', 'true')
  await expect(cards(page)).toHaveCount(3)
  await expect(bar(page).getByRole('button', { name: 'Epic: 2 epics' })).toBeVisible()
  await expect(strip(page).locator('[data-epic]')).toHaveCount(50)
  await bar(page).getByRole('button', { name: 'Epic: 2 epics' }).click()
  const box = page.getByRole('dialog', { name: 'Epics to show' })
  await expect(box.getByRole('checkbox', { name: /Epic 001/ })).toHaveAttribute('aria-checked', 'true')
  await expect(box.getByRole('checkbox', { name: /Epic 002/ })).toHaveAttribute('aria-checked', 'true')
  await expect(box.getByRole('checkbox', { name: /Epic 003/ })).toHaveAttribute('aria-checked', 'false')
  // The box ticks another, and its funnel is pressed.
  await box.getByRole('checkbox', { name: /Epic 003/ }).click()
  await page.keyboard.press('Escape')
  await expect(funnel(page, 'ep-003-aa')).toHaveAttribute('aria-pressed', 'true')
  // A second click on a funnel takes its epic out.
  await funnel(page, 'ep-001-aa').click()
  await expect(funnel(page, 'ep-001-aa')).toHaveAttribute('aria-pressed', 'false')
  await expect(bar(page).getByRole('button', { name: 'Epic: 2 epics' })).toBeVisible()
  await funnel(page, 'ep-002-aa').click()
  await funnel(page, 'ep-003-aa').click()
  await expect(bar(page).getByRole('button', { name: 'Epic: Any' })).toBeVisible()
  await expect(cards(page).first()).toBeVisible()
})

test('a pick and a funnel are separate: a filtered board still lights the picked epic', async ({ openFake }) => {
  const { page } = await open(openFake)
  await funnel(page, 'ep-001-aa').click()
  await pick(page, 'ep-001-aa').click()
  await expect(lit(page)).toHaveCount(2)
  await expect(cards(page)).toHaveCount(2)
  await expect(litLine(page)).toContainText('2 cards lit')
  await expect(funnel(page, 'ep-001-aa')).toHaveAttribute('aria-pressed', 'true')
  await expect(pick(page, 'ep-001-aa')).toHaveAttribute('aria-pressed', 'true')
})
