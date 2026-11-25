// The Field follows the files of a real daemon (ts-0182-ad): a temp repository with plan tasks and a second worktree,
// in a world of its own (`support/world.ts`). A task changed in a worktree shows as a divergence chip, and a task
// changed in the main checkout moves its card, which flashes once, with no toast and no reload of the page. The
// edits are made by the test's own file and git helpers.
import { type Page } from '@playwright/test'
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { expect, test } from './support/fixtures.ts'
import { firstPhase } from './support/recordings.ts'
import { type World, git } from './support/world.ts'

/**
 * The Field shows a change within the watch debounce, which is under the daemon's 10 s recheck (Stage 6a, Success
 * Criteria). The wait is shorter than the recheck on purpose: a pass proves the `watch` path and not the recheck. It
 * needs the bundle staged by `pnpm build:packs` to be one with `watch`.
 */
const WITHIN = 8_000

const EPIC = '---\nid: ep-001-aa\nkind: epic\nstatus: active\ntitle: "Made-up epic"\nowners: []\nlinks: []\n---\n'

const TASKS = [
  ['ts-0001-aa', 'ready', 'Write the parser'],
  ['ts-0002-bb', 'ready', 'Design the board'],
  ['ts-0003-cc', 'active', 'Build the Field'],
] as const

const taskFile = (id: string) => `.collide/planning/epics/ep-001-aa-alpha/tasks/${id}-work.md`

function planTask(id: string, status: string, title: string): string {
  return `---\nid: ${id}\nkind: task\nstatus: ${status}\ntitle: "${title}"\nowners: []\nlinks: []\n---\n\n## Goal\n`
}

function write(dir: string, path: string, text: string) {
  const file = join(dir, path)
  mkdirSync(dirname(file), { recursive: true })
  writeFileSync(file, text)
}

/** Sets a task's status in a checkout, as `plan set` does. */
function setStatus(dir: string, id: string, from: string, to: string) {
  const file = join(dir, taskFile(id))
  writeFileSync(file, readFileSync(file, 'utf8').replace(`status: ${from}`, `status: ${to}`))
}

/** The repository, committed, and a workspace of it. Returns the workspace's folder. */
function fill(world: World): string {
  write(world.repo, '.collide/planning/epics/ep-001-aa-alpha/epic.md', EPIC)
  for (const [id, status, title] of TASKS) write(world.repo, taskFile(id), planTask(id, status, title))
  git(world.repo, 'add', '.')
  git(world.repo, 'commit', '-q', '-m', 'the plan')
  // Only a worktree in the daemon's workspaces folder is a workspace of the board, and only one is named divergent.
  const worktree = join(world.root, 'data', 'workspaces', 'wt-a')
  mkdirSync(dirname(worktree), { recursive: true })
  git(world.repo, 'worktree', 'add', '-q', '-b', 'feature/a', worktree)
  return worktree
}

/** Opens the repository as a board and trusts its line, as the owner does in the app. */
async function openBoard(world: World): Promise<void> {
  const opened = await world.rpc<{ board: { id: string } }>('board.open', { root: world.repo, init_git: false })
  const board = opened.board.id
  const review = await world.rpc<{ trust: { hash: string } }>('board.trust_review', { board, line: 'main' })
  await world.rpc('board.trust', { board, line: 'main', hash: review.trust.hash, hosts: [], dev_folders: [] })
}

function column(page: Page, name: string) {
  return page.getByRole('region', { name, exact: true })
}

/** Remembers every card that was ever marked as flashing, since a flash is gone after two seconds, and the page. */
function watchFlashes(page: Page): Promise<void> {
  return page.evaluate(() => {
    const seen = new Set<string>()
    ;(window as unknown as { flashed: Set<string> }).flashed = seen
    // A reload of the page would take this away.
    ;(window as unknown as { loaded: boolean }).loaded = true
    const note = (card: Element) => {
      if (card.getAttribute('data-flash') === 'true') seen.add(card.getAttribute('data-card')!)
    }
    new MutationObserver((records) => {
      for (const record of records) {
        if (record.target instanceof Element) note(record.target)
        record.addedNodes.forEach((node) => {
          if (node instanceof Element) [node, ...node.querySelectorAll('[data-card]')].forEach(note)
        })
      }
    }).observe(document.body, { subtree: true, childList: true, attributes: true, attributeFilter: ['data-flash'] })
  })
}

test('a status change in a worktree shows as a divergence chip, and one in the main checkout moves its card', async ({
  startWorld,
  openPage,
}) => {
  const world = await startWorld({ first: firstPhase([]), path: [dirname(process.execPath)] })
  const worktree = fill(world)
  await openBoard(world)
  const page = await openPage(world)
  await expect(column(page, 'Ready')).toContainText('Write the parser')
  await expect(column(page, 'Ready')).toContainText('Design the board')
  await watchFlashes(page)

  // A worktree's copy that differs is a chip on the card in the main checkout's column.
  setStatus(worktree, 'ts-0001-aa', 'ready', 'active')
  const parser = column(page, 'Ready').getByRole('button', { name: /Write the parser/ })
  await expect(parser).toContainText('Active in wt-a', { timeout: WITHIN })

  // The main checkout's own copy moves the card to its new column.
  setStatus(world.repo, 'ts-0002-bb', 'ready', 'active')
  await expect(column(page, 'Active').getByRole('button', { name: /Design the board/ })).toBeVisible({
    timeout: WITHIN,
  })
  await expect(column(page, 'Ready')).not.toContainText('Design the board')
  await expect
    .poll(() => page.evaluate(() => [...(window as unknown as { flashed: Set<string> }).flashed]))
    .toContainEqual(expect.stringContaining('plan:ts-0002-bb'))
  await expect(page.getByRole('alert')).toHaveCount(0)
  const loaded = await page.evaluate(() => (window as unknown as { loaded?: boolean }).loaded)
  expect(loaded, 'the page was not reloaded').toBe(true)
})
