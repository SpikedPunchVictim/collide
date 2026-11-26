// The first-run page, the empty board, Share with team and the trust prompt in Chrome over a fake owner (ts-0194-qp):
// no Tauri and no daemon. The page plays a scripted world (`test/support/first-run-world.ts`), and what the client
// called is read back from the fake (`window.collideFake`). The clipboard is replaced in the page by a recorder, so
// nothing is written to the system's. The real daemon's side is `first-run.spec.ts`.
import { type Page } from '@playwright/test'
import { expect, test } from './support/fixtures.ts'

const INSTALL = 'curl -fsSL https://claude.ai/install.sh | bash'

function calls(page: Page): Promise<string[]> {
  return page.evaluate(() => window.collideFake.calls())
}

function callsTo(page: Page, method: string): Promise<unknown[]> {
  return page.evaluate((name) => window.collideFake.callsTo(name), method)
}

/** Replaces the clipboard's write with a recorder, before the page loads. */
async function recordClipboard(page: Page) {
  await page.addInitScript(() => {
    const copied: string[] = []
    ;(window as unknown as { copied: string[] }).copied = copied
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: {
        writeText: async (text: string) => {
          copied.push(text)
        },
      },
    })
  })
}

function copied(page: Page): Promise<string[]> {
  return page.evaluate(() => (window as unknown as { copied: string[] }).copied)
}

const firstRun = (page: Page) => page.getByLabel('Open a folder', { exact: true })
const check = (page: Page, id: string) => firstRun(page).locator(`[data-check="${id}"]`)

/** Types a folder into the first-run page and looks at it. */
async function look(page: Page, folder: string) {
  await page.getByLabel('Folder to look at').fill(folder)
  await page.getByRole('button', { name: 'Check this folder' }).click()
  await expect(page.getByRole('heading', { name: 'Checks' })).toBeVisible()
  await expect(firstRun(page).getByText('Checking the folder…')).toHaveCount(0)
}

// ---- first launch and the checks ---------------------------------------------------------------------------------

test('with no board open the window asks for a folder, and looks at it before it opens it', async ({ openFake }) => {
  const page = await openFake('first-run')
  await expect(page.getByRole('heading', { name: 'Open a project' })).toBeVisible()
  await expect(page.getByRole('heading', { name: 'Checks' })).toHaveCount(0)

  await look(page, '/work/fresh')
  await expect(check(page, 'git_repo')).toHaveAttribute('data-mark', 'ok')
  await expect(check(page, 'claude_installed')).toContainText('version 2.1.330')
  await expect(check(page, 'claude_logged_in')).toHaveAttribute('data-mark', 'ok')
  await expect(check(page, 'gh_logged_in')).toContainText('Checked when GitHub sources arrive, in Stage 5.')
  await expect(firstRun(page)).toContainText('/work/fresh')
  expect(await calls(page)).not.toContain('board.open')

  await page.getByRole('button', { name: 'Open the board' }).click()
  await expect(page.getByLabel('Empty board')).toBeVisible()
  expect(await callsTo(page, 'board.open')).toEqual([{ root: '/work/fresh' }])
})

test('claude missing is named with its install command, and Copy puts it on the clipboard', async ({
  openFake,
}) => {
  const page = await openFake('first-run', recordClipboard)
  await look(page, '/work/noclaude')
  const row = check(page, 'claude_installed')
  await expect(row).toContainText('Claude Code is not installed')
  await expect(row.locator('code')).toHaveText(INSTALL)
  await expect(check(page, 'claude_logged_in')).toHaveCount(0)

  await row.getByRole('button', { name: 'Copy' }).click()
  await expect(row.getByRole('button', { name: 'Copied' })).toBeVisible()
  expect(await copied(page)).toEqual([INSTALL])

  await page.getByRole('button', { name: 'Open the board' }).click()
  await expect(page.getByLabel('Empty board')).toBeVisible()
})

test('claude logged out is named with its own fix and Copy, and the board still opens', async ({ openFake }) => {
  const page = await openFake('first-run', recordClipboard)
  await look(page, '/work/loggedout')
  const row = check(page, 'claude_logged_in')
  await expect(row).toHaveAttribute('data-mark', 'no')
  await expect(row).toContainText('Claude Code is not logged in')
  await row.getByRole('button', { name: 'Copy' }).click()
  expect(await copied(page)).toEqual(['claude'])
  await expect(check(page, 'claude_installed').getByRole('button', { name: 'Copy' })).toHaveCount(0)
  await expect(page.getByRole('button', { name: 'Open the board' })).toBeEnabled()
})

test('a login that could not be checked is neither passed nor failed', async ({ openFake }) => {
  const page = await openFake('first-run')
  await look(page, '/work/unknown')
  const row = check(page, 'claude_logged_in')
  await expect(row).toHaveAttribute('data-mark', 'unknown')
  await expect(row).toContainText('Could not check that Claude Code is logged in')
  await expect(row.getByRole('button')).toHaveCount(0)
  await expect(page.getByRole('button', { name: 'Open the board' })).toBeEnabled()
})

