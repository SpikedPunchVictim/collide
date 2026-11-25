// A board whose folder moved, over a real daemon (ts-0191-nv): the page lists it as missing, shows the missing page
// with the folder Collide looked in, and locating the moved folder brings the board back with the same id and its
// tasks. A folder of another repository is refused with the daemon's reason. The page is the bridge's, so it has the
// typed path and no folder picker. Nothing is deleted: the moved folder is still there at each step.
import { mkdirSync, renameSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { expect, test } from './support/fixtures.ts'
import { firstPhase } from './support/recordings.ts'
import { type World, git } from './support/world.ts'

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
  const dir = join(world.root, 'other')
  mkdirSync(dir)
  git(dir, 'init', '-q', '-b', 'main')
  git(dir, 'config', 'user.name', 'test')
  git(dir, 'config', 'user.email', 'test@example.invalid')
  writeFileSync(join(dir, 'file.txt'), 'something else entirely\n')
  git(dir, 'add', '.')
  git(dir, 'commit', '-q', '-m', 'start')
  return dir
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
