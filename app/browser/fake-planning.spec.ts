// The Planning tab in Chrome over a fake owner (ts-0195-ju): the tiles, the proposals with the branch-only chip, the
// features by section, the readiness of the proposal picked, the drawer's planning parts, and that nothing on the tab
// or in the drawer writes. The page plays the scripted `planning` world (`test/support/planning-world.ts`); what is on
// the page is read by role and name, as a person reads it.
import { type Page } from '@playwright/test'
import { expect, test } from './support/fixtures.ts'

async function open(openFake: (world: string) => Promise<Page>) {
  const page = await openFake('planning')
  await expect(page.getByRole('region', { name: 'Field' })).toBeVisible()
  await page.keyboard.press('4')
  await expect(page.getByRole('heading', { name: 'Planning', level: 1 })).toBeVisible()
  return page
}

const drawerOf = (page: Page) => page.getByRole('complementary', { name: 'Task drawer' })
const proposal = (page: Page, title: string) => page.locator('[data-proposal]', { hasText: title })

/** The methods the window may call while the tab and the drawer are used: each one only reads. */
const READS = new Set([
  // The window's own reads at start.
  'board.list',
  'daemon.agent_versions',
  'queue.list',
  'run.list',
  // The Field's, the tab's and the drawer's.
  'task.list',
  'task.get',
  'task.views',
  'task.sections',
  'task.changes',
])

/** The boxes, menus and text fields on the tab and in the drawer that a person could change: none is on. */
function liveFields(page: Page): Promise<string[]> {
  const scope = ['section[aria-label="Planning"]', 'aside[aria-label="Task drawer"]']
  const fields = scope.map((each) => `${each} :is(input, select, textarea)`).join(', ')
  return page.evaluate(
    (selector) =>
      [...document.querySelectorAll<HTMLInputElement>(selector)]
        .filter((each) => !each.disabled)
        .map((each) => `${each.tagName.toLowerCase()} ${each.getAttribute('aria-label') ?? ''}`),
    fields,
  )
}

test('the tab shows a tile for each epic, the proposals and the branch-only chip', async ({ openFake }) => {
  const page = await open(openFake)
  const planning = page.getByRole('region', { name: 'Planning' })
  await expect(planning.getByText('read only', { exact: true })).toBeVisible()
  await expect(planning.locator('[data-epic]')).toHaveCount(2)
  await expect(planning.locator('[data-epic="plan:ep-001-aa"]')).toContainText('1 of 5 done · next: Show each copy')
  await expect(proposal(page, 'Board UI before GitHub')).toContainText('draft · 3 sections, 4 features')
  await expect(proposal(page, 'Loose ideas')).toContainText('only on branch plan/ui-first')
  await expect(planning.getByText('only on branch')).toHaveCount(1)
})

test('the features are a row each, and the near-duplicates say what they look like', async ({ openFake }) => {
  const page = await open(openFake)
  await expect(page.locator('tr[data-feature]')).toHaveCount(7)
  await expect(page.locator('tr[data-section="Field"]')).toHaveCount(2)
  await expect(page.locator('tr[data-section="Setting"]')).toContainText('looks like Settings')
  await expect(page.locator('tr[data-section="Settings"]')).toContainText('looks like Setting')
  await expect(page.locator('tr[data-section="Unsorted"]')).toHaveCount(1)
})

test('Readiness follows the proposal picked, and a pick opens the proposal in the drawer', async ({ openFake }) => {
  const page = await open(openFake)
  const readiness = page.locator('[data-readiness]')
  await expect(proposal(page, 'Board UI before GitHub')).toHaveAttribute('aria-pressed', 'true')
  await expect(page.getByText('For Board UI before GitHub.')).toBeVisible()
  await proposal(page, 'Loose ideas').click()
  await expect(proposal(page, 'Loose ideas')).toHaveAttribute('aria-pressed', 'true')
  await expect(page.getByText('For Loose ideas.')).toBeVisible()
  await expect(page.getByText('line 9: a feature has no title')).toBeVisible()
  await expect(readiness).toHaveCount(5)
  await expect(drawerOf(page).getByRole('heading', { level: 2, name: 'Loose ideas' })).toBeVisible()
})

test('the drawer of a proposal names an incoming link by its inverse, and a link opens its task', async ({
  openFake,
}) => {
  const page = await open(openFake)
  await proposal(page, 'Board UI before GitHub').click()
  const drawer = drawerOf(page)
  const links = drawer.locator('[data-part="links"]')
  await expect(links).toContainText('implemented by')
  await expect(links).toContainText('informed by')
  await expect(drawer.locator('[data-part="coverage"]')).toContainText('F2 has no task')
  await links.getByRole('button', { name: /Draw the columns/ }).click()
  await expect(drawer.getByRole('heading', { level: 2, name: 'Draw the columns' })).toBeVisible()
  await expect(drawer.locator('[data-part="links"]')).toContainText('implements')
})

test('the drawer of a task has its checklist, in boxes that are off', async ({ openFake }) => {
  const page = await open(openFake)
  await page.locator('tr[data-feature="plan:pr-0001-aa#F3"]').getByRole('button', { name: '0003-cc' }).click()
  const drawer = drawerOf(page)
  await expect(drawer.getByRole('heading', { level: 2, name: 'Build the drawer' })).toBeVisible()
  await expect(drawer.locator('[data-part="checklist"]')).toContainText('by ana, 2026-11-20')
  await expect(drawer.locator('[data-part="checklist"] input')).toHaveCount(4)
})

test('nothing on the tab or in the drawer writes', async ({ openFake }) => {
  const page = await open(openFake)
  const planning = page.getByRole('region', { name: 'Planning' })
  await expect(planning.getByRole('button', { name: 'Plan…' })).toBeDisabled()
  await expect(planning.getByText('Planning sessions arrive in Stage 9d.')).toHaveCount(1)
  await expect(planning.getByRole('button', { name: /Sort|Merge/i })).toHaveCount(0)
  // Walk the tab and the drawer's planning parts: a proposal, a task with a checklist, a spike with claims.
  await proposal(page, 'Board UI before GitHub').click()
  await expect(drawerOf(page).locator('[data-part="coverage"]')).toBeVisible()
  await page.locator('tr[data-feature="plan:pr-0001-aa#F3"]').getByRole('button', { name: '0003-cc' }).click()
  await expect(drawerOf(page).locator('[data-part="checklist"]')).toBeVisible()
  expect(await liveFields(page), 'no box, menu or field is on').toEqual([])
  await expect(page.locator('[data-part="checklist"] input, [data-readiness] input')).toHaveCount(9)
  const called = await page.evaluate(() => window.collideFake.calls())
  const wrote = called.filter((method) => !READS.has(method))
  expect(wrote, 'every method the tab and the drawer called only reads').toEqual([])
  expect(called).toContain('task.sections')
})
