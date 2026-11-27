// A board whose folder is gone, and locating it, in Chrome over a fake owner (ts-0191-nv; note 15): no Tauri and no
// daemon. The world `missing-board` has one board that is there and one whose folder is gone, and answers
// `board.locate` as the daemon does (`test/support/worlds.ts`). What the client called is read back from the fake,
// never from the page. A page opened with `&shell=1` acts as the shell's, which has the system's folder picker; the
// fake answers `shell.pick_folder` with what the test says (`window.collideFake.pickNext`).
import { type Page } from '@playwright/test'
import { expect, test } from './support/fixtures.ts'

function calls(page: Page): Promise<string[]> {
  return page.evaluate(() => window.collideFake.calls())
}

function boardMenu(page: Page) {
  return page.getByRole('dialog', { name: 'Open boards' })
}

async function openBoardMenu(page: Page) {
  await page.getByRole('button', { name: /alpha|gone|new|^Boards/ }).first().click()
  await expect(boardMenu(page)).toBeVisible()
}

/** Chooses the board whose folder is gone, and gives the missing page. */
async function showMissingBoard(page: Page) {
  await openBoardMenu(page)
  await boardMenu(page).getByRole('button', { name: /gone/ }).click()
  const missing = page.getByLabel('Board folder not found')
  await expect(missing).toBeVisible()
  return missing
}

function locateWindow(page: Page) {
  return page.getByRole('dialog', { name: 'Locate gone' })
}

test('a board whose folder is gone says so in the board menu, and no other row does', async ({ openFake }) => {
  const page = await openFake('missing-board')
  await openBoardMenu(page)
  const rows = boardMenu(page).getByRole('button')
  await expect(rows.filter({ hasText: 'folder not found' })).toHaveCount(1)
  await expect(rows.filter({ hasText: 'folder not found' })).toContainText('gone')
  await expect(rows.filter({ hasText: 'alpha' })).not.toContainText('folder not found')
})

test('choosing it shows where Collide looked, that nothing was deleted, and Locate', async ({ openFake }) => {
  const page = await openFake('missing-board')
  const missing = await showMissingBoard(page)
  await expect(missing).toContainText('gone: its folder is gone')
  await expect(missing.locator('code')).toHaveText('/work/gone')
  await expect(missing).toContainText('Nothing has been deleted.')
  await expect(missing.getByRole('button', { name: 'Locate the folder…' })).toBeEnabled()
  await expect(missing.getByRole('button', { name: 'Remove from Collide' })).toBeEnabled()
  // Nothing of the board is read, and the view picker has nothing to show.
  await expect(page.getByRole('button', { name: /All tasks/ })).toHaveCount(0)
})

test('a folder that is another repository is refused with the reason, and the board stays missing', async ({
  openFake,
}) => {
  const page = await openFake('missing-board')
  const missing = await showMissingBoard(page)
  await missing.getByRole('button', { name: 'Locate the folder…' }).click()
  const window = locateWindow(page)
  await expect(window).toBeVisible()
  await window.getByLabel('Where is the folder now?').fill('/work/other')
  await window.getByRole('button', { name: 'Use this folder' }).click()
  await expect(window.getByRole('alert')).toContainText('is a different repository')
  await expect(missing).toBeVisible()
  expect(await calls(page)).toContain('board.locate')
})

test('a folder that is the same repository brings the board back with its id, and the window closes', async ({
  openFake,
}) => {
  const page = await openFake('missing-board')
  const missing = await showMissingBoard(page)
  await missing.getByRole('button', { name: 'Locate the folder…' }).click()
  const window = locateWindow(page)
  await window.getByLabel('Where is the folder now?').fill('/work/new')
  await window.getByRole('button', { name: 'Use this folder' }).click()
  await expect(window).toBeHidden()
  await expect(missing).toBeHidden()
  // It is the same board, now at the new folder: the menu has two rows and none says "folder not found".
  await openBoardMenu(page)
  await expect(boardMenu(page).getByRole('button', { name: /new/ })).toContainText('/work/new')
  await expect(boardMenu(page).getByRole('button', { name: /new|alpha/ })).toHaveCount(2)
  await expect(boardMenu(page)).not.toContainText('folder not found')
})

