// The thin loop in a browser (Stage 3c; ts-0082-fp): the page of ts-0081-wm in Chrome, over the generic `rpc` call,
// to a daemon of a temp root whose `claude` is fake Claude. Each test has a world of its own (`support/world.ts`).
//
// The page is driven by the roles and names it was built with. What happened to the repository is read with git, and
// what the daemon holds with the owner's protocol through the bridge, never from the page. The window opens on the
// frame's Field (ts-0179-ke), so each test first goes where a person goes: Queue, then "Open the Loop page".
import { type Page } from '@playwright/test'
import { expect, test } from './support/fixtures.ts'
import { adds, firstPhase, permissionPhase, resumedPhase } from './support/recordings.ts'
import { type World, git } from './support/world.ts'
import { existsSync, readFileSync, readdirSync } from 'node:fs'
import { join } from 'node:path'

const PROMPT = 'Rename the widget'
const WORDS = 'Also add b.txt'
const REVIEW = /ready to review/i

// ---- what a person does -------------------------------------------------------------------------------------------

/** The loop page, reached through the frame: it is one button from Queue. */
async function openLoopPage(page: Page) {
  await page.getByRole('tab', { name: /^Queue/ }).click()
  await page.getByRole('button', { name: 'Open the Loop page' }).click()
}

async function openRepository(page: Page, world: World) {
  await page.getByLabel('Folder path').fill(world.repo)
  await page.getByRole('button', { name: 'Open folder' }).click()
  await expect(page.getByRole('combobox', { name: 'Repository' })).toBeVisible()
}

async function sendPrompt(page: Page, prompt = PROMPT) {
  await page.getByLabel('What should the agent do?').fill(prompt)
  await page.getByRole('button', { name: 'Start', exact: true }).click()
}

/** The page after a prompt, with the repository open. */
async function prompted(page: Page, world: World) {
  await openLoopPage(page)
  // The chip shows once the page's first load has finished. A folder opened before then can be lost to that load, which
  // read the boards before it was opened (see Decisions in ts-0082-fp), so a person's first click is waited for here.
  await expect(page.getByRole('button', { name: /Claude version in use/ })).toBeVisible()
  await openRepository(page, world)
  await sendPrompt(page)
}

function changedFiles(page: Page) {
  return page.getByRole('list', { name: 'Changed files' })
}

async function runOnPage(page: Page): Promise<string> {
  return (await page.locator('details dd').first().textContent()) ?? ''
}

// ---- what the repository and the daemon say -----------------------------------------------------------------------

function worktrees(world: World): string[] {
  return git(world.repo, 'worktree', 'list', '--porcelain')
    .split('\n')
    .filter((line) => line.startsWith('worktree '))
}

function branches(world: World): string[] {
  return git(world.repo, 'branch', '--list', 'collide/*')
    .split('\n')
    .filter((line) => line !== '')
}

function workspaceFolders(world: World): string[] {
  const dir = join(world.root, 'data', 'workspaces')
  return existsSync(dir) ? readdirSync(dir) : []
}

async function queueItems(world: World): Promise<{ id: string; kind: string; task?: string; run?: string }[]> {
  return (await world.rpc('queue.list', {})).items
}

/** The category of the task the run was made for. */
async function taskCategory(world: World, key: string): Promise<string> {
  const [board] = (await world.rpc('board.list')).boards
  return (await world.rpc('task.get', { board: board.id, id: key })).task.state.category
}

/** The words fake Claude was started with, one launch a line. */
function launches(world: World): string[][] {
  return readFileSync(world.claudeLog, 'utf8')
    .split('\n')
    .filter((line) => line.includes('--session-id') || line.includes('--resume'))
    .map((line) => line.split(' '))
}

function valueOf(words: string[], flag: string): string | undefined {
  return words[words.indexOf(flag) + 1]
}

// ---- the loop ---------------------------------------------------------------------------------------------------

test('a prompt runs in a fresh worktree and ends in a review that accept merges', async ({ startWorld, openPage }) => {
  const world = await startWorld({ first: firstPhase(adds('a.txt')) })
  const page = await openPage(world)
  const start = git(world.repo, 'rev-parse', 'HEAD')

  await prompted(page, world)
  await expect(page.getByText(REVIEW).first()).toBeVisible()
  await expect(changedFiles(page)).toContainText('a.txt')
  await expect(changedFiles(page)).toContainText('1 added, 0 removed')

  // The run has a worktree of its own, and the checkout is as it was.
  expect(worktrees(world)).toHaveLength(2)
  expect(workspaceFolders(world)).toHaveLength(1)
  expect(git(world.repo, 'rev-parse', 'HEAD')).toBe(start)
  expect(existsSync(join(world.repo, 'a.txt'))).toBe(false)
  const [item] = await queueItems(world)
  expect(item.kind).toBe('review')
  const [run] = (await world.rpc('run.list', {})).runs
  expect(run.workflow).toBe('collide/quick')

  await changedFiles(page).getByRole('button', { name: /a\.txt/ }).click()
  await expect(page.getByText('+from the agent')).toBeVisible()

  await page.getByRole('button', { name: 'Accept', exact: true }).click()
  await expect(page.getByRole('status')).toHaveText('Accepted. The work is merged into your checkout.')

  expect(git(world.repo, 'log', '-n', '1', '--format=%s')).toBe('Add a.txt')
  expect(existsSync(join(world.repo, 'a.txt'))).toBe(true)
  expect(worktrees(world)).toHaveLength(1)
  expect(branches(world)).toEqual([])
  expect(workspaceFolders(world)).toEqual([])
  expect(await taskCategory(world, item.task!)).toBe('done')
  await expect(page.getByText('Nothing is waiting on you.')).toBeVisible()
})

