// A board whose folder moved, over a real daemon (ts-0191-nv): the page lists it as missing, shows the missing page
// with the folder Collide looked in, and locating the moved folder brings the board back with the same id and its
// tasks. A folder of another repository is refused with the daemon's reason. The page is the bridge's, so it has the
// typed path and no folder picker. Nothing is deleted: the moved folder is still there at each step. The last test is
// the board a window opens on (ts-0218-sc): the one picked last, which a reload keeps.
import { renameSync } from 'node:fs'
import { join } from 'node:path'
import { expect, test } from './support/fixtures.ts'
import { firstPhase } from './support/recordings.ts'
import { type World, git, latestBoard, repository } from './support/world.ts'

/** The id of the board the world's repository opens as, with a scratch task on it. */
async function boardWithATask(world: World): Promise<string> {
  const opened = await world.rpc<{ board: { id: string } }>('board.open', { root: world.repo, init_git: false })
  const board = opened.board.id
  await world.rpc('task.apply', {
    board,
    source: 'scratch',
    apply: { op: 'create', args: { prompt: 'Outlives the move' } },
    commit: false,
  })
  return board
}

/** Another repository, with a commit of its own, in the world's root. */
function otherRepository(world: World): string {
  return repository(world, 'other', 'something else entirely\n')
}

test('a moved folder shows as missing and locating it brings the board back, with its id and its tasks', async ({
  startWorld,
  openPage,
}) => {
  const world = await startWorld({ first: firstPhase([]) })
  const board = await boardWithATask(world)
  const moved = join(world.root, 'moved')
  renameSync(world.repo, moved)
  const page = await openPage(world)

  const missing = page.getByLabel('Board folder not found')
  await expect(missing).toBeVisible()
  await expect(missing.locator('code')).toHaveText(world.repo)
  await expect(missing).toContainText('Nothing has been deleted.')
  await page.getByRole('button', { name: /repo|Boards/ }).first().click()
  await expect(page.getByRole('dialog', { name: 'Open boards' })).toContainText('folder not found')
  await page.keyboard.press('Escape')

  // A repository that is not the board's is refused, and nothing changes.
  const other = otherRepository(world)
  await missing.getByRole('button', { name: 'Locate the folder…' }).click()
  const window = page.getByRole('dialog', { name: /^Locate/ })
  await window.getByLabel('Where is the folder now?').fill(other)
  await window.getByRole('button', { name: 'Use this folder' }).click()
  await expect(window.getByRole('alert')).toContainText('different repository')
  await expect(missing).toBeVisible()

  // The moved folder is the same repository.
  await window.getByLabel('Where is the folder now?').fill(moved)
  await window.getByRole('button', { name: 'Use this folder' }).click()
  await expect(window).toBeHidden()
  await expect(missing).toBeHidden()
  const listed = await world.rpc<{ boards: { id: string; root: string; missing?: boolean }[] }>('board.list')
  expect(listed.boards).toEqual([expect.objectContaining({ id: board, root: moved })])
  expect(listed.boards[0]!.missing).not.toBe(true)
  const task = await world.rpc<{ task: { title: string } }>('task.get', { board, id: 'scratch:1' })
  expect(task.task.title).toBe('Outlives the move')
})

// ---- the board a window opens on (ts-0218-sc) -----------------------------------------------------------------------

test('a board picked in the menu is the one a reload opens on, not the first or the last opened', async ({
  startWorld,
  openPage,
}) => {
  const world = await startWorld({ first: firstPhase([]) })
  await world.rpc('board.open', { root: world.repo })
  const second = (await world.rpc<{ board: { id: string } }>('board.open', { root: otherRepository(world) })).board.id
  // A third folder, picked last: it is the board a window opens on until another pick. Opening marks nothing.
  const third = repository(world, 'third', 'a third thing entirely\n')
  const thirdId = (await world.rpc<{ board: { id: string } }>('board.open', { root: third })).board.id
  expect(await latestBoard(world)).toBeUndefined()
  await world.rpc('board.pick', { board: thirdId })
  const page = await openPage(world)
  const button = page.getByTitle('Board and code line')
  await expect(button).toContainText('third')

  await button.click()
  await page.getByRole('dialog', { name: 'Open boards' }).getByRole('button', { name: /^other/ }).click()
  await expect(button).toContainText('other')
  await expect.poll(() => latestBoard(world)).toBe(second)

  await page.reload()
  await expect(button).toContainText('other')
})

test('a folder opened in the window is picked, so a reload opens on it', async ({ startWorld, openPage }) => {
  const world = await startWorld({ first: firstPhase([]) })
  const first = (await world.rpc<{ board: { id: string } }>('board.open', { root: world.repo })).board.id
  await world.rpc('board.pick', { board: first })
  const folder = otherRepository(world)
  const page = await openPage(world)
  const button = page.getByTitle('Board and code line')
  await expect(button).toContainText('repo')

  await button.click()
  const menu = page.getByRole('dialog', { name: 'Open boards' })
  await menu.getByLabel('Folder path').fill(folder)
  await menu.getByRole('button', { name: 'Open folder' }).click()
  await page.getByRole('button', { name: 'Open the board' }).click()
  await expect(button).toContainText('other')
  await expect.poll(() => latestBoard(world)).not.toBe(first)

  await page.reload()
  await expect(button).toContainText('other')
})

test('a board.open with no pick, as the command line does, leaves the board a window opens on alone', async ({
  startWorld,
  openPage,
}) => {
  const world = await startWorld({ first: firstPhase([]) })
  await world.rpc('board.open', { root: world.repo })
  const other = (await world.rpc<{ board: { id: string } }>('board.open', { root: otherRepository(world) })).board.id
  await world.rpc('board.pick', { board: other })
  // Opening the first board again would, if it marked, make it the latest and the first listed too.
  await world.rpc('board.open', { root: world.repo })
  expect(await latestBoard(world)).toBe(other)

  const page = await openPage(world)
  await expect(page.getByTitle('Board and code line')).toContainText('other')
})
