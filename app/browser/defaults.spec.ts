// Start with defaults over a real daemon (ts-0215-gt): a board whose person has broken their own files in the three
// ways the item names, a settings file that does not parse, a personal view that does not compile and a user pack whose
// folder is gone, in a world of its own (`support/world.ts`). The window opens by the entry the shell's menu uses, the
// `defaults` key on the fragment, and the page tells the shell, whose connection to the daemon is made with the flag.
// The command, a client of its own on the same daemon, still sees the files and their problems. Reset moves the files
// into the set-aside folder, and the window starts normally. Every path is below the root of the world, which is the
// HOME of its daemon: nothing here reads or writes the person's folders or keychain.
import { type Page } from '@playwright/test'
import { spawnSync } from 'node:child_process'
import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { expect, test } from './support/fixtures.ts'
import { WITHIN, fill, openBoard, start } from './support/octo-board.ts'
import { COLLIDE_EXE, type World, git } from './support/world.ts'

const BROKEN_SETTINGS = 'disk: [unclosed\n'
const BROKEN_VIEW = 'name: broken\nwhere: "status =="\n'

const userDir = (world: World) => join(world.root, '.config', 'collide')

function writeUser(world: World, path: string, text: string) {
  const file = join(userDir(world), path)
  mkdirSync(dirname(file), { recursive: true })
  writeFileSync(file, text)
}

/** The repository's fixture, opened as a board and trusted, with two scratch tasks, then the person's files broken. */
async function brokenWorld(startWorld: Parameters<typeof start>[0]) {
  const world = await start(startWorld)
  fill(world)
  const board = await openBoard(world)
  for (const prompt of ['first made-up task', 'second made-up task']) {
    await world.rpc('task.apply', { board, source: 'scratch', apply: { op: 'create', args: { prompt } } })
  }
  writeUser(world, 'config.yaml', BROKEN_SETTINGS)
  writeUser(world, `boards/${board}/views/broken.yaml`, BROKEN_VIEW)
  writeUser(world, 'packs.yaml', `packs:\n  - source: dev:${join(world.root, 'devs', 'gone')}\n`)
  writeUser(world, `boards/${board}/packs.yaml`, 'on: [gone]\n')
  return { world, board }
}

/** What the command prints for the settings of the board, run as a person runs it in the repository. */
function configGet(world: World) {
  return spawnSync(COLLIDE_EXE, ['config', 'get'], {
    cwd: world.repo,
    env: { PATH: '/usr/bin:/bin', HOME: world.root, COLLIDE_DAEMON_ROOT: world.root },
    encoding: 'utf8',
    timeout: WITHIN,
  })
}

/** Opens the page on the bridge in defaults mode: the flag on the fragment, as the shell's menu puts it. */
async function openWithDefaults(world: World, openPage: (world: World) => Promise<Page>) {
  const page = await openPage(world)
  await page.evaluate(() => {
    const keys = new URLSearchParams(location.hash.slice(1))
    keys.set('defaults', '')
    location.hash = keys.toString()
  })
  await page.reload()
  return page
}

const banner = (page: Page) => page.getByRole('status', { name: 'Started with defaults' })

const card = (page: Page, title: string) => page.locator('[data-card]', { hasText: title })

test('with broken files, defaults mode opens the board with the built-in views and the fixture’s tasks', async ({
  startWorld,
  openPage,
}) => {
  const { world } = await brokenWorld(startWorld)
  const page = await openWithDefaults(world, openPage)
  await expect(banner(page)).toContainText('Started with defaults.')
  // The built-in plan pack reads the plan's tasks, and the scratch source has the two made-up ones.
  await expect(card(page, 'Write the parser')).toBeVisible()
  await expect(card(page, 'first made-up task')).toBeVisible()
  await expect(card(page, 'second made-up task')).toBeVisible()
  // The repo's pack and its tasks are not loaded, and neither are its views or the person's.
  await expect(card(page, 'Waits its turn')).toHaveCount(0)
  await page.getByTitle('The view of this board').click()
  const views = page.getByRole('dialog', { name: 'Views' })
  await expect(views.getByRole('button', { name: /^All tasks/ })).toBeVisible()
  await expect(views.getByRole('button', { name: /^edge-mvp/ })).toHaveCount(0)
  await expect(views.getByRole('button', { name: /^octo-only/ })).toHaveCount(0)
  await expect(views.getByRole('button', { name: /^broken/ })).toHaveCount(0)
  await page.keyboard.press('Escape')

  // Settings shows every key at Default, and no problem.
  await page.getByRole('button', { name: 'Settings' }).click()
  await expect(page.getByRole('heading', { name: 'Settings' })).toBeVisible()
  const rows = page.getByRole('row').filter({ has: page.getByRole('cell') })
  await expect(rows.first()).toBeVisible()
  const layers = await rows.evaluateAll((all) => all.map((row) => row.textContent ?? ''))
  expect(layers.length).toBeGreaterThan(3)
  for (const row of layers) expect(row, row).toContain('Default')
  await expect(page.getByRole('list', { name: 'Problems with the settings' })).toHaveCount(0)

  // The command, a client of its own on the same daemon, still sees the person's settings file and its problem.
  const printed = configGet(world)
  expect(printed.status, printed.stdout).not.toBe(0)
  expect(printed.stderr).toContain('config.yaml')
})

