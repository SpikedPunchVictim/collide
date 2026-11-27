// A window started with defaults, in Chrome over a fake owner (ts-0215-gt): the page tells the shell before it asks the
// daemon anything, shows the banner, ignores the stored state and keeps it, lists what Reset moves, and after a Reset
// starts normally and names the folder. The page plays a scripted world (`test/support/worlds.ts`); `&defaults` on the
// fragment is what the shell's menu puts there. What is on the page is read by role and name, as a person reads it.
import { type Page } from '@playwright/test'
import { expect, test } from './support/fixtures.ts'

const WORLD = 'field-one-source'
const FOLDER = '/fake/.config/collide/set-aside/2026-11-26-101507'

function column(page: Page, name: string) {
  return page.getByRole('region', { name, exact: true })
}

function drawerOf(page: Page) {
  return page.getByRole('complementary', { name: 'Task drawer' })
}

function banner(page: Page) {
  return page.getByRole('status', { name: 'Started with defaults' })
}

/** The keys the page keeps in the webview's storage, with their values. */
function stored(page: Page): Promise<Record<string, string>> {
  return page.evaluate(() => Object.fromEntries(Object.entries(localStorage)))
}

/** Puts the window into defaults mode the way the shell's menu does: the flag on the fragment, then a reload. */
async function startWithDefaults(page: Page) {
  await page.evaluate(() => {
    const keys = new URLSearchParams(location.hash.slice(1))
    keys.set('defaults', '')
    location.hash = keys.toString()
  })
  await page.reload()
}

async function drawerWidth(page: Page): Promise<number> {
  return Math.round((await drawerOf(page).boundingBox())!.width)
}

test('a window started with defaults tells the shell first, then shows the banner', async ({ openFake }) => {
  const page = await openFake(`${WORLD}&defaults`)
  await expect(banner(page)).toContainText(
    'Started with defaults. Your settings, views and packs are set aside for this window.',
  )
  await expect(banner(page).getByRole('button', { name: 'Start normally' })).toBeVisible()
  await expect(banner(page).getByRole('button', { name: 'Reset…' })).toBeVisible()
  await expect(column(page, 'Ready')).toBeVisible()
  const calls = await page.evaluate(() => window.collideFake.calls())
  expect(calls[0]).toBe('shell.set_defaults')
  expect(await page.evaluate(() => window.collideFake.callsTo('shell.set_defaults'))).toEqual([{ defaults: true }])
})

test('a window started normally has no banner and does not tell the shell', async ({ openFake }) => {
  const page = await openFake(WORLD)
  await expect(column(page, 'Ready')).toBeVisible()
  await expect(banner(page)).toHaveCount(0)
  expect(await page.evaluate(() => window.collideFake.callsTo('shell.set_defaults'))).toEqual([])
})

test('a stored fold and drawer width are ignored in defaults mode and are still there after Start normally', async ({
  openFake,
}) => {
  const page = await openFake(WORLD)
  await page.getByRole('button', { name: /^Done\s*2$/ }).click()
  await expect(column(page, 'Done')).toBeVisible()
  await page.evaluate(() => localStorage.setItem('collide.drawer.width', '520'))
  const kept = await stored(page)
  expect(Object.keys(kept).length).toBeGreaterThan(1)

  await startWithDefaults(page)
  await expect(banner(page)).toBeVisible()
  await expect(column(page, 'Ready')).toBeVisible()
  await expect(column(page, 'Done')).toHaveCount(0)
  await page.locator('[data-card]').first().click()
  await expect(drawerOf(page)).toBeVisible()
  expect(await drawerWidth(page)).toBe(460)
  // A fold made in defaults mode is not kept either.
  await page.getByRole('button', { name: 'Fold Ready' }).click()
  await expect(column(page, 'Ready')).toHaveCount(0)
  expect(await stored(page)).toEqual(kept)

  await banner(page).getByRole('button', { name: 'Start normally' }).click()
  await expect(banner(page)).toHaveCount(0)
  await expect(column(page, 'Done')).toBeVisible()
  await expect(column(page, 'Ready')).toBeVisible()
  await page.locator('[data-card]').first().click()
  expect(await drawerWidth(page)).toBe(520)
  expect(await stored(page)).toEqual(kept)
  const told = await page.evaluate(() => window.collideFake.callsTo('shell.set_defaults'))
  expect(told).toEqual([])
})