test('Check again asks the daemon for the checks once more, and opens nothing', async ({ openFake }) => {
  const page = await openFake('first-run')
  await look(page, '/work/noclaude')
  await page.getByRole('button', { name: 'Check again' }).click()
  await expect.poll(async () => (await callsTo(page, 'board.checks')).length).toBe(2)
  expect(await callsTo(page, 'board.checks')).toEqual([{ root: '/work/noclaude' }, { root: '/work/noclaude' }])
  expect(await calls(page)).not.toContain('board.open')
})

test('a folder that is not a git repository cannot open, and Make it one asks first, then opens it', async ({
  openFake,
}) => {
  const page = await openFake('first-run')
  await look(page, '/work/plain')
  await expect(check(page, 'git_repo')).toHaveAttribute('data-mark', 'no')
  await expect(page.getByRole('button', { name: 'Open the board' })).toBeDisabled()
  await expect(firstRun(page)).toContainText('A folder that is not a git repo cannot open.')

  await check(page, 'git_repo').getByRole('button', { name: 'Make it one…' }).click()
  const window = page.getByRole('dialog', { name: 'Make this folder a git repository?' })
  await expect(window).toContainText('git init')
  await expect(window).toContainText('/work/plain')
  await expect(window).toContainText('changes nothing else')
  expect(await calls(page)).not.toContain('board.open')

  await window.getByRole('button', { name: 'Run git init' }).click()
  await expect(window).toBeHidden()
  await expect(page.getByLabel('Empty board')).toBeVisible()
  expect(await callsTo(page, 'board.open')).toEqual([{ root: '/work/plain', init_git: true }])
})

test('Cancel in the git window leaves the folder alone', async ({ openFake }) => {
  const page = await openFake('first-run')
  await look(page, '/work/plain')
  await page.getByRole('button', { name: 'Make it one…' }).click()
  const window = page.getByRole('dialog', { name: 'Make this folder a git repository?' })
  await window.getByRole('button', { name: 'Cancel' }).click()
  await expect(page.getByRole('dialog')).toHaveCount(0)
  expect(await calls(page)).not.toContain('board.open')
})

// ---- the empty board ---------------------------------------------------------------------------------------------

test('the empty board offers Plan it first (off, 9d), a scratch task, the sources, and Add a pack', async ({
  openFake,
}) => {
  const page = await openFake('empty-board')
  const empty = page.getByLabel('Empty board')
  await expect(empty).toContainText('Nothing on this board yet')
  const plan = empty.getByLabel('Plan it first')
  await expect(plan.getByRole('button', { name: 'Start a planning session' })).toBeDisabled()
  await expect(plan).toContainText('9d')
  await expect(plan).toContainText('Arrives in Stage 9d.')
  await expect(empty.getByLabel('Sources Collide found')).toContainText('scratch')
  await expect(empty.getByLabel('Sources Collide found')).toContainText('plan')
  await expect(empty.getByLabel('Add a pack')).toContainText('GitHub arrives in Stage 5.')
})

test('Start with a task makes a scratch task, and the Field shows it', async ({ openFake }) => {
  const page = await openFake('empty-board')
  const add = page.getByRole('button', { name: 'Add as a scratch task' })
  await expect(add).toBeDisabled()
  await page.getByLabel(/What needs doing/).fill('the sign-in page forgets where you were going')
  await add.click()
  await expect(page.getByLabel('Empty board')).toHaveCount(0)
  await expect(page.getByRole('button', { name: /the sign-in page forgets where you were going/ })).toBeVisible()
  expect(await callsTo(page, 'task.apply')).toEqual([
    {
      board: 'b-alpha',
      source: 'scratch',
      apply: { op: 'create', args: { prompt: 'the sign-in page forgets where you were going' } },
    },
  ])
})

test('Add a pack opens the window of the Settings page', async ({ openFake }) => {
  const page = await openFake('empty-board')
  await page.getByRole('button', { name: 'Add a pack…' }).click()
  await expect(page.getByRole('dialog', { name: 'Add a pack' })).toBeVisible()
})

// ---- Share with team ---------------------------------------------------------------------------------------------

const banner = (page: Page) => page.getByLabel('Share with team', { exact: true })

test('a board with no Collide folder says its settings are yours only, and Share lists the files first', async ({
  openFake,
}) => {
  const page = await openFake('empty-board')
  await expect(banner(page)).toContainText("This board's settings are yours only.")
  await banner(page).getByRole('button', { name: 'Share with team' }).click()
  const window = page.getByRole('dialog', { name: 'Share this board with your team?' })
  const files = window.getByRole('list', { name: 'Files to write' }).getByRole('listitem')
  await expect(files).toHaveText(['.collide/board.yaml', '.collide/views/mine.yaml'])
  await expect(window).toContainText('Nothing is committed')
  expect(await callsTo(page, 'board.init')).toEqual([{ board: 'b-alpha', dry_run: true }])

  await window.getByRole('button', { name: 'Not now' }).click()
  await expect(window).toBeHidden()
  expect(await callsTo(page, 'board.init')).toHaveLength(1)
  await expect(banner(page)).toContainText('yours only')
})