test('every write is refused in defaults mode, with words that name it, and no file changes', async ({
  startWorld,
  openPage,
}) => {
  const { world, board } = await brokenWorld(startWorld)
  const page = await openWithDefaults(world, openPage)
  await expect(banner(page)).toBeVisible()
  const files = () =>
    ['config.yaml', 'packs.yaml', `boards/${board}/packs.yaml`, `boards/${board}/views/broken.yaml`].map((path) =>
      readFileSync(join(userDir(world), path), 'utf8'),
    )
  const before = files()
  const repoBefore = git(world.repo, 'status', '--porcelain')
  // The test's calls go through the bridge's one shell, which the page put into defaults mode.
  await expect(world.rpc('config.set', { layer: 'user', key: 'disk.floor_gib', value: 3 })).rejects.toThrow(/defaults/)
  await expect(
    world.rpc('task.save_view', { board, view: { name: 'mine' }, target: 'personal' }),
  ).rejects.toThrow(/defaults/)
  await expect(world.rpc('board.init', { board })).rejects.toThrow(/defaults/)
  expect(files()).toEqual(before)
  expect(git(world.repo, 'status', '--porcelain')).toBe(repoBefore)
})

test('Reset moves the files into the set-aside folder, and the window starts normally and names it', async ({
  startWorld,
  openPage,
}) => {
  const { world, board } = await brokenWorld(startWorld)
  const page = await openWithDefaults(world, openPage)
  await expect(banner(page)).toBeVisible()
  const kept = {
    models: readFileSync(join(userDir(world), 'models.yaml'), 'utf8'),
  }

  await banner(page).getByRole('button', { name: 'Reset…' }).click()
  const dialog = page.getByRole('dialog', { name: /^Reset/ })
  await expect(dialog.getByRole('list', { name: 'Reset moves' })).toContainText('config.yaml')
  const reloaded = page.waitForEvent('load')
  await dialog.getByRole('button', { name: 'Move them aside' }).click()
  await reloaded

  await expect(banner(page)).toHaveCount(0)
  const folders = readdirSync(join(userDir(world), 'set-aside'))
  expect(folders).toHaveLength(1)
  const aside = join(userDir(world), 'set-aside', folders[0]!)
  await expect(page.getByRole('status', { name: 'Reset done' })).toContainText(aside)
  // The files moved with their paths and bytes. What the list does not name stays where it was.
  expect(readFileSync(join(aside, 'config.yaml'), 'utf8')).toBe(BROKEN_SETTINGS)
  expect(readFileSync(join(aside, 'boards', board, 'views', 'broken.yaml'), 'utf8')).toBe(BROKEN_VIEW)
  expect(readFileSync(join(aside, 'boards', board, 'packs.yaml'), 'utf8')).toBe('on: [gone]\n')
  expect(existsSync(join(aside, 'packs.yaml'))).toBe(true)
  expect(existsSync(join(userDir(world), 'config.yaml'))).toBe(false)
  expect(existsSync(join(userDir(world), 'boards', board))).toBe(false)
  expect(readFileSync(join(userDir(world), 'models.yaml'), 'utf8')).toBe(kept.models)
  expect(git(world.repo, 'status', '--porcelain')).toBe('')

  // The window starts normally: the shell's connection is a normal one again, and the files that broke it are gone.
  await expect(card(page, 'Write the parser')).toBeVisible()
  expect(configGet(world).status).toBe(0)
})
