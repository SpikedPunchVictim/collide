// The Field in Chrome over a fake owner (ts-0181-ft): folds, selecting a card, and the arrow keys. The page plays a
// scripted world (`test/support/worlds.ts`); what is on the page is read by role and name, as a person reads it.
import { type Page } from '@playwright/test'
import { expect, test } from './support/fixtures.ts'

function column(page: Page, name: string) {
  return page.getByRole('region', { name, exact: true })
}

/** A card on the board. The drawer's links carry the same words, so a card is found by what a card is. */
function card(page: Page, title: string) {
  return page.locator('[data-card]', { hasText: title })
}

/** The place of the card that has the focus: its column's name, and its title. */
function focused(page: Page) {
  return page.evaluate(() => {
    const on = document.activeElement
    return {
      column: on?.closest('section[data-column]')?.getAttribute('aria-label') ?? null,
      title: on?.querySelector('span')?.textContent ?? null,
    }
  })
}

test('the board fills a wide window, as the mock does, inside the page padding', async ({ openFake }) => {
  const page = await openFake('field-one-source')
  await page.setViewportSize({ width: 2000, height: 900 })
  const field = page.getByRole('region', { name: 'Field', exact: true })
  await expect(column(page, 'Ready')).toBeVisible()
  const box = (await field.boundingBox())!
  expect([Math.round(box.x), Math.round(box.x + box.width)]).toEqual([16, 2000 - 16])
})

test('Done and Dropped start folded with their counts, and a click opens one', async ({ openFake }) => {
  const page = await openFake('field-one-source')
  await expect(column(page, 'Ready')).toBeVisible()
  await expect(column(page, 'Done')).toHaveCount(0)
  await expect(column(page, 'Dropped')).toHaveCount(0)
  const done = page.getByRole('button', { name: /^Done\s*2$/ })
  await expect(done).toHaveAttribute('aria-expanded', 'false')
  await expect(page.getByRole('button', { name: /^Dropped\s*1$/ })).toBeVisible()
  await done.click()
  await expect(column(page, 'Done')).toBeVisible()
  await expect(card(page, 'Ship the shell')).toBeVisible()
})

test('a fold is kept across a reload, and is its own for each view', async ({ openFake }) => {
  const page = await openFake('field-one-source')
  await page.getByRole('button', { name: /^Done\s*2$/ }).click()
  await page.getByRole('button', { name: 'Fold Ready' }).click()
  await expect(column(page, 'Ready')).toHaveCount(0)
  await page.reload()
  await expect(column(page, 'Active')).toBeVisible()
  await expect(column(page, 'Done')).toBeVisible()
  await expect(column(page, 'Ready')).toHaveCount(0)
  await expect(page.getByRole('button', { name: /^Ready\s*5$/ })).toBeVisible()
  // The same board under another view keeps its own folds: there Done is still folded.
  await page.getByRole('button', { name: 'All tasks' }).click()
  await page.getByRole('button', { name: /^mine/ }).click()
  await expect(page.getByRole('heading', { name: 'mine' })).toBeVisible()
  await expect(column(page, 'Ready')).toBeVisible()
  await expect(column(page, 'Done')).toHaveCount(0)
})

test('a card is selected by a click and opens the drawer', async ({ openFake }) => {
  const page = await openFake('field-one-source')
  await expect(page.getByRole('complementary', { name: 'Task drawer' })).toHaveCount(0)
  await card(page, 'Design the board').click()
  const drawer = page.getByRole('complementary', { name: 'Task drawer' })
  await expect(drawer).toContainText('Design the board')
  await expect(drawer).toContainText('Read only. Edit the file, or ask an agent; the board follows it.')
  await expect(card(page, 'Design the board')).toHaveAttribute('aria-current', 'true')
})

test('a card is selected by Enter', async ({ openFake }) => {
  const page = await openFake('field-one-source')
  await card(page, 'Build the Field').focus()
  await page.keyboard.press('Enter')
  const drawer = page.getByRole('complementary', { name: 'Task drawer' })
  await expect(drawer).toContainText('Build the Field')
  await expect(card(page, 'Build the Field')).toHaveAttribute('aria-current', 'true')
})

test('arrow keys move between the cards, down a column and across the columns', async ({ openFake }) => {
  const page = await openFake('field-one-source')
  await card(page, 'Write the parser').focus()
  expect(await focused(page)).toEqual({ column: 'Ready', title: 'Write the parser' })
  await page.keyboard.press('ArrowDown')
  expect(await focused(page)).toEqual({ column: 'Ready', title: 'Design the board' })
  await page.keyboard.press('ArrowUp')
  expect((await focused(page)).title).toBe('Write the parser')
  await page.keyboard.press('ArrowUp')
  expect((await focused(page)).title).toBe('Write the parser')
  await page.keyboard.press('ArrowRight')
  expect(await focused(page)).toEqual({ column: 'Active', title: 'Build the Field' })
  await page.keyboard.press('ArrowRight')
  expect(await focused(page)).toEqual({ column: 'Review', title: 'Plan the drawer' })
  // Done and Dropped are folded, so there is no card to the right.
  await page.keyboard.press('ArrowRight')
  expect((await focused(page)).column).toBe('Review')
  await page.keyboard.press('ArrowLeft')
  expect((await focused(page)).column).toBe('Active')
})

test('a key pressed with the focus off the cards does nothing to them', async ({ openFake }) => {
  const page = await openFake('field-one-source')
  await expect(column(page, 'Ready')).toBeVisible()
  await page.keyboard.press('Enter')
  await expect(page.getByRole('complementary', { name: 'Task drawer' })).toHaveCount(0)
})

test('the eight-status source gives its own columns in its own order, each saying what it counts as', async ({
  openFake,
}) => {
  const page = await openFake('field-eight')
  await expect(column(page, 'Idea')).toBeVisible()
  const names = await page.getByRole('region').evaluateAll((all) =>
    all.filter((each) => each.hasAttribute('data-column')).map((each) => each.getAttribute('aria-label')),
  )
  // Done and Dropped are folded, so they show as buttons and not as columns.
  expect(names).toEqual(['Idea', 'Triage', 'Queued', 'Doing', 'In review', 'Qa'])
  await expect(column(page, 'Idea')).toContainText('counts as backlog')
  await expect(column(page, 'Qa')).toContainText('counts as in review')
  await expect(page.getByRole('button', { name: /^Done\s*1$/ })).toBeVisible()
})
