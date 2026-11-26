// The window's frame in Chrome over a fake owner (ts-0179-ke): no Tauri and no daemon. The page plays a scripted world
// (`test/support/worlds.ts`), and what the client called is read back from the fake (`window.collideFake`), never from
// the page. The page is driven by the roles and names it was built with.
import { type Page } from '@playwright/test'
import { expect, test } from './support/fixtures.ts'

const TABS = [
  ['Field', 'the whole board'],
  ['Tracks', "what's running now"],
  ['Queue', 'what needs you'],
  ['Planning', 'what to build next'],
  ['Comms', 'sessions with an agent'],
] as const

/** The methods the client called so far, in order. */
function calls(page: Page): Promise<string[]> {
  return page.evaluate(() => window.collideFake.calls())
}

function tab(page: Page, name: string) {
  return page.getByRole('tab', { name: new RegExp(`^${name}`) })
}

/** The board menu's button, which names the board and its default line. */
function boardButton(page: Page, name: string | RegExp) {
  return page.getByRole('button', { name })
}

function boardMenu(page: Page) {
  return page.getByRole('dialog', { name: 'Open boards' })
}

async function openBoardMenu(page: Page) {
  await page.getByRole('button', { name: /alpha|beta|gamma|^Boards/ }).click()
  await expect(boardMenu(page)).toBeVisible()
}

// ---- the tabs -------------------------------------------------------------------------------------------------------

test('the window shows the five tabs with their subtitles, in order', async ({ openFake }) => {
  const page = await openFake('two-boards')
  await expect(page.getByRole('tab')).toHaveCount(5)
  for (const [index, [name, subtitle]] of TABS.entries()) {
    const shown = page.getByRole('tab').nth(index)
    await expect(shown).toContainText(name)
    await expect(shown.locator('small')).toHaveText(subtitle)
  }
  await expect(tab(page, 'Field')).toHaveAttribute('aria-selected', 'true')
})

test('keys 1 to 5 switch the tabs', async ({ openFake }) => {
  const page = await openFake('two-boards')
  await expect(tab(page, 'Field')).toHaveAttribute('aria-selected', 'true')
  for (const [index, [name]] of [...TABS.entries()].reverse()) {
    await page.keyboard.press(String(index + 1))
    await expect(tab(page, name)).toHaveAttribute('aria-selected', 'true')
    await expect(page.getByRole('tab', { selected: true })).toHaveCount(1)
  }
})

test('a key typed into a text box does not switch the tab', async ({ openFake }) => {
  const page = await openFake('two-boards')
  await openBoardMenu(page)
  const folder = page.getByLabel('Folder path')
  await folder.click()
  await page.keyboard.type('345')
  await expect(folder).toHaveValue('345')
  await expect(tab(page, 'Field')).toHaveAttribute('aria-selected', 'true')
  // The same keys, with nothing typed into, do switch it.
  await page.keyboard.press('Escape')
  await expect(boardMenu(page)).toHaveCount(0)
  await page.keyboard.press('4')
  await expect(tab(page, 'Planning')).toHaveAttribute('aria-selected', 'true')
})

test('Queue and Comms say what arrives and in which stage', async ({ openFake }) => {
  const page = await openFake('two-boards')
  await page.keyboard.press('3')
  await expect(page.getByText('Every decision waiting on you, longest wait first.')).toBeVisible()
  await expect(page.getByText(/It arrives in Stage 9a/)).toBeVisible()
  await page.keyboard.press('5')
  await expect(page.getByText(/Sessions with an agent that are not a task or a plan/)).toBeVisible()
  await expect(page.getByText(/It arrives in Stage 9b/)).toBeVisible()
})

test('Field shows the board, and Planning a placeholder', async ({ openFake }) => {
  const page = await openFake('two-boards')
  await expect(page.getByRole('region', { name: 'Field' })).toBeVisible()
  await page.keyboard.press('4')
  await expect(page.getByRole('heading', { name: 'Planning' })).toBeVisible()
})

test('Tracks shows the still preview and its banner, and calls nothing', async ({ openFake }) => {
  const page = await openFake('two-boards')
  // The first load is done once the board menu names a board and the Field has read it.
  await expect(boardButton(page, /alpha/)).toBeVisible()
  await expect(page.getByRole('region', { name: 'Field' })).toBeVisible()
  const before = await calls(page)
  await page.keyboard.press('2')
  await expect(page.getByText('A preview of Stage 9b.')).toBeVisible()
  await expect(page.getByText(/these runs are made up/)).toBeVisible()
  await expect(page.getByText('Run R-121')).toBeVisible()
  await expect(page.getByText('Run R-119')).toBeVisible()
  // A still preview: its rows and phases are not buttons, and showing it made no call.
  await expect(page.getByRole('button', { name: /Run R-|Build/ })).toHaveCount(0)
  expect(await calls(page)).toEqual(before)
})

// ---- the top bar ----------------------------------------------------------------------------------------------------