test('Start normally reloads without the flag and keeps the other keys of the fragment', async ({ openFake }) => {
  const page = await openFake(`${WORLD}&defaults`)
  await expect(banner(page)).toBeVisible()
  const first = page.waitForEvent('load')
  await banner(page).getByRole('button', { name: 'Start normally' }).click()
  await first
  await expect(banner(page)).toHaveCount(0)
  expect(page.url()).not.toContain('defaults')
  expect(page.url()).toContain('world=')
})

test('Reset lists what it moves and what it leaves, and Cancel moves nothing', async ({ openFake }) => {
  const page = await openFake(`${WORLD}&defaults`)
  await banner(page).getByRole('button', { name: 'Reset…' }).click()
  const dialog = page.getByRole('dialog', { name: /^Reset/ })
  await expect(dialog).toBeVisible()
  const moves = dialog.getByRole('list', { name: 'Reset moves' })
  await expect(moves.getByRole('listitem')).toHaveCount(4)
  await expect(moves).toContainText('config.yaml')
  await expect(moves).toContainText('packs.yaml')
  await expect(moves).toContainText('packs.lock')
  await expect(moves).toContainText('The folder of this board')
  await expect(moves).toContainText('stored state')
  const leaves = dialog.getByRole('list', { name: 'Reset leaves' })
  await expect(leaves).toContainText('other boards')
  await expect(leaves).toContainText('The repo')
  await expect(leaves).toContainText('keychain')
  await dialog.getByRole('button', { name: 'Cancel' }).click()
  await expect(dialog).toHaveCount(0)
  expect(await page.evaluate(() => window.collideFake.callsTo('config.reset'))).toEqual([])
})

test('a confirmed Reset moves the files, clears the stored state, starts normally and names the folder', async ({
  openFake,
}) => {
  const page = await openFake(WORLD)
  await page.getByRole('button', { name: /^Done\s*2$/ }).click()
  await page.evaluate(() => localStorage.setItem('collide.drawer.width', '520'))
  await page.evaluate(() => localStorage.setItem('theme', 'dark'))
  await startWithDefaults(page)
  await expect(banner(page)).toBeVisible()

  await banner(page).getByRole('button', { name: 'Reset…' }).click()
  const reloaded = page.waitForEvent('load')
  await page.getByRole('dialog', { name: /^Reset/ }).getByRole('button', { name: 'Move them aside' }).click()
  await reloaded

  await expect(banner(page)).toHaveCount(0)
  const notice = page.getByRole('status', { name: 'Reset done' })
  await expect(notice).toContainText(FOLDER)
  await expect(column(page, 'Ready')).toBeVisible()
  // The stored state is gone (Done starts folded again), and what is not the UI's is left.
  await expect(column(page, 'Done')).toHaveCount(0)
  expect(await stored(page)).toEqual({ theme: 'dark' })
  await notice.getByRole('button', { name: 'Dismiss' }).click()
  await expect(notice).toHaveCount(0)
  expect(page.url()).not.toContain('reset=')
})

test('Reset is off until the boards have been read, and then names the board that shows', async ({ openFake }) => {
  const page = await openFake(`${WORLD}&defaults&hold-boards`)
  const reset = banner(page).getByRole('button', { name: 'Reset…' })
  await expect(banner(page)).toBeVisible()
  await expect(reset).toBeDisabled()
  await expect(banner(page).getByRole('button', { name: 'Start normally' })).toBeEnabled()

  await page.evaluate(() => window.collideFake.releaseBoards())
  await expect(reset).toBeEnabled()
  await reset.click()
  const moves = page.getByRole('dialog', { name: /^Reset/ }).getByRole('list', { name: 'Reset moves' })
  await expect(moves).toContainText('The folder of this board')
  await expect(moves).not.toContainText('No board is open')
})
