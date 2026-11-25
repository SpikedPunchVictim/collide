// The Field's live updates in Chrome over a fake owner (ts-0182-ad): the daemon's events make the board read again,
// the card that changed flashes once, and nothing is said aloud. Time is the page's own fake clock
// (`page.clock`), so the debounce and the flash are stepped by the test and nothing waits for real time.
import { type Page } from '@playwright/test'
import { expect, test } from './support/fixtures.ts'

const KEY = 'plan:ts-0001-aa'
const DEBOUNCE = 300
const FLASH = 2100

function column(page: Page, name: string) {
  return page.getByRole('region', { name, exact: true })
}

/** A card on the board. The drawer's links carry the same words, so a card is found by what a card is. */
function card(page: Page, title: string) {
  return page.locator('[data-card]', { hasText: title })
}

/** The board's reads so far. A read of the whole board is one `task.list`. */
function reads(page: Page): Promise<number> {
  return page.evaluate(() => window.collideFake.calls().filter((method) => method === 'task.list').length)
}

/** Puts the page's clock in the test's hands: it stands still until `runFor` moves it. */
async function freeze(page: Page) {
  await page.clock.install({ time: new Date(0) })
  await page.clock.pauseAt(new Date(1000))
}

/** Opens the Field, waits for its first read, and freezes the page's clock. */
async function open(openFake: (world: string) => Promise<Page>, world = 'field-one-source') {
  const page = await openFake(world)
  await expect(column(page, 'Ready')).toContainText('Write the parser')
  await freeze(page)
  return page
}

const toActive = { state: { category: 'active', status: 'active' } }

test('a status change moves the card, and the card flashes once with no toast', async ({ openFake }) => {
  const page = await open(openFake)
  await expect(page.getByText(/Live · updated/)).toBeVisible()

  await page.evaluate((over) => window.collideFake.edit('plan:ts-0001-aa', over), toActive)
  await page.evaluate(() => window.collideFake.announce('plan'))
  await page.clock.runFor(DEBOUNCE)
  await expect(column(page, 'Active')).toContainText('Write the parser')
  await expect(column(page, 'Ready')).not.toContainText('Write the parser')

  await expect(page.locator('[data-flash="true"]')).toHaveCount(1)
  await expect(card(page, 'Write the parser')).toHaveAttribute('data-flash', 'true')
  await expect(page.getByRole('alert')).toHaveCount(0)
  await expect(page.locator('[aria-live="assertive"]')).toHaveCount(0)

  await page.clock.runFor(FLASH)
  await expect(page.locator('[data-flash]')).toHaveCount(0)
  await expect(card(page, 'Write the parser')).toBeVisible()
})

test('a burst of events is one read', async ({ openFake }) => {
  const page = await open(openFake)
  const before = await reads(page)
  await page.evaluate((over) => window.collideFake.edit('plan:ts-0001-aa', over), toActive)
  for (let i = 0; i < 8; i++) {
    await page.evaluate(() => window.collideFake.announce('plan'))
    await page.clock.runFor(DEBOUNCE / 10)
  }
  expect(await reads(page), 'nothing is read before the debounce is over').toBe(before)
  await page.clock.runFor(DEBOUNCE)
  await expect(column(page, 'Active')).toContainText('Write the parser')
  expect(await reads(page)).toBe(before + 1)
})

test('an event for a source the board does not have is ignored', async ({ openFake }) => {
  const page = await open(openFake)
  const before = await reads(page)
  await page.evaluate(() => window.collideFake.announce('github', 'github:acme/edge#1'))
  await page.clock.runFor(10 * DEBOUNCE)
  expect(await reads(page)).toBe(before)
})

test('the selection, a fold and the scroll stay while the board is read again', async ({ openFake }) => {
  const page = await openFake('field-one-source')
  await page.setViewportSize({ width: 700, height: 800 })
  await expect(column(page, 'Ready')).toContainText('Write the parser')
  await page.getByRole('button', { name: /^Done\s*2$/ }).click()
  await page.getByRole('button', { name: 'Fold Review' }).click()
  await card(page, 'Design the board').click()
  const columns = page.getByLabel('Columns')
  await columns.evaluate((each) => each.scrollTo({ left: 250 }))
  const left = await columns.evaluate((each) => each.scrollLeft)
  expect(left, 'the board is wider than the window').toBeGreaterThan(0)
  await freeze(page)

  await page.evaluate((over) => window.collideFake.edit('plan:ts-0001-aa', over), toActive)
  await page.evaluate(() => window.collideFake.announce('plan'))
  await page.clock.runFor(DEBOUNCE)
  await expect(card(page, 'Write the parser')).toHaveAttribute('data-flash', 'true')

  await expect(card(page, 'Design the board')).toHaveAttribute('aria-current', 'true')
  await expect(page.getByRole('complementary', { name: 'Task drawer' })).toContainText('Design the board')
  await expect(column(page, 'Done')).toBeVisible()
  await expect(column(page, 'Review')).toHaveCount(0)
  expect(await columns.evaluate((each) => each.scrollLeft)).toBe(left)
})

test('the board is read again when the connection to the daemon comes back', async ({ openFake }) => {
  const page = await open(openFake)
  const before = await reads(page)
  await page.evaluate((over) => window.collideFake.edit('plan:ts-0001-aa', over), toActive)
  await page.evaluate(() => window.collideFake.connection('back'))
  await page.clock.runFor(DEBOUNCE)
  await expect(column(page, 'Active')).toContainText('Write the parser')
  expect(await reads(page)).toBe(before + 1)
})
