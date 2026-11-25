// The run order and the graph in Chrome over a fake owner (ts-0187-hb): the layout switch and where a view starts, the
// goal picker and the walk it asks the daemon for, a hover or a focus lighting the lines of a task, and a cycle. The
// page plays the walk world (`test/support/field-world.ts`, `walkWorld`) under the built content security policy.
import { type Page } from '@playwright/test'
import { expect, test } from './support/fixtures.ts'

const FIELD_EPIC = 'plan:ep-006-wq'

function layout(page: Page, word: string) {
  return page.getByRole('group', { name: 'Layout' }).getByRole('button', { name: word, exact: true })
}

function column(page: Page, name: string) {
  return page.getByRole('region', { name, exact: true })
}

/** The tasks of the run order or the graph. The drawer's links carry the same words, so a task is found by its kind. */
function task(page: Page, title: string) {
  return page.locator('[data-card]', { hasText: title })
}

/** Opens the walk world, and the view named. The page watches for what the content security policy refuses. */
async function open(openFake: (world: string, watch: (page: Page) => void) => Promise<Page>, view = 'everything') {
  const refused: string[] = []
  const page = await openFake('field-walk', (made) =>
    made.on('console', (message) => {
      if (/Content Security Policy/i.test(message.text())) refused.push(message.text())
    }),
  )
  await expect(column(page, 'Ready')).toBeVisible()
  await pickView(page, view)
  return { page, refused }
}

async function pickView(page: Page, name: string, from = 'All tasks') {
  await page.getByRole('button', { name: from }).click()
  await page.getByRole('button', { name: new RegExp(`^${name}`) }).click()
  await expect(page.getByRole('heading', { name })).toBeVisible()
}

test('the board is where a view starts, and the switch shows the run order, then the graph', async ({ openFake }) => {
  const { page } = await open(openFake)
  await expect(layout(page, 'Board')).toHaveAttribute('aria-pressed', 'true')
  await expect(column(page, 'Ready')).toBeVisible()

  await layout(page, 'Run order').click()
  await expect(layout(page, 'Run order')).toHaveAttribute('aria-pressed', 'true')
  await expect(column(page, 'Ready')).toHaveCount(0)
  await expect(page.getByText('Step 1', { exact: true })).toBeVisible()

  await layout(page, 'Graph').click()
  await expect(page.getByLabel('Graph', { exact: true })).toBeVisible()
  await expect(page.getByText('Step 1 · can start now')).toBeVisible()

  await layout(page, 'Board').click()
  await expect(column(page, 'Ready')).toBeVisible()
})

test('a view opens in its own layout, and the choice is kept for that view and no other', async ({ openFake }) => {
  const { page } = await open(openFake, 'graph-first')
  await expect(layout(page, 'Graph')).toHaveAttribute('aria-pressed', 'true')
  await expect(page.getByLabel('Graph', { exact: true })).toBeVisible()

  await layout(page, 'Board').click()
  await expect(column(page, 'Ready')).toBeVisible()
  await pickView(page, 'run-first', 'graph-first')
  await expect(layout(page, 'Run order')).toHaveAttribute('aria-pressed', 'true')

  await page.reload()
  await expect(column(page, 'Ready')).toBeVisible()
  await pickView(page, 'graph-first')
  await expect(layout(page, 'Board')).toHaveAttribute('aria-pressed', 'true')
  await pickView(page, 'run-first', 'graph-first')
  await expect(layout(page, 'Run order')).toHaveAttribute('aria-pressed', 'true')
})

test('the goal picker lists the collections with open tasks, and a pick asks the daemon for its walk', async ({
  openFake,
}) => {
  const { page } = await open(openFake)
  await layout(page, 'Run order').click()
  const goal = page.getByRole('combobox', { name: 'Toward' })
  await expect(goal.locator('option')).toHaveText(['The Field', 'Later work', 'Tied in a knot'])
  await expect(goal).toHaveValue(FIELD_EPIC)
  await expect(page.getByText('5 open tasks in 4 steps.')).toBeVisible()
  await expect(task(page, 'Write the parser')).toBeVisible()
  await expect(task(page, 'Plan the drawer')).toHaveCount(0)

  await goal.selectOption({ label: 'Later work' })
  await expect(page.getByText('1 open task in 1 step.')).toBeVisible()
  await expect(task(page, 'Plan the drawer')).toBeVisible()
  await expect(task(page, 'Write the parser')).toHaveCount(0)

  // The daemon was asked for the view with the goal's closure added, as an inline view.
  const asked = await page.evaluate(() => window.collideFake.views())
  expect(asked).toContainEqual({
    type: 'inline',
    view: { name: 'everything', closure: { goal: 'plan:ep-007-xx', follow: 'depends_on' } },
  })
})

