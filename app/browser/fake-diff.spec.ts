// The Changes part and the diff window in Chrome over a fake owner (ts-0189-fg; notes 17 and 29): the runs and commits
// of a task with their files, a file that opens its diff as the patch or as the whole file with the lines numbered and
// the changes marked, the window that scrolls to the first change, a cut patch that says so, and Escape that closes the
// window before the drawer. The page plays the `field-changes` world (`test/support/changes-world.ts`).
import { type Page } from '@playwright/test'
import { COMMIT, RUN_LIVE, changesOf, file, runEntry } from '../test/support/changes-world.ts'
import { expect, test } from './support/fixtures.ts'

const WORLD = 'field-changes'

const drawerOf = (page: Page) => page.getByRole('complementary', { name: 'Task drawer' })
const windowOf = (page: Page, path: string) => page.getByRole('dialog', { name: path })
const fileIn = (page: Page, path: string) => drawerOf(page).locator(`[data-file="${path}"]`)

async function open(openFake: (world: string) => Promise<Page>, title = 'Write the parser') {
  const page = await openFake(WORLD)
  await page.locator('[data-card]', { hasText: title }).click()
  await expect(drawerOf(page).getByRole('heading', { level: 2, name: title })).toBeVisible()
  return page
}

test('Changes lists each run and each commit with its files, the live run first and marked live', async ({
  openFake,
}) => {
  const page = await open(openFake)
  const changes = drawerOf(page).locator('[data-part="changes"]')
  await expect(changes.getByRole('heading', { level: 3 })).toHaveText(/^Changes/)
  await expect(changes.locator('[data-run]')).toHaveCount(2)
  await expect(changes.locator('[data-run="r-12"]')).toContainText('Run r-12 · Working')
  await expect(changes.locator('[data-run="r-12"]')).toContainText('Live, as the agent edits')
  await expect(changes.locator('[data-run="r-9"]')).toContainText('Run r-9 · Finished: done')
  await expect(changes.locator('[data-run="r-9"]')).not.toContainText('Live, as the agent edits')
  await expect(changes.locator('[data-run="r-12"]')).toContainText('node_modules: 1203 files, 4.3 MB, not listed')
  await expect(changes.locator(`[data-commit="${COMMIT}"]`)).toContainText('Fix the typo that ts-0001-aa left')
  await expect(changes).not.toContainText('No changes yet.')
})

test('a task with no change says so', async ({ openFake }) => {
  const page = await open(openFake, 'Build the Field')
  await expect(drawerOf(page).locator('[data-part="changes"]')).toContainText('No changes yet.')
})

test('a file opens its patch, with the lines numbered and the added ones marked', async ({ openFake }) => {
  const page = await open(openFake)
  await fileIn(page, 'src/parser.ts').click()
  const dialog = windowOf(page, 'src/parser.ts')
  await expect(dialog).toBeVisible()
  await expect(dialog).toContainText('Run r-12')
  await expect(dialog.getByRole('button', { name: 'Changes', exact: true })).toHaveAttribute('aria-pressed', 'true')
  await expect(dialog.locator('[data-kind="hunk"]').first()).toContainText('@@ -1,4 +1,4 @@')
  await expect(dialog.locator('[data-kind="add"][data-line="2"]')).toContainText('export const VERSION = 2')
  await expect(dialog.locator('[data-kind="del"]')).toContainText('export const VERSION = 1')
  await expect(dialog.locator('[data-kind="add"][data-line="41"]')).toContainText('// a new line')
})

test('Whole file shows the file after the changes and scrolls to the first change', async ({ openFake }) => {
  const page = await open(openFake)
  await fileIn(page, 'src/parser.ts').click()
  const dialog = windowOf(page, 'src/parser.ts')
  await dialog.getByRole('button', { name: 'Whole file' }).click()
  await expect(dialog.getByRole('button', { name: 'Whole file' })).toHaveAttribute('aria-pressed', 'true')
  const first = dialog.locator('[data-line="150"]')
  await expect(first).toHaveAttribute('data-kind', 'add')
  await expect(first).toBeInViewport()
  await expect(dialog.locator('[data-line="1"]')).not.toBeInViewport()
  await expect(dialog.locator('[data-line="2"]')).toHaveAttribute('data-kind', 'ctx')
  // Back to the patch.
  await dialog.getByRole('button', { name: 'Changes', exact: true }).click()
  await expect(dialog.locator('[data-kind="hunk"]').first()).toBeVisible()
})

test('a cut patch and a cut file say so', async ({ openFake }) => {
  const page = await open(openFake)
  await fileIn(page, 'docs/guide.md').click()
  const dialog = windowOf(page, 'docs/guide.md')
  await expect(dialog).toContainText('The patch is cut here')
  await dialog.getByRole('button', { name: 'Whole file' }).click()
  await expect(dialog).toContainText('The file is cut here')
})

test('a binary file says it has no lines to show', async ({ openFake }) => {
  const page = await open(openFake)
  await fileIn(page, 'assets/logo.png').click()
  const dialog = windowOf(page, 'assets/logo.png')
  await dialog.getByRole('button', { name: 'Whole file' }).click()
  await expect(dialog).toContainText('This file is not text')
})

test('a file of a commit opens the commit\'s diff', async ({ openFake }) => {
  const page = await open(openFake)
  await fileIn(page, 'README.md').click()
  const dialog = windowOf(page, 'README.md')
  await expect(dialog).toContainText('Commit abc1234')
  await expect(dialog.locator('[data-kind="add"]')).toContainText('The')
})

test('Escape closes the window and not the drawer; Close closes it too', async ({ openFake }) => {
  const page = await open(openFake)
  await fileIn(page, 'src/parser.ts').click()
  await expect(windowOf(page, 'src/parser.ts')).toBeVisible()
  await page.keyboard.press('Escape')
  await expect(windowOf(page, 'src/parser.ts')).toHaveCount(0)
  await expect(drawerOf(page)).toBeVisible()
  await fileIn(page, 'src/parser.ts').click()
  await windowOf(page, 'src/parser.ts').getByRole('button', { name: 'Close' }).click()
  await expect(windowOf(page, 'src/parser.ts')).toHaveCount(0)
  await expect(drawerOf(page)).toBeVisible()
})

test('a run in the person\'s checkout says so in the wire\'s words', async ({ openFake }) => {
  const page = await open(openFake, 'Review the wire')
  await expect(drawerOf(page).locator('[data-run="r-20"]')).toContainText(
    'Your checkout, since the agent phase began, including edits not committed yet and your own edits.',
  )
})

test('a run from before heads were kept shows the note as it comes', async ({ openFake }) => {
  const page = await open(openFake, 'Design the board')
  await expect(drawerOf(page).locator('[data-run="r-3"]')).toContainText('no changes recorded')
})

test('a live run follows the daemon: its files, and its end, as the events say them', async ({ openFake }) => {
  const page = await open(openFake)
  const live = drawerOf(page).locator('[data-run="r-12"]')
  await expect(live).toContainText('Live, as the agent edits')
  const more = runEntry(RUN_LIVE, {
    state: { state: 'ended', outcome: 'done' },
    files: [file('src/parser.ts', 31, 4), file('src/extra.ts', 7, 0, 'added')],
  })
  await page.evaluate((changes) => window.collideFake.setChanges('plan:ts-0001-aa', changes), changesOf([more]))
  await page.evaluate(() => window.collideFake.announceRun('r-12'))
  await expect(live).toContainText('Run r-12 · Finished: done')
  await expect(live).not.toContainText('Live, as the agent edits')
  await expect(fileIn(page, 'src/extra.ts')).toBeVisible()
})
