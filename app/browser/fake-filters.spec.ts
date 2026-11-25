// The filter bar in Chrome over a fake owner (ts-0190-tz): chips narrow the board, the header folds and the fold is
// kept, `/` goes to the search, the Epic box over fifty epics, Save as a view, and Share with team. The page plays a
// scripted world (`test/support/worlds.ts`); what is on it is read by role and name, as a person reads it.
import { type Page } from '@playwright/test'
import { expect, test } from './support/fixtures.ts'

const card = (page: Page, title: string) => page.locator('[data-card]', { hasText: title })
const bar = (page: Page) => page.getByRole('region', { name: 'Filters', exact: true })
const chip = (page: Page, name: string) => bar(page).getByRole('button', { name, exact: true })

test('a chip narrows the board to what it names, and Clear filters brings the rest back', async ({ openFake }) => {
  const page = await openFake('field-filters')
  await expect(card(page, 'Write the parser')).toBeVisible()
  await expect(card(page, 'Build the Field')).toBeVisible()
  await chip(page, 'Ready, 3 tasks').click()
  await expect(chip(page, 'Ready, 3 tasks')).toHaveAttribute('aria-pressed', 'true')
  await expect(card(page, 'Write the parser')).toBeVisible()
  await expect(card(page, 'Build the Field')).toHaveCount(0)
  await expect(bar(page)).toContainText('Showing 3 of 7')
  await bar(page).getByRole('button', { name: 'Clear filters' }).click()
  await expect(card(page, 'Build the Field')).toBeVisible()
  await expect(chip(page, 'Ready, 3 tasks')).toHaveAttribute('aria-pressed', 'false')
})

test('the groups are and-ed, the choices of a group or-ed, and Labels and Comes from are chips too', async ({
  openFake,
}) => {
  const page = await openFake('field-filters')
  await chip(page, 'Ready, 3 tasks').click()
  await chip(page, 'Active, 1 task').click()
  await expect(card(page, 'Build the Field')).toBeVisible()
  await chip(page, 'plan, 5 tasks').click()
  await expect(card(page, 'Parse the old notes')).toHaveCount(0)
  await chip(page, 'perf, 2 tasks').click()
  await expect(card(page, 'Write the parser')).toBeVisible()
  await expect(card(page, 'Build the Field')).toHaveCount(0)
  await expect(bar(page)).toContainText('Showing 2 of 7')
  await bar(page).getByText('Advanced: this view as an expression').click()
  await expect(bar(page).locator('code')).toContainText('category in ["ready", "active"]')
  await expect(bar(page).locator('code')).toContainText('"perf" in labels')
})

test('a search finds a title or an id in any case, and keeps a quote in the text a quote', async ({ openFake }) => {
  const page = await openFake('field-filters')
  const search = page.getByRole('searchbox', { name: 'Search tasks' })
  await search.fill('PARSER')
  await expect(card(page, 'Write the parser')).toBeVisible()
  await expect(card(page, 'Build the Field')).toHaveCount(0)
  await search.fill('ts-0003')
  await expect(card(page, 'Build the Field')).toBeVisible()
  await expect(card(page, 'Write the parser')).toHaveCount(0)
  await search.fill('say "hi" \\ (x)')
  await expect(bar(page)).toContainText('Showing 0 of 7')
  await expect(page.getByRole('alert')).toHaveCount(0)
})

test('/ goes to the search, and a / typed into the search is typed', async ({ openFake }) => {
  const page = await openFake('field-filters')
  await expect(card(page, 'Write the parser')).toBeVisible()
  await page.keyboard.press('/')
  const search = page.getByRole('searchbox', { name: 'Search tasks' })
  await expect(search).toBeFocused()
  await expect(search).toHaveValue('')
  await page.keyboard.type('a/b')
  await expect(search).toHaveValue('a/b')
})

test('the header folds the bar to a line of words, / opens it, and the fold is kept across a reload', async ({
  openFake,
}) => {
  const page = await openFake('field-filters')
  await chip(page, 'Active, 1 task').click()
  const header = bar(page).getByRole('button', { name: /^Filters/ })
  await expect(header).toHaveAttribute('aria-expanded', 'true')
  await header.click()
  await expect(header).toHaveAttribute('aria-expanded', 'false')
  await expect(header).toContainText('Where it stands: Active · showing 1 of 7')
  await expect(page.getByRole('searchbox', { name: 'Search tasks' })).toHaveCount(0)
  // The choice still narrows the board while the bar is folded.
  await expect(card(page, 'Write the parser')).toHaveCount(0)
  // The choices are not kept across a reload, the fold is.
  await page.reload()
  await expect(bar(page).getByRole('button', { name: /^Filters/ })).toHaveAttribute('aria-expanded', 'false')
  await expect(bar(page).getByRole('button', { name: /^Filters/ })).toContainText('No filters · 7 tasks')
  await page.keyboard.press('/')
  await expect(page.getByRole('searchbox', { name: 'Search tasks' })).toBeFocused()
  await expect(bar(page).getByRole('button', { name: /^Filters/ })).toHaveAttribute('aria-expanded', 'true')
})

