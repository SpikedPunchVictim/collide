// Removing a board from Collide in Chrome over a fake owner (ts-0222-gx): no Tauri and no daemon. The worlds
// `missing-board`, `two-boards` and `remove-refused` answer `board.remove` as the daemon does (`test/support/
// loop-world.ts`): the board leaves the list and the window is told. What the client called is read back from the fake,
// never from the page. The window asks first, in the page, and says the folder is not touched.
import { type Page } from '@playwright/test'
import { expect, test } from './support/fixtures.ts'

function calls(page: Page): Promise<string[]> {
  return page.evaluate(() => window.collideFake.calls())
}

function callsTo(page: Page, method: string): Promise<unknown[]> {
  return page.evaluate((name) => window.collideFake.callsTo(name), method)
}

function boardMenu(page: Page) {
  return page.getByRole('dialog', { name: 'Open boards' })
}

async function openBoardMenu(page: Page, current: RegExp) {
  await page.getByRole('button', { name: current }).first().click()
  await expect(boardMenu(page)).toBeVisible()
}

function removeWindow(page: Page, board: string) {
  return page.getByRole('dialog', { name: `Remove ${board} from Collide?` })
}

/** Chooses the board whose folder is gone, and gives the missing page. */
async function showMissingBoard(page: Page) {
  await openBoardMenu(page, /alpha/)
  await boardMenu(page).getByRole('button', { name: /gone/ }).click()
  const missing = page.getByLabel('Board folder not found')
  await expect(missing).toBeVisible()
  return missing
}

test('the missing page asks first, and says the folder is not touched and opening it brings the board back', async ({
  openFake,
}) => {
  const page = await openFake('missing-board')
  const missing = await showMissingBoard(page)
  await missing.getByRole('button', { name: 'Remove from Collide' }).click()

  const dialog = removeWindow(page, 'gone')
  await expect(dialog).toBeVisible()
  await expect(dialog).toContainText('/work/gone')
  await expect(dialog).toContainText('is not touched')
  await expect(dialog).toContainText('Opening the folder again brings the board back')
  expect(await calls(page)).not.toContain('board.remove')
})

test('Cancel removes nothing', async ({ openFake }) => {
  const page = await openFake('missing-board')
  const missing = await showMissingBoard(page)
  await missing.getByRole('button', { name: 'Remove from Collide' }).click()
  await removeWindow(page, 'gone').getByRole('button', { name: 'Cancel' }).click()

  await expect(removeWindow(page, 'gone')).toBeHidden()
  await expect(missing).toBeVisible()
  expect(await calls(page)).not.toContain('board.remove')
})

test('removing the board of the missing page moves the window to the board still listed', async ({ openFake }) => {
  const page = await openFake('missing-board')
  const missing = await showMissingBoard(page)
  await missing.getByRole('button', { name: 'Remove from Collide' }).click()
  await removeWindow(page, 'gone').getByRole('button', { name: 'Remove from Collide' }).click()

  await expect(removeWindow(page, 'gone')).toBeHidden()
  await expect(missing).toBeHidden()
  expect(await callsTo(page, 'board.remove')).toEqual([{ board: 'b-gone' }])
  await openBoardMenu(page, /alpha/)
  await expect(boardMenu(page).getByRole('button', { name: /gone/ })).toHaveCount(0)
})

test('the board menu removes the board in use, and the window moves to the other board', async ({ openFake }) => {
  const page = await openFake('two-boards')
  await openBoardMenu(page, /alpha/)
  await boardMenu(page).getByRole('button', { name: 'Remove from Collide…' }).click()

  const dialog = removeWindow(page, 'alpha')
  await expect(dialog).toBeVisible()
  await expect(dialog).toContainText('is not touched')
  await dialog.getByRole('button', { name: 'Remove from Collide' }).click()

  await expect(dialog).toBeHidden()
  expect(await callsTo(page, 'board.remove')).toEqual([{ board: 'b-alpha' }])
  await expect(page.getByRole('button', { name: /^beta/ })).toBeVisible()
  await openBoardMenu(page, /beta/)
  await expect(boardMenu(page).getByRole('button', { name: /alpha/ })).toHaveCount(0)
})

test('removing the last board shows the first-run page', async ({ openFake }) => {
  const page = await openFake('two-boards')
  for (const name of ['alpha', 'beta']) {
    await openBoardMenu(page, new RegExp(`^${name}`))
    await boardMenu(page).getByRole('button', { name: 'Remove from Collide…' }).click()
    await removeWindow(page, name).getByRole('button', { name: 'Remove from Collide' }).click()
    await expect(removeWindow(page, name)).toBeHidden()
  }

  await expect(page.getByLabel('Open a folder', { exact: true })).toBeVisible()
  await expect(page.getByLabel('Folder to look at')).toBeVisible()
})

test('a refusal stays in the window in the daemon’s words, and the board stays', async ({ openFake }) => {
  const page = await openFake('remove-refused')
  await openBoardMenu(page, /alpha/)
  await boardMenu(page).getByRole('button', { name: 'Remove from Collide…' }).click()
  const dialog = removeWindow(page, 'alpha')
  await dialog.getByRole('button', { name: 'Remove from Collide' }).click()

  await expect(dialog.getByRole('alert')).toContainText('run r-7 is in progress on the board b-alpha')
  await expect(dialog).toBeVisible()
  await dialog.getByRole('button', { name: 'Cancel' }).click()
  await openBoardMenu(page, /alpha/)
  await expect(boardMenu(page).getByRole('button', { name: /alpha/ })).toBeVisible()
})
