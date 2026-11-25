// The filter bar and saved views over a real daemon (ts-0190-tz): the board under the bar's choices is `task.list` over
// the bar's expression, for every kind of view; a saved view lists what the board showed, as `collide tasks` prints it;
// and a personal view is shared into the repo's Collide folder. The repository is the one of `field.spec.ts`, with
// labels on some of its tasks. The command runs with the environment of the world and nothing else, never the person's.
import { type Page } from '@playwright/test'
import { execFileSync } from 'node:child_process'
import { existsSync, readFileSync } from 'node:fs'
import { join } from 'node:path'
import { expect, test } from './support/fixtures.ts'
import { type Listing, WITHIN, fill, openBoard, start, trust, until } from './support/octo-board.ts'
import { COLLIDE_EXE, type World, git } from './support/world.ts'

const LABELS = { '1': ['perf'], '2': ['perf'], '5': ['ux'] }

const bar = (page: Page) => page.getByRole('region', { name: 'Filters', exact: true })

/** The keys of the cards on the page, sorted, with the folded columns (Done and Dropped) opened. */
async function shown(page: Page): Promise<string[]> {
  const folded = page.locator('button[data-column][aria-expanded="false"]')
  while ((await folded.count()) > 0) await folded.first().click()
  const ids = await page
    .locator('[data-card]')
    .evaluateAll((all) => all.map((each) => (each.getAttribute('data-card') ?? '').split('|')[0]!))
  return ids.sort()
}

/** The keys the daemon lists for a view, narrowed by `filter` when one is given, sorted. */
async function listed(world: World, board: string, view: string | null, filter?: string): Promise<string[]> {
  const answer = await world.rpc<Listing>('task.list', {
    board,
    ...(view === null ? {} : { view: { type: 'named', name: view } }),
    ...(filter === undefined ? {} : { filter }),
  })
  return answer.tasks.map((task) => task.key).sort()
}

/** Picks a view in the picker, or the whole board for null. */
async function chooseView(page: Page, name: string | null) {
  await page.getByTitle('The view of this board').click()
  const row = new RegExp(`^${name ?? 'All tasks'}`)
  await page.getByRole('dialog', { name: 'Views' }).getByRole('button', { name: row }).click()
  await expect(page.getByRole('heading', { name: name ?? 'All tasks' })).toBeVisible()
}

/** The expression the bar shows under Advanced. */
async function expressionOf(page: Page): Promise<string> {
  const details = bar(page).locator('details')
  if ((await details.getAttribute('open')) === null) await details.locator('summary').click()
  return (await details.locator('code').innerText()).trim()
}

test('the labels the tasks carry are a group of chips, and a chip keeps the tasks that carry it', async ({
  startWorld,
  openPage,
}) => {
  const world = await start(startWorld)
  fill(world, LABELS)
  const board = await openBoard(world)
  const page = await openPage(world)
  await expect(bar(page).getByRole('group', { name: 'Labels' })).toBeVisible()
  await bar(page).getByRole('button', { name: 'perf, 2 tasks', exact: true }).click()
  await expect(page.getByText('2 tasks', { exact: true })).toBeVisible()
  expect(await shown(page)).toEqual(['eight:1', 'eight:2'])
  expect(await shown(page)).toEqual(await listed(world, board, null, await expressionOf(page)))
})

test('under every kind of view the board is task.list over the expression, and a search stays text', async ({
  startWorld,
  openPage,
}) => {
  const world = await start(startWorld)
  fill(world, LABELS)
  const board = await openBoard(world)
  const page = await openPage(world)
  const search = page.getByRole('searchbox', { name: 'Search tasks' })
  // A built-in rule, a view with `include` and `exclude`, a view of only `include`, and the whole board.
  for (const view of ['ready', 'edge-mvp', 'octo-only', null]) {
    await chooseView(page, view)
    const members = await listed(world, board, view)
    await search.fill('TH')
    const expression = await expressionOf(page)
    const expected = await listed(world, board, view, expression)
    expect(expected.every((key) => members.includes(key)), `${view}: the filter only narrows the view`).toBe(true)
    const count = `${expected.length} task${expected.length === 1 ? '' : 's'}`
    await expect(page.getByText(count, { exact: true })).toBeVisible()
    expect(await shown(page), `${view} under the search`).toEqual(expected)
    await search.fill('')
  }
  // A task `include` names passes through the filter as any other, and one `exclude` names stays out.
  await chooseView(page, 'edge-mvp')
  await search.fill('drawer')
  await expect(page.getByText('1 task', { exact: true })).toBeVisible()
  expect(await shown(page)).toEqual(['plan:ts-0004-dd'])
  // Quotes, a backslash and brackets in the text are text: the daemon takes the expression and nothing matches.
  await search.fill('say "hi" \\ (x) [y]')
  await expect(page.getByText('0 tasks', { exact: true })).toBeVisible()
  await expect(page.getByText('The filter could not be applied')).toHaveCount(0)
  await expect(page.getByRole('alert')).toHaveCount(0)
})

