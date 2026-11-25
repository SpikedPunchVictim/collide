// The Field over a real daemon (ts-0181-ft): a temp repository with a pack that declares eight statuses (the host's
// corpus fixture) and plan tasks, in a world of its own (`support/world.ts`). The page is read by role and name. What
// the daemon holds is read through the owner's protocol, and the command line's answer is what `collide tasks` prints,
// run by the test with the environment of the world and nothing else, never the person's.
import { type Page } from '@playwright/test'
import { execFileSync } from 'node:child_process'
import { expect, test } from './support/fixtures.ts'
import { type Listing, WITHIN, declaration, fill, openBoard, start, until } from './support/octo-board.ts'
import { COLLIDE_EXE } from './support/world.ts'

function column(page: Page, name: string) {
  return page.getByRole('region', { name, exact: true })
}

/** The columns that are open, in order. */
function columns(page: Page): Promise<(string | null)[]> {
  return page
    .locator('section[data-column]')
    .evaluateAll((all) => all.map((each) => each.getAttribute('aria-label')))
}

async function chooseView(page: Page, name: string) {
  await page.getByRole('button', { name: 'All tasks', exact: true }).click()
  await page.getByRole('button', { name: new RegExp(`^${name}`) }).click()
  await expect(page.getByRole('heading', { name })).toBeVisible()
}

test('the columns are the declared statuses of one source, and the six categories of a mixed board', async ({
  startWorld,
  openPage,
}) => {
  const world = await start(startWorld)
  fill(world)
  await openBoard(world)
  const page = await openPage(world)

  // The whole board has two sources, so its columns are the categories; Done and Dropped start folded.
  await expect(column(page, 'Backlog')).toBeVisible()
  expect(await columns(page)).toEqual(['Backlog', 'Ready', 'Active', 'In review'])
  await expect(page.getByRole('button', { name: /^Done\s*\d+$/ })).toBeVisible()

  // A view over the eight-status source alone has that source's own statuses, in its order, each with its category.
  await chooseView(page, 'octo-only')
  await expect(column(page, 'Idea')).toBeVisible()
  expect(await columns(page)).toEqual(['Idea', 'Triage', 'Queued', 'Doing', 'In review', 'Qa'])
  await expect(column(page, 'Idea')).toContainText('counts as backlog')
  await expect(column(page, 'Qa')).toContainText('counts as in review')
  await expect(page.getByRole('button', { name: /^Done\s*1$/ })).toBeVisible()
  await expect(page.getByRole('button', { name: /^Dropped\s*0$/ })).toBeVisible()
  await expect(column(page, 'Qa').getByRole('button', { name: /Check it twice/ })).toBeVisible()
})

test('a rename in the declaration restarts the source and changes the columns', async ({ startWorld, openPage }) => {
  const world = await start(startWorld)
  fill(world)
  const board = await openBoard(world)
  const page = await openPage(world)
  await chooseView(page, 'octo-only')
  await expect(column(page, 'Triage')).toBeVisible()

  await world.rpc('config.set', {
    key: 'octo.declare',
    value: declaration({ triage: 'sorting' }),
    layer: 'user',
    board,
  })
  await until('the source declares the new name', async () => {
    const list = await world.rpc<Listing>('task.list', { board })
    const eight = list.sources.find((source) => source.name === 'eight')
    return eight?.statuses.some((status) => status.name === 'sorting') && !list.refreshing ? list : undefined
  })

  // The Field reads when a board or a view is chosen, so a person looks again.
  await page.reload()
  await chooseView(page, 'octo-only')
  await expect(column(page, 'Sorting')).toBeVisible()
  expect(await columns(page)).toEqual(['Idea', 'Sorting', 'Queued', 'Doing', 'In review', 'Qa'])
  await expect(column(page, 'Triage')).toHaveCount(0)
  await expect(column(page, 'Qa').getByRole('button', { name: /Check it twice/ })).toBeVisible()
})

test('the MVP view shows the tasks that `collide tasks --view edge-mvp --json` lists', async ({
  startWorld,
  openPage,
}) => {
  const world = await start(startWorld)
  fill(world)
  await openBoard(world)

  // The command, run as a person runs it in the repository, with the environment of the world and nothing else.
  const printed = execFileSync(COLLIDE_EXE, ['tasks', '--view', 'edge-mvp', '--json'], {
    cwd: world.repo,
    env: { PATH: '/usr/bin:/bin', HOME: world.root, COLLIDE_DAEMON_ROOT: world.root },
    encoding: 'utf8',
    timeout: WITHIN,
  })
  const document = JSON.parse(printed) as { tasks: { key: string }[] }
  const expected = document.tasks.map((task) => task.key).sort()
  expect(expected.length, 'the view holds tasks').toBeGreaterThan(0)
  expect(expected).toContain('plan:ts-0004-dd')
  expect(expected).not.toContain('plan:ts-0002-bb')

  const page = await openPage(world)
  await chooseView(page, 'edge-mvp')
  await expect(page.getByText(`${expected.length} task${expected.length === 1 ? '' : 's'}`)).toBeVisible()
  const shown = await page
    .locator('[data-card]')
    .evaluateAll((all) => all.map((each) => (each.getAttribute('data-card') ?? '').split('|')[0]!))
  expect(shown.sort()).toEqual(expected)
  // The view names an item that is no task, and the line above the board says so.
  await expect(page.getByRole('status', { name: 'About this view' })).toContainText('github:acme/edge#212')
})