test('a note the daemon adds is shown before the window closes', async ({ openFake }) => {
  const page = await openFake('missing-board')
  const missing = await showMissingBoard(page)
  await missing.getByRole('button', { name: 'Locate the folder…' }).click()
  const window = page.getByRole('dialog')
  await window.getByLabel('Where is the folder now?').fill('/work/unrecorded')
  await window.getByRole('button', { name: 'Use this folder' }).click()
  await expect(window).toContainText('could not check')
  await window.getByRole('button', { name: 'Close' }).click()
  await expect(window).toBeHidden()
})

test('the window can be cancelled and locates nothing', async ({ openFake }) => {
  const page = await openFake('missing-board')
  const missing = await showMissingBoard(page)
  await missing.getByRole('button', { name: 'Locate the folder…' }).click()
  await locateWindow(page).getByRole('button', { name: 'Cancel' }).click()
  await expect(locateWindow(page)).toBeHidden()
  await expect(missing).toBeVisible()
  expect(await calls(page)).not.toContain('board.locate')
})

test('a page that is not the shell has the typed path and no folder picker', async ({ openFake }) => {
  const page = await openFake('missing-board')
  await openBoardMenu(page)
  await expect(boardMenu(page).getByLabel('Folder path')).toBeVisible()
  await expect(boardMenu(page).getByRole('button', { name: 'Choose a folder…' })).toHaveCount(0)
  await boardMenu(page).getByRole('button', { name: /gone/ }).click()
  await page.getByRole('button', { name: 'Locate the folder…' }).click()
  await expect(locateWindow(page).getByRole('button', { name: 'Choose a folder…' })).toHaveCount(0)
  expect(await calls(page)).not.toContain('shell.pick_folder')
})

test('in the shell the menu opens the folder the person chose with the picker', async ({ openFake }) => {
  const page = await openFake('missing-board&shell=1')
  await page.evaluate(() => window.collideFake.pickNext('/work/gamma'))
  await openBoardMenu(page)
  await boardMenu(page).getByRole('button', { name: 'Choose a folder…' }).click()
  // The folder is looked at on the first-run page, and opens when the person says so.
  await expect.poll(() => calls(page)).toContain('board.checks')
  expect(await calls(page)).not.toContain('board.open')
  await page.getByRole('button', { name: 'Open the board' }).click()
  await expect.poll(() => calls(page)).toContain('board.open')
  await expect(page.getByRole('button', { name: /gamma/ })).toBeVisible()
})

test('in the shell a cancelled picker opens nothing', async ({ openFake }) => {
  const page = await openFake('missing-board&shell=1')
  await page.evaluate(() => window.collideFake.pickNext(null))
  await openBoardMenu(page)
  await boardMenu(page).getByRole('button', { name: 'Choose a folder…' }).click()
  await expect.poll(() => calls(page)).toContain('shell.pick_folder')
  expect(await calls(page)).not.toContain('board.checks')
  expect(await calls(page)).not.toContain('board.open')
})

test('in the shell the locate window fills the folder from the picker, and Use this folder locates it', async ({
  openFake,
}) => {
  const page = await openFake('missing-board&shell=1')
  const missing = await showMissingBoard(page)
  await missing.getByRole('button', { name: 'Locate the folder…' }).click()
  const dialog = locateWindow(page)
  await page.evaluate(() => window.collideFake.pickNext('/work/new'))
  await dialog.getByRole('button', { name: 'Choose a folder…' }).click()
  await expect(dialog.getByLabel('Where is the folder now?')).toHaveValue('/work/new')
  await dialog.getByRole('button', { name: 'Use this folder' }).click()
  await expect(dialog).toBeHidden()
  await expect(missing).toBeHidden()
})