test('the choices are the view\'s: another view starts with none, and the chips count that view', async ({
  openFake,
}) => {
  const page = await openFake('field-filters')
  await chip(page, 'Ready, 3 tasks').click()
  await page.getByRole('button', { name: 'All tasks' }).click()
  await page.getByRole('button', { name: /^open-work/ }).click()
  await expect(page.getByRole('heading', { name: 'open-work' })).toBeVisible()
  await expect(chip(page, 'Ready, 3 tasks')).toHaveAttribute('aria-pressed', 'false')
  await expect(chip(page, 'Done, 0 tasks')).toBeVisible()
  await chip(page, 'Active, 1 task').click()
  await expect(bar(page)).toContainText('Showing 1 of 6')
  // The counts stay what the view holds, as the choices are made.
  await expect(chip(page, 'Ready, 3 tasks')).toBeVisible()
})

test('the Epic box lists fifty epics with a find box, takes a tick, says how many, and clears', async ({
  openFake,
}) => {
  const page = await openFake('field-epics-50')
  await expect(page.getByText('Task of epic 001')).toBeVisible()
  const trigger = bar(page).getByRole('button', { name: 'Epic: Any' })
  await trigger.click()
  const box = page.getByRole('dialog', { name: 'Epics to show' })
  await expect(box.getByRole('checkbox')).toHaveCount(50)
  await box.getByRole('searchbox', { name: 'Find an epic' }).fill('daemon')
  await expect(box.getByRole('checkbox')).toHaveCount(25)
  await box.getByRole('checkbox', { name: /Epic 002/ }).click()
  await expect(box).toContainText('1 chosen')
  await expect(page.getByText('Task of epic 002')).toBeVisible()
  await expect(page.getByText('Task of epic 001')).toHaveCount(0)
  await box.getByRole('checkbox', { name: /Epic 004/ }).click()
  await expect(box).toContainText('2 chosen')
  await page.keyboard.press('Escape')
  await expect(bar(page).getByRole('button', { name: 'Epic: 2 epics' })).toBeVisible()
  await bar(page).getByRole('button', { name: 'Epic: 2 epics' }).click()
  await page.getByRole('button', { name: 'Clear', exact: true }).click()
  await page.keyboard.press('Escape')
  await expect(bar(page).getByRole('button', { name: 'Epic: Any' })).toBeVisible()
  await expect(page.getByText('Task of epic 001')).toBeVisible()
})

test('one epic is named by its title in the box', async ({ openFake }) => {
  const page = await openFake('field-filters')
  await bar(page).getByRole('button', { name: 'Epic: Any' }).click()
  await page.getByRole('checkbox', { name: /The drawer/ }).click()
  await page.keyboard.press('Escape')
  await expect(bar(page).getByRole('button', { name: 'Epic: The drawer' })).toBeVisible()
  await expect(card(page, 'Plan the drawer')).toBeVisible()
  await expect(card(page, 'Write the parser')).toHaveCount(0)
})

test('Save as a view keeps the choices as a view the person can pick, and the board is the same under it', async ({
  openFake,
}) => {
  const page = await openFake('field-filters')
  await chip(page, 'Ready, 3 tasks').click()
  await expect(bar(page)).toContainText('Showing 3 of 7')
  await bar(page).getByRole('button', { name: 'Save as a view…' }).click()
  const dialog = page.getByRole('dialog', { name: 'Save as a view' })
  await expect(dialog.getByRole('textbox', { name: 'Name' })).toHaveValue('My view')
  await expect(dialog).toContainText('The view is kept for you.')
  await dialog.getByRole('textbox', { name: 'Name' }).fill('Ready ones')
  await dialog.getByRole('button', { name: 'Save view' }).click()
  await expect(page.getByRole('heading', { name: 'Ready ones' })).toBeVisible()
  await expect(page.getByText('3 tasks', { exact: true })).toBeVisible()
  await expect(card(page, 'Write the parser')).toBeVisible()
  await expect(card(page, 'Build the Field')).toHaveCount(0)
  await page.getByRole('button', { name: 'Ready ones' }).click()
  await expect(page.getByRole('dialog', { name: 'Views' }).getByRole('button', { name: /^Ready ones/ })).toContainText(
    'yours',
  )
})

test('Save is off over a built-in view, and over a view that names tasks, with the words why', async ({ openFake }) => {
  const page = await openFake('field-filters')
  await page.getByRole('button', { name: 'All tasks' }).click()
  await page.getByRole('button', { name: /^ready/ }).click()
  await chip(page, 'Ready, 3 tasks').click()
  await expect(bar(page).getByRole('button', { name: 'Save as a view…' })).toBeDisabled()
  await expect(bar(page).getByRole('note')).toContainText('Save is off: ready is a built-in view')
  await page.getByRole('button', { name: 'ready' }).first().click()
  await page.getByRole('button', { name: /^picked/ }).click()
  await chip(page, 'Ready, 1 task').click()
  await expect(bar(page).getByRole('note')).toContainText('names tasks or walks to a goal')
})

test('Share with team is on a personal view, writes it to the repo, and then the view is one row, in the repo', async ({
  openFake,
}) => {
  const page = await openFake('field-filters')
  await page.getByRole('button', { name: 'All tasks' }).click()
  await expect(page.getByRole('button', { name: 'Share ready with team' })).toHaveCount(0)
  await expect(page.getByRole('button', { name: 'Share open-work with team' })).toHaveCount(0)
  await page.getByRole('button', { name: 'Share mine with team' }).click()
  await expect(page.getByRole('status').filter({ hasText: 'Shared as repo/mine.yaml' })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Share mine with team' })).toHaveCount(0)
  await expect(page.getByRole('button', { name: /^mine/ })).toContainText('in this repo')
  await expect(page.getByRole('button', { name: /^mine/ })).toHaveCount(1)
})
