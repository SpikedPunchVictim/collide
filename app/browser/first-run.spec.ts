// First run over a real daemon (ts-0194-qp): the checks on a folder with and without `claude` on the daemon's PATH, a
// plain folder made a repository, a fresh repository opened with nothing set up, the first task, Share with team, and a
// clone of what was shared asking before it applies the repo's own Collide folder. The page is the bridge's, so it has
// the typed path and no folder picker. The daemon cannot probe a login under a test root (it answers "could not
// check"), so the logged-in and logged-out rows are the fake-owner spec's (`fake-first-run.spec.ts`).
import { type Page } from '@playwright/test'
import { existsSync, mkdirSync, readFileSync, readdirSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { expect, test } from './support/fixtures.ts'
import { firstPhase } from './support/recordings.ts'
import { type World, git } from './support/world.ts'

// `node` is on the daemon's PATH, as the built-in plan pack's provider needs it to start; without it the board's plan
// source stays unstarted, and a board that is still settling shows no empty page.
const plan = { first: firstPhase([]), path: [dirname(process.execPath)] }
const firstRun = (page: Page) => page.getByLabel('Open a folder', { exact: true })
const check = (page: Page, id: string) => firstRun(page).locator(`[data-check="${id}"]`)

/** Types `folder` into the first-run page (shown when no board is open) and waits for its checks. */
async function look(page: Page, folder: string) {
  await page.getByLabel('Folder to look at').fill(folder)
  await page.getByRole('button', { name: 'Check this folder' }).click()
  await expect(page.getByRole('heading', { name: 'Checks' })).toBeVisible()
  await expect(firstRun(page).getByText('Checking the folder…')).toHaveCount(0)
}

/** Opens `folder` from the board menu of a window that has a board, and waits for its checks. */
async function lookFromMenu(page: Page, folder: string) {
  await page.getByRole('button', { name: /repo|clone|Boards/ }).first().click()
  const menu = page.getByRole('dialog', { name: 'Open boards' })
  await menu.getByLabel('Folder path').fill(folder)
  await menu.getByRole('button', { name: 'Open folder' }).click()
  await expect(page.getByRole('heading', { name: 'Checks' })).toBeVisible()
  await expect(firstRun(page).getByText('Checking the folder…')).toHaveCount(0)
}

/** A folder in the world's root that is not a repository, with one file. */
function plainFolder(world: World): string {
  const dir = join(world.root, 'plain')
  mkdirSync(dir)
  return dir
}

const EMPTY = (page: Page) => page.getByLabel('Empty board')
const SHARE = (page: Page) => page.getByLabel('Share with team', { exact: true })

test('without claude on the PATH the row says so, names the install command, and the board still opens', async ({
  startWorld,
  openPage,
}) => {
  const world = await startWorld({ ...plan, claude: 'none' })
  const page = await openPage(world)
  await look(page, world.repo)
  await expect(check(page, 'git_repo')).toHaveAttribute('data-mark', 'ok')
  const row = check(page, 'claude_installed')
  await expect(row).toHaveAttribute('data-mark', 'no')
  await expect(row).toContainText('Claude Code is not installed')
  await expect(row.locator('code')).toHaveText('curl -fsSL https://claude.ai/install.sh | bash')
  await expect(row.getByRole('button', { name: 'Copy' })).toBeVisible()
  await expect(check(page, 'claude_logged_in')).toHaveCount(0)

  await page.getByRole('button', { name: 'Open the board' }).click()
  await expect(EMPTY(page)).toBeVisible()
})

test('with claude on the PATH the row gives its version, and the login is named as not checked', async ({
  startWorld,
  openPage,
}) => {
  const world = await startWorld(plan)
  const page = await openPage(world)
  await look(page, world.repo)
  await expect(check(page, 'claude_installed')).toHaveAttribute('data-mark', 'ok')
  await expect(check(page, 'claude_installed')).toContainText('2.1.325')
  await expect(check(page, 'claude_logged_in')).toHaveAttribute('data-mark', 'unknown')
  await expect(check(page, 'claude_logged_in')).toContainText('Could not check')
  await expect(page.getByRole('button', { name: 'Open the board' })).toBeEnabled()
})

test('a folder that is not a repository is made one only when the person says so, and nothing else', async ({
  startWorld,
  openPage,
}) => {
  const world = await startWorld(plan)
  const plain = plainFolder(world)
  const page = await openPage(world)
  await look(page, plain)
  await expect(check(page, 'git_repo')).toHaveAttribute('data-mark', 'no')
  await expect(page.getByRole('button', { name: 'Open the board' })).toBeDisabled()
  expect(readdirSync(plain)).toEqual([])

  await check(page, 'git_repo').getByRole('button', { name: 'Make it one…' }).click()
  const dialog = page.getByRole('dialog', { name: 'Make this folder a git repository?' })
  await expect(dialog).toContainText(plain)
  expect(readdirSync(plain)).toEqual([])
  await dialog.getByRole('button', { name: 'Run git init' }).click()
  await expect(EMPTY(page)).toBeVisible()
  expect(readdirSync(plain)).toEqual(['.git'])
})

test('a fresh repository opens with nothing set up, and Start with a task makes a scratch task', async ({
  startWorld,
  openPage,
}) => {
  const world = await startWorld(plan)
  const page = await openPage(world)
  await look(page, world.repo)
  await page.getByRole('button', { name: 'Open the board' }).click()
  const empty = EMPTY(page)
  await expect(empty).toContainText('Nothing on this board yet')
  expect(existsSync(join(world.repo, '.collide'))).toBe(false)
  expect(git(world.repo, 'status', '--porcelain')).toBe('')

  const add = page.getByRole('button', { name: 'Add as a scratch task' })
  await expect(add).toBeDisabled()
  await page.getByLabel(/What needs doing/).fill('the sign-in page forgets where you were going')
  await add.click()
  await expect(EMPTY(page)).toHaveCount(0)
  await expect(page.getByRole('button', { name: /the sign-in page forgets where you were going/ })).toBeVisible()
  expect(existsSync(join(world.repo, '.collide'))).toBe(false)
})

test('Share writes the board file and the saved view, a commit shares them, and a clone asks first', async ({
  startWorld,
  openPage,
}) => {
  const world = await startWorld(plan)
  const page = await openPage(world)
  await look(page, world.repo)
  await page.getByRole('button', { name: 'Open the board' }).click()
  await expect(EMPTY(page)).toBeVisible()
  const board = (await world.rpc<{ boards: { id: string }[] }>('board.list')).boards[0]!.id
  await world.rpc('task.save_view', { board, target: 'personal', view: { name: 'mine', where: 'true' } })

  // The offer lists the files, and writes none until the person chooses.
  await expect(SHARE(page)).toContainText("This board's settings are yours only.")
  await SHARE(page).getByRole('button', { name: 'Share with team' }).click()
  const dialog = page.getByRole('dialog', { name: 'Share this board with your team?' })
  await expect(dialog.getByRole('list', { name: 'Files to write' }).getByRole('listitem')).toHaveText([
    '.collide/board.yaml',
    '.collide/views/mine.yaml',
  ])
  expect(existsSync(join(world.repo, '.collide'))).toBe(false)
  await dialog.getByRole('button', { name: 'Write .collide/' }).click()
  await expect(dialog).toBeHidden()
  await expect(SHARE(page)).toContainText('is written in the repo folder')
  expect(readFileSync(join(world.repo, '.collide', 'views', 'mine.yaml'), 'utf8')).toContain('"mine"')
  expect(existsSync(join(world.repo, '.collide', 'config.yaml'))).toBe(false)

  // The person's own commit of what was written asks nothing: Share recorded the trust of those bytes.
  git(world.repo, 'add', '.collide')
  git(world.repo, 'commit', '-q', '-m', 'share the board')
  await page.reload()
  await expect(EMPTY(page)).toBeVisible()
  await expect(SHARE(page)).toHaveCount(0)
  await expect(page.getByRole('dialog', { name: /Trust the .* this repo brings\?/ })).toHaveCount(0)
  await expect(page.getByLabel('Not trusted', { exact: true })).toHaveCount(0)

  // A clone has the Collide folder in its line and no trust of its own: it asks, by what the folder brings.
  const clone = join(world.root, 'clone')
  git(world.root, 'clone', '-q', world.repo, clone)
  await lookFromMenu(page, clone)
  await page.getByRole('button', { name: 'Open the board' }).click()
  const prompt = page.getByRole('dialog', { name: /Trust the .* this repo brings\?/ })
  await expect(prompt).toBeVisible()
  await expect(prompt).toContainText('view')
  await expect(prompt.getByRole('button', { name: 'Open without it' })).toBeVisible()

  // Open without it names what was not applied; the shared view is not listed until the person trusts.
  await prompt.getByRole('button', { name: 'Open without it' }).click()
  const notTrusted = page.getByLabel('Not trusted', { exact: true })
  await expect(notTrusted).toBeVisible()
  await notTrusted.getByRole('button', { name: 'Review and trust…' }).click()
  const again = page.getByRole('dialog', { name: /Trust the .* this repo brings\?/ })
  await again.getByRole('button', { name: 'Trust and open' }).click()
  await expect(notTrusted).toHaveCount(0)
  await page.getByRole('button', { name: 'All tasks' }).click()
  await expect(page.getByRole('dialog', { name: 'Views' }).getByRole('button', { name: /^mine/ })).toContainText(
    'in this repo',
  )
})