test('Save as a view lists what the board showed, and Share with team writes it to the repo', async ({
  startWorld,
  openPage,
}) => {
  const world = await start(startWorld)
  fill(world, LABELS)
  const board = await openBoard(world)
  const page = await openPage(world)
  await bar(page).getByRole('button', { name: /^In review, \d+ tasks$/ }).click()
  await page.getByRole('searchbox', { name: 'Search tasks' }).fill('check')
  await expect(page.getByText('2 tasks', { exact: true })).toBeVisible()
  const onTheBoard = await shown(page)
  expect(onTheBoard).toEqual(['eight:2', 'eight:3'])

  await bar(page).getByRole('button', { name: 'Save as a view…' }).click()
  const dialog = page.getByRole('dialog', { name: 'Save as a view' })
  await dialog.getByRole('textbox', { name: 'Name' }).fill('checks')
  await dialog.getByRole('button', { name: 'Save view' }).click()
  await expect(page.getByRole('heading', { name: 'checks' })).toBeVisible()
  await expect(page.getByText('2 tasks', { exact: true })).toBeVisible()
  expect(await shown(page)).toEqual(onTheBoard)
  expect(await listed(world, board, 'checks')).toEqual(onTheBoard)

  // The command, run as a person runs it in the repository, lists the same tasks.
  const printed = execFileSync(COLLIDE_EXE, ['tasks', '--view', 'checks', '--json'], {
    cwd: world.repo,
    env: { PATH: '/usr/bin:/bin', HOME: world.root, COLLIDE_DAEMON_ROOT: world.root },
    encoding: 'utf8',
    timeout: WITHIN,
  })
  const keys = (JSON.parse(printed) as { tasks: { key: string }[] }).tasks.map((task) => task.key).sort()
  expect(keys).toEqual(onTheBoard)

  // The line has a Collide folder, so the view of the person's own can be shared. Then it is one row, in the repo.
  await page.getByRole('button', { name: 'checks', exact: true }).click()
  const views = page.getByRole('dialog', { name: 'Views' })
  await views.getByRole('button', { name: 'Share checks with team' }).click()
  await expect(views.getByRole('status').filter({ hasText: 'Shared as' })).toBeVisible()
  const file = join(world.repo, '.collide', 'views', 'checks.yaml')
  expect(existsSync(file), 'the view is in the repo').toBe(true)
  expect(readFileSync(file, 'utf8')).toContain('where:')
  // The repo's views are read from what is committed, so the daemon lists the copy once the person commits it. Until
  // then the view of the person's own is the one in the picker.
  await expect(views.getByRole('button', { name: 'Share checks with team' })).toHaveCount(1)
  git(world.repo, 'add', '.collide/views/checks.yaml')
  git(world.repo, 'commit', '-q', '-m', 'share the view')
  await trust(world, board)
  await until('the daemon lists the repo copy', async () => {
    const answer = await world.rpc<{ views: { source: { type: string }; view: { name: string } }[] }>('task.views', {
      board,
    })
    return answer.views.filter((each) => each.view.name === 'checks').length === 2 ? answer : undefined
  })
  await page.keyboard.press('Escape')
  await page.getByTitle('The view of this board').click()
  await expect(views.getByRole('button', { name: 'Share checks with team' })).toHaveCount(0)
  await expect(views.getByRole('button', { name: /^checks/ })).toHaveCount(1)
  await expect(views.getByRole('button', { name: /^checks/ })).toContainText('in this repo')
})