test('a view with a goal of its own names it and offers no pick', async ({ openFake }) => {
  const { page } = await open(openFake, 'to-the-wire')
  await layout(page, 'Run order').click()
  await expect(page.getByRole('group', { name: 'Goal' })).toContainText('Review the wire')
  await expect(page.getByRole('combobox')).toHaveCount(0)
  await expect(page.getByText('5 open tasks in 4 steps.')).toBeVisible()
  // It asks for no walk of its own: the view is the walk.
  const asked = await page.evaluate(() => window.collideFake.views())
  expect(asked.filter((view) => (view as { type?: string } | undefined)?.type === 'inline')).toEqual([])
})

test('the run order numbers its steps, and a card opens its task in the drawer', async ({ openFake }) => {
  const { page } = await open(openFake, 'to-the-wire')
  await layout(page, 'Run order').click()
  for (const [step, words] of [['Step 1', 'can start now'], ['Step 2', 'after step 1'], ['Step 4', 'after step 3']]) {
    await expect(page.getByText(step!, { exact: true })).toBeVisible()
    await expect(page.getByText(words!, { exact: true }).first()).toBeVisible()
  }
  await task(page, 'Build the Field').click()
  const drawer = page.getByRole('complementary', { name: 'Task drawer' })
  await expect(drawer).toContainText('Build the Field')
  await expect(task(page, 'Build the Field')).toHaveAttribute('aria-current', 'true')
})

test('a hover lights the lines of a task and dims the rest, and leaving puts them back', async ({ openFake }) => {
  const { page, refused } = await open(openFake, 'to-the-wire')
  await layout(page, 'Graph').click()
  await expect(page.locator('svg path[data-from]')).toHaveCount(5)
  await expect(page.locator('[data-hot="true"]')).toHaveCount(0)

  await task(page, 'Build the Field').hover()
  await expect(page.locator('path[data-hot="true"]')).toHaveCount(3)
  // Only the task with no line to it is dimmed: the tasks at the ends of the lit lines are not.
  await expect(page.locator('[data-dim="true"]')).toHaveCount(1)
  await expect(task(page, 'Draw the tabs')).toHaveAttribute('data-dim', 'true')

  await page.mouse.move(0, 0)
  await expect(page.locator('[data-hot="true"]')).toHaveCount(0)
  await expect(page.locator('[data-dim="true"]')).toHaveCount(0)
  expect(refused).toEqual([])
})

test('the focus lights the lines as a hover does, and a click opens the task', async ({ openFake }) => {
  const { page } = await open(openFake, 'to-the-wire')
  await layout(page, 'Graph').click()
  await task(page, 'Write the parser').focus()
  await expect(page.locator('path[data-hot="true"]')).toHaveCount(2)
  await expect(page.locator('[data-dim="true"]')).toHaveCount(2)
  await task(page, 'Write the parser').blur()
  await expect(page.locator('[data-hot="true"]')).toHaveCount(0)

  await task(page, 'Review the wire').click()
  await expect(page.getByRole('complementary', { name: 'Task drawer' })).toContainText('Review the wire')
})

test('a cycle is named above the graph, with no line among its tasks', async ({ openFake }) => {
  const { page } = await open(openFake)
  await layout(page, 'Graph').click()
  await page.getByRole('combobox', { name: 'Toward' }).selectOption({ label: 'Tied in a knot' })
  const note = page.getByRole('status', { name: 'A cycle' })
  await expect(note).toContainText('wait on each other')
  await expect(note).toContainText('Chicken')
  await expect(note).toContainText('Egg')
  await expect(note).not.toContainText('Omelette')
  await expect(page.locator('svg path[data-from]')).toHaveCount(1)
  await expect(page.locator('svg path[data-from]')).toHaveAttribute('data-to', 'plan:ts-0012-ll')
})
