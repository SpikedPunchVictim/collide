// The Settings page in Chrome over a fake owner (ts-0193-gk): no Tauri and no daemon. The page plays a scripted world
// (`test/support/settings-world.ts`), and what the client called is read back from the fake (`window.collideFake`).
// The secret in these tests is made up and easy to search for; it must be in no text of the page and in no answer.
import { type Page } from '@playwright/test'
import { expect, test } from './support/fixtures.ts'

const SECRET = 'not-a-real-secret-0193'

async function openSettings(page: Page) {
  await page.getByRole('button', { name: 'Settings' }).click()
  await expect(page.getByRole('heading', { name: 'Settings' })).toBeVisible()
}

function calls(page: Page): Promise<string[]> {
  return page.evaluate(() => window.collideFake.calls())
}

/** Everything the page can show or keep: its text, its address, and what the browser stores for it. */
function whatThePageHolds(page: Page): Promise<string> {
  return page.evaluate(() => {
    const stored = (area: Storage) => JSON.stringify(Object.entries(area))
    return [document.documentElement.outerHTML, location.href, stored(localStorage), stored(sessionStorage)].join('\n')
  })
}

test('General is one table with each key, its value in use, and its layer as a badge', async ({ openFake }) => {
  const page = await openFake('settings')
  await openSettings(page)
  await expect(page.getByText('for alpha')).toBeVisible()
  const rows = page.getByRole('row')
  await expect(rows.filter({ hasText: 'ask.timeout_s' })).toContainText('120 s')
  await expect(rows.filter({ hasText: 'ask.timeout_s' })).toContainText('You')
  await expect(rows.filter({ hasText: 'disk.floor_gib' })).toContainText('5 GiB')
  await expect(rows.filter({ hasText: 'disk.floor_gib' })).toContainText('Default')
  await expect(rows.filter({ hasText: 'notify.webhook' })).toHaveCount(0)
  const calledWith = await page.evaluate(() => window.collideFake.calls())
  expect(calledWith).toContain('config.get')
})

test('a value the repo overrode shows struck out beside the repo\'s', async ({ openFake }) => {
  const page = await openFake('settings')
  await openSettings(page)
  const row = page.getByRole('row').filter({ hasText: 'ask.budget_usd' })
  await expect(row).toContainText('$0.25')
  await expect(row).toContainText('This repo')
  await expect(row.locator('s')).toHaveText('$0.50')
  await expect(row).toContainText('yours, overridden by this repo')
})

test('a problem shows on its row, and one that names no row shows above the table', async ({ openFake }) => {
  const page = await openFake('settings-problems')
  await openSettings(page)
  const row = page.getByRole('row').filter({ hasText: 'ask.timeout_s' })
  await expect(row).toContainText("`ask.timeout_s` in the user's config must be at least 10")
  await expect(page.getByRole('list', { name: 'Problems with the settings' })).toContainText('`config.ferry`')
})

test('Details lists the files the daemon reports', async ({ openFake }) => {
  const page = await openFake('settings')
  await openSettings(page)
  await page.getByText('Details for engineers: the files').click()
  await expect(page.getByText('/home/mock/.config/collide/config.yaml', { exact: true })).toBeVisible()
  await expect(page.getByText('/work/alpha/.collide/config.yaml', { exact: true })).toBeVisible()
  await expect(page.locator('dd').filter({ hasText: 'b-alpha/config.yaml' })).toContainText('(no file yet)')
})

test('a refusal of the settings shows in the daemon\'s words', async ({ openFake }) => {
  const page = await openFake('settings-refused')
  await openSettings(page)
  await expect(page.getByRole('alert')).toContainText('it does not read')
})

test('Agents says it arrives in Stage 6b', async ({ openFake }) => {
  const page = await openFake('settings')
  await openSettings(page)
  await page.getByRole('tab', { name: 'Agents' }).click()
  await expect(page.getByText('This page arrives with the in-app agent, in Stage 6b.')).toBeVisible()
})

// ---- Packs ----------------------------------------------------------------------------------------------------------

test('Packs lists each pack with where it is from and its status count', async ({ openFake }) => {
  const page = await openFake('settings')
  await openSettings(page)
  await page.getByRole('tab', { name: 'Packs' }).click()
  const packs = page.getByRole('list', { name: 'Packs' })
  await expect(packs.getByRole('listitem').filter({ hasText: 'collide/plan' })).toContainText('Built in')
  await expect(packs.getByRole('listitem').filter({ hasText: 'collide/plan' })).toContainText('5 statuses')
  const tracker = packs.getByRole('listitem').filter({ hasText: 'demo/tracker' })
  await expect(tracker).toContainText('Brought by this repo')
  await expect(tracker).toContainText('repo:.collide/packs/tracker')
  await expect(tracker).toContainText('6 statuses')
})

test('Add a pack shows what the pack runs, writes nothing until Add, then lists it', async ({ openFake }) => {
  const page = await openFake('settings')
  await openSettings(page)
  await page.getByRole('tab', { name: 'Packs' }).click()
  await page.getByRole('button', { name: 'Add a pack…' }).click()
  const window = page.getByRole('dialog', { name: 'Add a pack' })
  await window.getByLabel('Where is the pack?').fill('/work/tools/tidy')
  await window.getByRole('button', { name: 'Review the pack' }).click()
  await expect(window).toContainText('tidy is a pack. A pack runs code on this Mac')
  await expect(window).toContainText('hook on_start')
  await expect(window).toContainText('provider tasks')
  expect((await calls(page)).filter((method) => method === 'config.pack_add')).toHaveLength(1)
  await expect(page.getByRole('list', { name: 'Packs' }).getByText('tidy')).toHaveCount(0)
  await window.getByRole('button', { name: 'Add the pack' }).click()
  await expect(window).toBeHidden()
  const added = page.getByRole('list', { name: 'Packs' }).getByRole('listitem').filter({ hasText: 'tidy' })
  await expect(added).toContainText('Added by you')
  expect((await calls(page)).filter((method) => method === 'config.pack_add')).toHaveLength(2)
})