test('Write .collide/ writes, and the banner then asks for a commit until the line holds the files', async ({
  openFake,
}) => {
  const page = await openFake('empty-board')
  await banner(page).getByRole('button', { name: 'Share with team' }).click()
  const dialog = page.getByRole('dialog', { name: 'Share this board with your team?' })
  await dialog.getByRole('button', { name: 'Write .collide/' }).click()
  await expect(dialog).toBeHidden()
  expect(await callsTo(page, 'board.init')).toEqual([
    { board: 'b-alpha', dry_run: true },
    { board: 'b-alpha', dry_run: false },
  ])
  await expect(banner(page)).toContainText('is written in the repo folder')
  await expect(banner(page).getByRole('button')).toHaveCount(0)

  // The person commits. The page reads the board's views again when the view picker opens, and finds the files.
  await page.evaluate(() => window.collideFake.commit())
  await page.getByRole('button', { name: 'All tasks' }).click()
  await expect(banner(page)).toHaveCount(0)
})

test('a refusal from the daemon stays in the window in its words, and nothing is written', async ({ openFake }) => {
  const page = await openFake('share-refused')
  await banner(page).getByRole('button', { name: 'Share with team' }).click()
  const window = page.getByRole('dialog', { name: 'Share this board with your team?' })
  await expect(window.getByRole('alert')).toContainText('already exists')
  await expect(window.getByRole('button', { name: 'Write .collide/' })).toBeDisabled()
  expect(await callsTo(page, 'board.init')).toEqual([{ board: 'b-alpha', dry_run: true }])
})

test('a board whose line holds a Collide folder shows no banner', async ({ openFake }) => {
  const page = await openFake('shared-board')
  await expect(page.getByRole('button', { name: /A task that is here/ })).toBeVisible()
  await expect(banner(page)).toHaveCount(0)
})

// ---- the trust prompt --------------------------------------------------------------------------------------------

const trustWindow = (page: Page) => page.getByRole('dialog', { name: /Trust the .* this repo brings\?/ })
const notTrusted = (page: Page) => page.getByLabel('Not trusted', { exact: true })

test('a clone whose Collide folder brings packs, views and settings asks first, by what it brings', async ({
  openFake,
}) => {
  const page = await openFake('trust-board')
  const window = trustWindow(page)
  await expect(window).toBeVisible()
  await expect(window).toContainText('Trust the packs, views and settings this repo brings?')
  await expect(window).toContainText('alpha brings one pack of its own, two views and its settings.')
  await expect(window).toContainText('repo:.collide/packs/tracker')
  await expect(window).toContainText('providers/tasks.mjs')
  await expect(window).toContainText('mine')
  await expect(window).toContainText('team')
  await expect(window).toContainText('The built-in packs need no trust.')
  expect(await callsTo(page, 'board.trust')).toEqual([])
})

test('Open without it opens the board, names what is not applied, and Review and trust asks again', async ({
  openFake,
}) => {
  const page = await openFake('trust-board')
  await trustWindow(page).getByRole('button', { name: 'Open without it' }).click()
  await expect(trustWindow(page)).toBeHidden()
  await expect(page.getByRole('button', { name: /A task that is here/ })).toBeVisible()
  await expect(notTrusted(page)).toContainText('Not applied: one pack of its own, two views and its settings.')
  expect(await callsTo(page, 'board.trust')).toEqual([])

  await notTrusted(page).getByRole('button', { name: 'Review and trust…' }).click()
  await expect(trustWindow(page)).toBeVisible()
})

test('Escape on the window is Open without it: it never trusts', async ({ openFake }) => {
  const page = await openFake('trust-board')
  await expect(trustWindow(page)).toBeVisible()
  await page.keyboard.press('Escape')
  await expect(trustWindow(page)).toBeHidden()
  await expect(notTrusted(page)).toBeVisible()
  expect(await callsTo(page, 'board.trust')).toEqual([])
})

test('Trust and open trusts the hash it showed, and the notice goes', async ({ openFake }) => {
  const page = await openFake('trust-board')
  await trustWindow(page).getByRole('button', { name: 'Trust and open' }).click()
  await expect(trustWindow(page)).toBeHidden()
  await expect(notTrusted(page)).toHaveCount(0)
  expect(await callsTo(page, 'board.trust')).toEqual([{ board: 'b-alpha', line: 'main', hash: 'a'.repeat(40) }])
  // The board and its views are read again, and the prompt does not come back.
  await expect.poll(async () => (await callsTo(page, 'board.list')).length).toBeGreaterThan(1)
  await expect(trustWindow(page)).toHaveCount(0)
})

test('a board that is trusted, or brings nothing that waits, shows no prompt', async ({ openFake }) => {
  const page = await openFake('shared-board')
  await expect(page.getByRole('button', { name: /A task that is here/ })).toBeVisible()
  await expect(page.getByRole('dialog')).toHaveCount(0)
  await expect(notTrusted(page)).toHaveCount(0)
  expect(await callsTo(page, 'board.trust_review')).toEqual([])
})