test('a re-prompt continues the same session and a second review follows', async ({ startWorld, openPage }) => {
  const world = await startWorld({
    first: firstPhase(adds('a.txt')),
    resumed: resumedPhase(WORDS, adds('b.txt')),
  })
  const page = await openPage(world)

  await prompted(page, world)
  await expect(changedFiles(page)).toContainText('a.txt')
  await page.getByLabel('What should change?').fill(WORDS)
  await page.getByRole('button', { name: 'Send back' }).click()
  // The page does not keep "Sent back" in view: it follows the new run, and a run change clears the notice, which fake
  // Claude's quick second phase makes certain. What shows is the review of the second run.

  // The second review holds the work of both phases, which the branch has in the worktree of the first run.
  await expect(changedFiles(page)).toContainText('b.txt')
  await expect(changedFiles(page)).toContainText('a.txt')
  const calls = launches(world)
  expect(calls).toHaveLength(2)
  const session = valueOf(calls[0]!, '--session-id')
  expect(session).toBeTruthy()
  expect(valueOf(calls[1]!, '--resume')).toBe(session)

  await page.getByRole('button', { name: 'Accept', exact: true }).click()
  await expect(page.getByRole('status')).toHaveText('Accepted. The work is merged into your checkout.')
  expect(git(world.repo, 'log', '--format=%s', 'main').split('\n').slice(0, 2)).toEqual(['Add b.txt', 'Add a.txt'])
  expect(workspaceFolders(world)).toEqual([])
})

test('discard removes the worktree and leaves the checkout as it was', async ({ startWorld, openPage }) => {
  const world = await startWorld({ first: firstPhase(adds('a.txt')) })
  const page = await openPage(world)
  const start = git(world.repo, 'rev-parse', 'HEAD')

  await prompted(page, world)
  await expect(changedFiles(page)).toContainText('a.txt')
  await page.getByRole('button', { name: 'Discard', exact: true }).click()
  // The first press asks, and nothing is deleted yet.
  await expect(page.getByRole('group', { name: 'Confirm discard' })).toContainText('Delete this work?')
  expect(workspaceFolders(world)).toHaveLength(1)
  await page.getByRole('button', { name: 'Yes, delete this work' }).click()
  await expect(page.getByRole('status')).toHaveText('Discarded. The work and its copy of the project are deleted.')

  expect(workspaceFolders(world)).toEqual([])
  expect(worktrees(world)).toHaveLength(1)
  expect(branches(world)).toEqual([])
  expect(git(world.repo, 'rev-parse', 'HEAD')).toBe(start)
  expect(git(world.repo, 'status', '--porcelain')).toBe('')
  expect(git(world.repo, 'log', '--format=%s')).toBe('start')
  expect(existsSync(join(world.repo, 'a.txt'))).toBe(false)
})

// ---- a permission request ---------------------------------------------------------------------------------------

test('a permission request shows what the agent would use, and allowing it lets the run go on', async ({
  startWorld,
  openPage,
}) => {
  const world = await startWorld({ first: permissionPhase() })
  const page = await openPage(world)

  await prompted(page, world)
  await expect(page.getByText('Wants to run a command')).toBeVisible()
  await expect(page.getByText('Tool: Bash')).toBeVisible()
  await expect(page.getByText('touch out.txt').first()).toBeVisible()
  await expect(page.getByText('Working', { exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: /always/i })).toHaveCount(0)

  await page.getByRole('button', { name: 'Allow once' }).click()
  await expect(page.getByText(REVIEW).first()).toBeVisible()
  await expect(page.getByText('Wants to run a command')).toHaveCount(0)
})

// ---- the window closed and opened again -------------------------------------------------------------------------

test('a page loaded again mid-run and at review shows the same run and the same item', async ({
  startWorld,
  openPage,
}) => {
  const world = await startWorld({ first: permissionPhase() })
  const page = await openPage(world)

  await prompted(page, world)
  await expect(page.getByText('Wants to run a command')).toBeVisible()
  const run = await runOnPage(page)
  expect(run).not.toBe('')
  const [waiting] = await queueItems(world)
  expect(waiting.kind).toBe('permission')

  // Mid-run: the agent waits for an answer.
  await page.reload()
  await openLoopPage(page)
  await expect(page.getByText('Wants to run a command')).toBeVisible()
  await expect(page.getByText('touch out.txt').first()).toBeVisible()
  expect(await runOnPage(page)).toBe(run)
  expect(await queueItems(world)).toEqual([waiting])

  // The reloaded page is live: it hears the run go on.
  await page.getByRole('button', { name: 'Allow once' }).click()
  await expect(page.getByText(REVIEW).first()).toBeVisible()
  const [review] = await queueItems(world)
  expect(review.kind).toBe('review')
  const shown = await page.getByText(REVIEW).first().textContent()

  // At review.
  await page.reload()
  await openLoopPage(page)
  await expect(page.getByText(REVIEW).first()).toBeVisible()
  expect(await page.getByText(REVIEW).first().textContent()).toBe(shown)
  expect(await runOnPage(page)).toBe(run)
  expect(await queueItems(world)).toEqual([review])
  await expect(page.getByRole('button', { name: 'Accept', exact: true })).toBeVisible()
})