test('a git source from a new host asks a yes for that host before the review', async ({ openFake }) => {
  const page = await openFake('settings')
  await openSettings(page)
  await page.getByRole('tab', { name: 'Packs' }).click()
  await page.getByRole('button', { name: 'Add a pack…' }).click()
  const window = page.getByRole('dialog', { name: 'Add a pack' })
  await window.getByLabel('Where is the pack?').fill('https://example.test/pack.git')
  await window.getByRole('button', { name: 'Review the pack' }).click()
  await expect(window).toContainText('has not fetched from example.test before')
  await window.getByRole('button', { name: 'Fetch from example.test' }).click()
  await expect(window).toContainText('remote is a pack.')
  await window.getByRole('button', { name: 'Add the pack' }).click()
  await expect(window).toBeHidden()
  await expect(page.getByRole('list', { name: 'Packs' }).getByText('remote')).toBeVisible()
})

test('a source the daemon refuses stays in the window in its words, and Cancel adds nothing', async ({ openFake }) => {
  const page = await openFake('settings')
  await openSettings(page)
  await page.getByRole('tab', { name: 'Packs' }).click()
  await page.getByRole('button', { name: 'Add a pack…' }).click()
  const window = page.getByRole('dialog', { name: 'Add a pack' })
  await window.getByLabel('Where is the pack?').fill('/nowhere')
  await window.getByRole('button', { name: 'Review the pack' }).click()
  await expect(window.getByRole('alert')).toContainText('no pack is there')
  await window.getByRole('button', { name: 'Cancel' }).click()
  await expect(window).toBeHidden()
  await expect(page.getByRole('list', { name: 'Packs' }).getByRole('listitem')).toHaveCount(2)
})

// ---- Secrets --------------------------------------------------------------------------------------------------------

test('Secrets shows Set or Not set and never a value', async ({ openFake }) => {
  const page = await openFake('settings')
  await openSettings(page)
  await page.getByRole('tab', { name: 'Secrets' }).click()
  const webhook = page.getByRole('row').filter({ hasText: 'webhook' })
  await expect(webhook).toContainText('notify asks for it')
  await expect(webhook).toContainText('Not set')
  await expect(webhook.getByRole('button')).toHaveText('Set…')
  const key = page.getByRole('row').filter({ hasText: 'apiKey' })
  await expect(key).toContainText('Set')
  await expect(key.getByRole('button')).toHaveText('Replace…')
})

test('a secret is typed in a password box, sent once, and then it is nowhere', async ({ openFake }) => {
  const page = await openFake('settings')
  await openSettings(page)
  await page.getByRole('tab', { name: 'Secrets' }).click()
  await page.getByRole('button', { name: 'Set webhook' }).click()
  const window = page.getByRole('dialog', { name: 'Set webhook' })
  const box = window.getByLabel('Value')
  await expect(box).toHaveAttribute('type', 'password')
  const save = window.getByRole('button', { name: 'Save to keychain' })
  await expect(save).toBeDisabled()
  await box.fill(SECRET)
  await expect(save).toBeEnabled()
  expect(await box.inputValue()).toBe(SECRET)
  await save.click()
  await expect(window).toBeHidden()
  // The row is now Set, from the daemon's own answer, and the value is in nothing the page holds.
  const webhook = page.getByRole('row').filter({ hasText: 'webhook' })
  await expect(webhook).toContainText('Set')
  await expect(webhook).not.toContainText('Not set')
  await expect(webhook.getByRole('button')).toHaveText('Replace…')
  expect((await calls(page)).filter((method) => method === 'config.set')).toHaveLength(1)
  expect(await whatThePageHolds(page)).not.toContain(SECRET)
  // A window opened again starts empty.
  await page.getByRole('button', { name: 'Replace webhook' }).click()
  await expect(page.getByRole('dialog', { name: 'Set webhook' }).getByLabel('Value')).toHaveValue('')
})

test('a secret the daemon refuses shows its words, clears the box and is nowhere on the page', async ({ openFake }) => {
  const page = await openFake('settings-secret-refused')
  await openSettings(page)
  await page.getByRole('tab', { name: 'Secrets' }).click()
  await page.getByRole('button', { name: 'Set webhook' }).click()
  const window = page.getByRole('dialog', { name: 'Set webhook' })
  await window.getByLabel('Value').fill(SECRET)
  await window.getByRole('button', { name: 'Save to keychain' }).click()
  await expect(window.getByRole('alert')).toContainText('cannot read it, so nothing was set')
  await expect(window.getByLabel('Value')).toHaveValue('')
  await expect(window.getByRole('button', { name: 'Save to keychain' })).toBeDisabled()
  expect(await whatThePageHolds(page)).not.toContain(SECRET)
  await window.getByRole('button', { name: 'Cancel' }).click()
  await expect(page.getByRole('row').filter({ hasText: 'webhook' })).toContainText('Not set')
})