test('the board menu lists every board with its default line, and switching stops nothing', async ({ openFake }) => {
  const page = await openFake('two-boards')
  await expect(boardButton(page, /alpha/)).toContainText('main')
  // The Field has read the board, so what it reads later is what switching causes.
  await expect(page.getByRole('region', { name: 'Field' })).toBeVisible()

  await openBoardMenu(page)
  await expect(boardMenu(page).getByRole('button', { name: /alpha/ })).toContainText('line main')
  await expect(boardMenu(page).getByRole('button', { name: /alpha/ })).toHaveAttribute('aria-current', 'true')
  await expect(boardMenu(page).getByRole('button', { name: /beta/ })).toContainText('line develop')
  await expect(boardMenu(page)).toContainText('Open a folder…')

  const before = await calls(page)
  await boardMenu(page).getByRole('button', { name: /beta/ }).click()
  await expect(boardMenu(page)).toHaveCount(0)
  await expect(boardButton(page, /beta/)).toContainText('develop')
  await expect(page.getByRole('region', { name: 'Field' })).toBeVisible()
  // Switching is the window's own: the Field reads the other board, nothing else is sent, and in particular no stop
  // or cancel.
  const after = await calls(page)
  expect(after.slice(0, before.length)).toEqual(before)
  expect(after.slice(before.length)).toEqual(['task.views', 'task.list'])
  expect(after.filter((method) => /stop|cancel/.test(method))).toEqual([])
})

test('a folder typed into the board menu opens as a board, and a refused one says why', async ({ openFake }) => {
  const page = await openFake('two-boards')
  await openBoardMenu(page)
  await page.getByLabel('Folder path').fill('/work/nowhere')
  await page.getByRole('button', { name: 'Open folder' }).click()
  await expect(boardMenu(page).getByRole('alert')).toContainText('This folder is not a git repository.')

  await page.getByLabel('Folder path').fill('/work/gamma')
  await page.getByRole('button', { name: 'Open folder' }).click()
  await expect(boardMenu(page)).toHaveCount(0)
  await expect(boardButton(page, /gamma/)).toContainText('main')
  expect((await calls(page)).filter((method) => method === 'board.open')).toHaveLength(2)
})

test('with no board open, the board menu says Boards', async ({ openFake }) => {
  const page = await openFake('no-boards')
  await expect(page.getByRole('button', { name: /^Boards/ })).toBeVisible()
  await expect(tab(page, 'Field')).toHaveAttribute('aria-selected', 'true')
})

test('Settings opens its page, Back to the board returns, and a tab key leaves it', async ({ openFake }) => {
  const page = await openFake('two-boards')
  await page.getByRole('button', { name: 'Settings' }).click()
  await expect(page.getByRole('heading', { name: 'Settings' })).toBeVisible()
  await expect(page.getByRole('tab', { name: 'General' })).toHaveAttribute('aria-selected', 'true')
  await expect(tab(page, 'Field')).not.toHaveAttribute('aria-selected', 'true')
  await page.getByRole('button', { name: 'Back to the board' }).click()
  await expect(page.getByRole('heading', { name: 'Settings' })).toHaveCount(0)
  await expect(tab(page, 'Field')).toHaveAttribute('aria-selected', 'true')
  await page.getByRole('button', { name: 'Settings' }).click()
  await expect(page.getByRole('heading', { name: 'Settings' })).toBeVisible()
  await page.keyboard.press('1')
  await expect(page.getByRole('heading', { name: 'Settings' })).toHaveCount(0)
  await expect(tab(page, 'Field')).toHaveAttribute('aria-selected', 'true')
})

// ---- the loop page --------------------------------------------------------------------------------------------------

test('"Open the Loop page" shows the loop page, and its way back returns to the tab', async ({ openFake }) => {
  const page = await openFake('two-boards')
  for (const [key, name] of [
    ['3', 'Queue'],
    ['2', 'Tracks'],
  ] as const) {
    await page.keyboard.press(key)
    await page.getByRole('button', { name: 'Open the Loop page' }).click()
    await expect(page.getByRole('button', { name: /Claude version in use/ })).toBeVisible()
    await expect(page.getByRole('combobox', { name: 'Repository' })).toBeVisible()
    await expect(page.getByRole('tab')).toHaveCount(0)

    await page.getByRole('button', { name: `Back to ${name}` }).click()
    await expect(tab(page, name)).toHaveAttribute('aria-selected', 'true')
    await expect(page.getByRole('button', { name: 'Open the Loop page' })).toBeVisible()
  }
})

test('the board chosen in the menu is the board the loop page uses', async ({ openFake }) => {
  const page = await openFake('two-boards')
  await openBoardMenu(page)
  await boardMenu(page).getByRole('button', { name: /beta/ }).click()
  await page.keyboard.press('3')
  await page.getByRole('button', { name: 'Open the Loop page' }).click()
  await expect(page.getByRole('combobox', { name: 'Repository' })).toHaveValue('b-beta')
})
