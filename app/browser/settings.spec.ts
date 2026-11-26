// The Settings page over a real daemon (ts-0193-gk): a temp repository with a pack that has settings and a secret, a
// settings file of the user's and one of the repo's, in a world of its own (`support/world.ts`). The daemon keeps the
// secret in memory, as every daemon under a test root does. The secret here is made up and easy to search for: it must
// be in no text of the page, in no answer of the owner's protocol that the page saw, and in no file of the world.
import { type APIResponse, type Page } from '@playwright/test'
import { mkdirSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { expect, test } from './support/fixtures.ts'
import { start, trust, write } from './support/octo-board.ts'
import { type World, git } from './support/world.ts'

const SECRET = 'not-a-real-secret-0193'

const PACK = [
  'apiVersion: collide/v0',
  'name: ferry',
  'version: 1.0.0',
  'config:',
  '  lanePool: { type: integer, minimum: 1, default: 2, overridable: true }',
  '  greeting: { type: string, default: hello }',
  '  notifyHook: { type: string, secret: true }',
  '',
].join('\n')

/** The user's settings file, below the HOME of the daemon, which is the root of the world. */
const userFile = (world: World) => join(world.root, '.config', 'collide', 'config.yaml')

function writeUser(world: World, text: string) {
  mkdirSync(dirname(userFile(world)), { recursive: true })
  writeFileSync(userFile(world), text)
}

/** The repository with its pack and its own settings, committed; opened as a board and trusted. Returns its id. */
async function openBoard(world: World): Promise<string> {
  write(world, '.collide/packs/ferry/pack.yaml', PACK)
  write(world, '.collide/board.yaml', 'packs:\n  - source: repo:.collide/packs/ferry\n')
  write(world, '.collide/config.yaml', 'config: { ferry: { lanePool: 1, greeting: team } }\n')
  git(world.repo, 'add', '.')
  git(world.repo, 'commit', '-q', '-m', 'the fixture')
  writeUser(world, 'config: { ferry: { lanePool: 4, greeting: mine } }\n')
  const opened = await world.rpc<{ board: { id: string } }>('board.open', { root: world.repo, init_git: false })
  await trust(world, opened.board.id)
  return opened.board.id
}

/**
 * Keeps the text of every answer of the owner's protocol that the page got. Each answer is read here, before the page
 * gets it (`route.fetch`, then `route.fulfill` with the text that was read), so a page that reloads while a body is on
 * the way cannot make a read fail: the read is not the page's. An answer that cannot be read is not passed on, and is
 * listed in `unread`, which a check asserts is empty. A call that the page dropped on a reload has no answer to read.
 */
class Answers {
  readonly texts: string[] = []
  /** The calls whose answer came and was not read. */
  readonly unread: string[] = []

  readonly watch = async (page: Page) => {
    await page.route(
      (url) => url.pathname === '/rpc',
      async (route) => {
        const call = route.request().url()
        let answer: APIResponse
        try {
          answer = await route.fetch()
        } catch (failure) {
          // A page that is gone took its call with it. Any other failure is a call with no answer: it is listed.
          if (!page.isClosed()) this.unread.push(`${call}: ${String(failure)}`)
          await route.abort().catch(() => undefined)
          return
        }
        let text: string
        try {
          text = await answer.text()
        } catch (failure) {
          this.unread.push(`${call}: ${String(failure)}`)
          await route.abort().catch(() => undefined)
          return
        }
        this.texts.push(text)
        // The page may have reloaded meanwhile; the text is kept either way.
        await route.fulfill({ response: answer, body: text }).catch(() => undefined)
      },
    )
  }

  /** Every answer so far, as one text. */
  all(): string {
    return this.texts.join('\n')
  }
}

async function openSettings(page: Page) {
  await page.getByRole('button', { name: 'Settings' }).click()
  await expect(page.getByRole('heading', { name: 'Settings' })).toBeVisible()
}

test('the page shows both files, the layer of each value, and the files Details lists', async ({
  startWorld,
  openPage,
}) => {
  const world = await start(startWorld)
  await openBoard(world)
  const page = await openPage(world)
  await openSettings(page)

  const lanes = page.getByRole('row').filter({ hasText: 'ferry.lanePool' })
  await expect(lanes).toContainText('4')
  await expect(lanes).toContainText('You')
  await expect(lanes.locator('s')).toHaveText('1')
  await expect(lanes).toContainText("this repo's, overridden by you")
  const greeting = page.getByRole('row').filter({ hasText: 'ferry.greeting' })
  await expect(greeting).toContainText('team')
  await expect(greeting).toContainText('This repo')
  await expect(greeting.locator('s')).toHaveText('mine')
  await expect(greeting).toContainText('yours, overridden by this repo')
  await expect(page.getByRole('row').filter({ hasText: 'disk.floor_gib' })).toContainText('Default')
  await expect(page.getByRole('row').filter({ hasText: 'ferry.notifyHook' })).toHaveCount(0)

  await page.getByText('Details for engineers: the files').click()
  await expect(page.locator('dd').filter({ hasText: userFile(world) })).toBeVisible()
  await expect(page.locator('dd').filter({ hasText: join(world.repo, '.collide', 'config.yaml') })).toBeVisible()
})

test('Packs lists the board\'s packs, and a secret is set, then reported Set, and is nowhere', async ({
  startWorld,
  openPage,
}) => {
  const world = await start(startWorld)
  const board = await openBoard(world)
  const answers = new Answers()
  const page = await openPage(world, answers.watch)
  await openSettings(page)

  await page.getByRole('tab', { name: 'Packs' }).click()
  const packs = page.getByRole('list', { name: 'Packs' })
  await expect(packs.getByRole('listitem').filter({ hasText: 'ferry' })).toContainText('Brought by this repo')
  await expect(packs.getByRole('listitem').filter({ hasText: 'collide/plan' })).toContainText('Built in')

  await page.getByRole('tab', { name: 'Secrets' }).click()
  const row = page.getByRole('row').filter({ hasText: 'notifyHook' })
  await expect(row).toContainText('Not set')
  await row.getByRole('button', { name: 'Set notifyHook' }).click()
  const window = page.getByRole('dialog', { name: 'Set notifyHook' })
  await window.getByLabel('Value').fill(SECRET)
  await window.getByRole('button', { name: 'Save to keychain' }).click()
  await expect(window).toBeHidden()
  await expect(row).toContainText('Set')
  await expect(row).not.toContainText('Not set')
  await expect(row.getByRole('button')).toHaveText('Replace…')

  // The daemon itself says Set, in an answer that holds no value.
  const read = await world.rpc<{ entries: { key: string; is_set?: boolean; value?: unknown }[] }>('config.get', {
    board,
    with_problems: true,
  })
  const entry = read.entries.find((each) => each.key === 'ferry.notifyHook')
  expect(entry?.is_set).toBe(true)
  expect(entry?.value).toBeUndefined()
  answers.texts.push(JSON.stringify(read))

  // A page that is loaded again says Set too, and shows no value.
  await page.reload()
  await openSettings(page)
  await page.getByRole('tab', { name: 'Secrets' }).click()
  await expect(page.getByRole('row').filter({ hasText: 'notifyHook' })).toContainText('Set')

  const seen = answers.all()
  expect(answers.unread).toEqual([])
  expect(seen).toContain('"is_set":true')
  expect(seen).not.toContain(SECRET)
  expect(await page.evaluate(() => document.documentElement.outerHTML + location.href)).not.toContain(SECRET)
  expect(world.logs()).not.toContain(SECRET)
  expect(world.filesHolding(SECRET)).toEqual([])
})

test('a secret the daemon refuses shows the daemon\'s words, clears the box, and is nowhere', async ({
  startWorld,
  openPage,
}) => {
  const world = await start(startWorld)
  await openBoard(world)
  const answers = new Answers()
  const page = await openPage(world, answers.watch)
  await openSettings(page)
  await page.getByRole('tab', { name: 'Secrets' }).click()
  // The page reads the settings once, when it opens. A file that is broken before that read is answered gives the
  // page the daemon's error and no row, so the file is broken only once the row is there.
  await expect(page.getByRole('row').filter({ hasText: 'notifyHook' })).toContainText('Not set')

  // The user's file stops reading after the page has loaded, so the daemon refuses the call.
  writeUser(world, 'config: [unclosed\n')
  await page.getByRole('button', { name: 'Set notifyHook' }).click()
  const window = page.getByRole('dialog', { name: 'Set notifyHook' })
  await window.getByLabel('Value').fill(SECRET)
  await window.getByRole('button', { name: 'Save to keychain' }).click()
  await expect(window.getByRole('alert')).toContainText('config.yaml')
  await expect(window.getByLabel('Value')).toHaveValue('')
  await expect(window.getByRole('alert')).not.toContainText(SECRET)
  await window.getByRole('button', { name: 'Cancel' }).click()
  await expect(page.getByRole('row').filter({ hasText: 'notifyHook' })).toContainText('Not set')

  const seen = answers.all()
  expect(answers.unread).toEqual([])
  expect(seen).not.toContain(SECRET)
  expect(await page.evaluate(() => document.documentElement.outerHTML + location.href)).not.toContain(SECRET)
  expect(world.logs()).not.toContain(SECRET)
  expect(world.filesHolding(SECRET)).toEqual([])
})
