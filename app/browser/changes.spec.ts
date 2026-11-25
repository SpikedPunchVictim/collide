// The Changes part over a real daemon (ts-0189-fg; note 17): a task with a run of its own, held where the agent has
// written a file, and then let go; and a commit made by hand that names a plan task. The run is a prompt run, which is
// how a run on a task starts (`run.start` with the board and the task, as the loop page does; the item says a file
// run, but a run on a task is a prompt run, ts-0188-kb), played by fake Claude (`support/world.ts`), never a real one.
// Only a scratch task runs so far, and a scratch task lists no commits, so the commit is on a plan task.
import { type Page } from '@playwright/test'
import { mkdirSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { expect, test } from './support/fixtures.ts'
import { heldPhase } from './support/recordings.ts'
import { type Plan, type World, git } from './support/world.ts'

const EPIC = '---\nid: ep-001-aa\nkind: epic\nstatus: active\ntitle: "Made-up epic"\nowners: []\nlinks: []\n---\n'

function write(world: World, path: string, text: string) {
  const file = join(world.repo, path)
  mkdirSync(dirname(file), { recursive: true })
  writeFileSync(file, text)
}

/** The repository: an epic and one plan task, committed. */
function fill(world: World) {
  write(world, '.collide/planning/epics/ep-001-aa-alpha/epic.md', EPIC)
  const head = '---\nid: ts-0001-aa\nkind: task\nstatus: ready\ntitle: "Write the parser"\nowners: []\nlinks: []\n---'
  write(world, '.collide/planning/epics/ep-001-aa-alpha/tasks/ts-0001-aa-work.md', `${head}\n\nParse the files.\n`)
  git(world.repo, 'add', '.')
  git(world.repo, 'commit', '-q', '-m', 'the plan')
}

async function openBoard(world: World): Promise<string> {
  const opened = await world.rpc<{ board: { id: string } }>('board.open', { root: world.repo, init_git: false })
  const board = opened.board.id
  const review = await world.rpc<{ trust: { hash: string } }>('board.trust_review', { board, line: 'main' })
  await world.rpc('board.trust', { board, line: 'main', hash: review.trust.hash, hosts: [], dev_folders: [] })
  return board
}

/** The world's plan: fake Claude plays `first`, and `node` is on the daemon's path for the plan pack. */
const planOf = (first: string): Plan => ({ first, path: [dirname(process.execPath)] })

const drawerOf = (page: Page) => page.getByRole('complementary', { name: 'Task drawer' })

async function openDrawer(page: Page, title: string) {
  await page.locator('[data-card]', { hasText: title }).click()
  await expect(drawerOf(page).getByRole('heading', { level: 2, name: title })).toBeVisible()
  return drawerOf(page).locator('[data-part="changes"]')
}

test('a run of a task shows its file while the agent is held, and after', async ({ startWorld, openPage }) => {
  const written = [{ write_file: { path: 'a.txt', text: 'from the agent\n' } }]
  const world = await startWorld(planOf(heldPhase(written, [{ git_commit: 'Add a.txt' }])))
  fill(world)
  const board = await openBoard(world)
  // Only a scratch task runs so far, so the run is on one (ts-0188-kb).
  const made = await world.rpc('task.apply', {
    board,
    source: 'scratch',
    apply: { op: 'create', args: { prompt: 'Try the scratch thing\n\nA note to myself.' } },
  })
  expect(made.result.outcome).toBe('ok')
  const started = await world.rpc('run.start', { board, tasks: [made.result.task.key], workflow: 'collide/quick' })
  expect(started.problems ?? [], 'the start is not refused').toEqual([])
  expect(started.outcome).toBe('started')
  const run: string = started.run

  const page = await openPage(world)
  const changes = await openDrawer(page, 'Try the scratch thing')
  // Held at the permission request, with a.txt written in the run's worktree and not committed.
  const entry = changes.locator(`[data-run="${run}"]`)
  await expect(entry).toContainText(`Run ${run} · Working`)
  await expect(entry).toContainText('Live, as the agent edits')
  await expect(entry).toContainText('including edits not committed yet')
  await expect(entry.locator('[data-file="a.txt"]')).toBeVisible()
  await expect(entry.locator('[data-file="a.txt"]')).toContainText('+1')

  // Its diff opens, with the file's one line marked.
  await entry.locator('[data-file="a.txt"]').click()
  const diff = page.getByRole('dialog', { name: 'a.txt' })
  await expect(diff.locator('[data-kind="add"][data-line="1"]')).toContainText('from the agent')
  await page.keyboard.press('Escape')
  await expect(diff).toHaveCount(0)
  await expect(drawerOf(page)).toBeVisible()

  // The person lets it go on: the run is no longer live, and the file it committed is still listed.
  const { items } = await world.rpc('queue.list', {})
  const waiting = items.find((item: { kind: string }) => item.kind === 'permission')
  await world.rpc('queue.answer', { item: waiting.id, answer: { type: 'permission', reply: { type: 'allow' } } })
  await expect(entry).not.toContainText('Live, as the agent edits')
  await expect(entry.locator('[data-file="a.txt"]')).toBeVisible()
  await expect(page.getByRole('alert')).toHaveCount(0)
})

test('a commit made by hand that names a plan task is listed with its files', async ({ startWorld, openPage }) => {
  const world = await startWorld(planOf(heldPhase([], [])))
  fill(world)
  await openBoard(world)
  write(world, 'notes.txt', 'tidied\n')
  git(world.repo, 'add', 'notes.txt')
  git(world.repo, 'commit', '-q', '-m', 'Tidy up after ts-0001-aa')
  const page = await openPage(world)
  const changes = await openDrawer(page, 'Write the parser')
  const commit = changes.locator('[data-commit]')
  await expect(commit).toContainText('Tidy up after ts-0001-aa')
  await expect(commit.locator('[data-file="notes.txt"]')).toBeVisible()
  await expect(changes.locator('[data-run]')).toHaveCount(0)
  await expect(changes).not.toContainText('No changes yet.')
  await commit.locator('[data-file="notes.txt"]').click()
  await expect(page.getByRole('dialog', { name: 'notes.txt' }).locator('[data-kind="add"]')).toContainText('tidied')
})

test('a task with no run and no commit says there are no changes yet', async ({ startWorld, openPage }) => {
  const world = await startWorld(planOf(heldPhase([], [])))
  fill(world)
  await openBoard(world)
  const page = await openPage(world)
  const changes = await openDrawer(page, 'Write the parser')
  await expect(changes).toContainText('No changes yet.')
  await expect(changes.locator('[data-run]')).toHaveCount(0)
})
