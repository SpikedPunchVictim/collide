// Stage 6a's smoke run (ts-0196-uw): S6's scenario 1 up to the board and scenario 2, ported from S6's pilots
// (`sp-0009-bb-interface-prototype`, `test/pilot.test.ts` and `test/ui.test.tsx`) to the built app, over a real daemon
// through the bridge. Each scenario ends where S6's detector says it does, read from the daemon and not from the page.
// The times are information: each scenario prints its time beside S6's advisory limit (R5) and a slow run never fails.
//
// What S6's controls are in the app:
//   S6 scenario 1   the first-run page (Check this folder, Open the board), the empty board, and Start with a task.
//                   The card goes on to a run and its review, which Stage 9a builds: the end state here is the board
//                   showing the scratch task. The daemon of a test root answers "could not check" to the login probe
//                   and never fails it, so "the checks pass" means no row is a failure, and git is the one required.
//   Stage E-07 chip the Epic filter: the funnel on the epic's card in the strip, or the Epic box in the filter bar.
//   Ready chip      the Where it stands chip Ready (the category `ready`).
//   perf chip       the Labels chip perf.
//   Save as view    Save as a view… in the filter bar, with the name; a view of the person's own. A view's name is
//                   its file name (letters, digits, `.`, `_`, `-`), so the card's "Slow spots" is refused, with the
//                   reason, and the run goes on with "Slow-spots".
//   S2.detect       a saved view named Slow spots (or Slow-spots) whose members, read with `task.list` and the view's
//                   name, are the five tasks of `S2_TARGET`.
import { type Page } from '@playwright/test'
import { cpSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { expect, test } from './support/fixtures.ts'
import { type Listing, trust, until, write } from './support/octo-board.ts'
import { firstPhase } from './support/recordings.ts'
import { SOURCE_ROOT, type World, git } from './support/world.ts'

// `node` is on the daemon's PATH: the built-in plan pack and the fixture pack both run a provider with it.
const plan = { first: firstPhase([]), path: [dirname(process.execPath)] }

/** S6's advisory limits in seconds (`record.md`; the owner, 2026-11-14). A run over one is reported and never fails. */
const LIMIT = { S1: 150, S2: 60 } as const

/** S1's card: "add a `--quiet` flag to the command line tool". The words of the task a person starts with. */
const S1_TASK = 'Add a --quiet flag to the CLI'

/** `S2_TARGET` of S6's `src/scenarios.ts`: the epic E-07 tasks that are ready and carry the label perf. */
const S2_TARGET = ['T-07-001', 'T-07-004', 'T-07-007', 'T-07-010', 'T-07-013'].map((id) => `s6:${id}`)
/** The card's name, which the host refuses as a view name (it has a space), and the name that stands for it. */
const CARD_NAME = 'Slow spots'
const SLOW_SPOTS = 'Slow-spots'

/** Prints the time from `startedAt` to now beside the limit. The time is never asserted. */
function report(scenario: keyof typeof LIMIT, startedAt: number) {
  const seconds = (Date.now() - startedAt) / 1000
  const over = seconds > LIMIT[scenario] ? ' (over it; advisory only)' : ''
  const took = `${seconds.toFixed(1)} s from the first action to the detected end`
  const line = `${scenario}: ${took}, limit ${LIMIT[scenario]} s${over}`
  console.log(line)
  test.info().annotations.push({ type: `time ${scenario}`, description: line })
}

const firstRun = (page: Page) => page.getByLabel('Open a folder', { exact: true })
const bar = (page: Page) => page.getByRole('region', { name: 'Filters', exact: true })
const strip = (page: Page) => page.getByRole('region', { name: 'Epics', exact: true })

/** The keys `task.list` gives for a board, over the view of this name, sorted. */
async function members(world: World, board: string, view: string): Promise<string[]> {
  const answer = await world.rpc<Listing>('task.list', { board, view: { type: 'named', name: view } })
  return answer.tasks.map((task) => task.key).sort()
}

/** The names of the views the daemon lists for the board. */
async function viewNames(world: World, board: string): Promise<string[]> {
  const answer = await world.rpc<{ views: { view: { name: string } }[] }>('task.views', { board })
  return answer.views.map((each) => each.view.name)
}

/**
 * S6's `S2.detect`: a view named Slow spots (any case, trimmed) whose members are exactly the five targets. A hyphen
 * may stand for the space, because a view's name cannot hold one.
 */
async function slowSpotsDetected(world: World, board: string): Promise<boolean> {
  const names = (await viewNames(world, board)).filter((name) => /^slow[ -]spots$/i.test(name.trim()))
  for (const name of new Set(names)) {
    const found = await members(world, board, name)
    if (found.length === S2_TARGET.length && found.every((key, at) => key === [...S2_TARGET].sort()[at])) return true
  }
  return false
}

/**
 * S2's board: the `s6-board` fixture pack copied into the repository's Collide folder and named in the board file,
 * committed, the board opened and its line trusted as the owner does in the app. Returns the board's id once the
 * pack's source has listed all its tasks.
 */
async function openSlowSpotsBoard(world: World): Promise<string> {
  cpSync(join(SOURCE_ROOT, 'fixtures/packs/s6-board'), join(world.repo, '.collide/packs/s6-board'), {
    recursive: true,
  })
  write(world, '.collide/board.yaml', 'packs:\n  - source: repo:.collide/packs/s6-board\n')
  git(world.repo, 'add', '.')
  git(world.repo, 'commit', '-q', '-m', 'the S6 board')
  const opened = await world.rpc<{ board: { id: string } }>('board.open', { root: world.repo, init_git: false })
  const board = opened.board.id
  await trust(world, board)
  let last: Listing | undefined
  await until(
    'the pack has listed its tasks',
    async () => {
      last = await world.rpc<Listing>('task.list', { board })
      const listed = last.sources.some((source) => source.name === 's6')
      return !last.refreshing && last.tasks.length === 17 && listed ? last : undefined
    },
    () =>
      JSON.stringify({
        tasks: last?.tasks.length,
        invalid: last?.invalid,
        sources: last?.sources.map((source) => source.name),
      }),
  )
  return board
}

test('S6 scenario 1, up to the board: a fresh repository opens with nothing set up, and Start with a task', async ({
  startWorld,
  openPage,
}) => {
  const world = await startWorld(plan)
  const page = await openPage(world)
  await expect(page.getByLabel('Folder to look at')).toBeVisible()

  // The first action: the folder to open.
  const startedAt = Date.now()
  await page.getByLabel('Folder to look at').fill(world.repo)
  await page.getByRole('button', { name: 'Check this folder' }).click()
  await expect(page.getByRole('heading', { name: 'Checks' })).toBeVisible()
  await expect(firstRun(page).getByText('Checking the folder…')).toHaveCount(0)
  // The checks pass: git is the one that is required, claude is the world's fake, and the login is not a failure.
  await expect(firstRun(page).locator('[data-check="git_repo"]')).toHaveAttribute('data-mark', 'ok')
  await expect(firstRun(page).locator('[data-check="claude_installed"]')).toHaveAttribute('data-mark', 'ok')
  await expect(firstRun(page).locator('[data-check][data-mark="no"]')).toHaveCount(0)

  // The board opens with no setup, and the empty board shows once the board settles.
  await page.getByRole('button', { name: 'Open the board' }).click()
  await expect(page.getByLabel('Empty board')).toContainText('Nothing on this board yet')

  // Start with a task, in the card's words.
  await page.getByLabel(/What needs doing/).fill(S1_TASK)
  await page.getByRole('button', { name: 'Add as a scratch task' }).click()

  // The end state, as S6's `S1.detect` reads it for a task the person made: the board shows the scratch task, and the
  // daemon lists it. Launching it and reviewing the run are Stage 9a's.
  await expect(page.getByLabel('Empty board')).toHaveCount(0)
  await expect(page.getByRole('button', { name: new RegExp(S1_TASK) })).toBeVisible()
  const board = (await world.rpc<{ boards: { id: string }[] }>('board.list')).boards[0]!.id
  const listed = await world.rpc<{ tasks: { key: string; title: string }[] }>('task.list', { board })
  expect(listed.tasks.map((task) => [task.key.split(':')[0], task.title])).toEqual([['scratch', S1_TASK]])
  report('S1', startedAt)
})

test('S6 scenario 2: Performance, Ready and perf, saved as Slow spots, is the five tasks and detects', async ({
  startWorld,
  openPage,
}) => {
  const world = await startWorld(plan)
  const board = await openSlowSpotsBoard(world)
  // The start state is S6's: the five targets are exactly the ready, perf tasks of E-07 among the decoys.
  const start = await world.rpc<Listing>('task.list', {
    board,
    filter: '"s6:E-07" in collections && category == "ready" && "perf" in labels',
  })
  expect(start.tasks.map((task) => task.key).sort()).toEqual([...S2_TARGET].sort())
  expect(await slowSpotsDetected(world, board)).toBe(false)

  const page = await openPage(world)
  await expect(strip(page).locator('[data-epic="s6:E-07"]')).toBeVisible()
  await expect(page.locator('[data-card]').first()).toBeVisible()

  // The first action: Show only the tasks of epic E-07 (its funnel), then the Ready and perf chips.
  const startedAt = Date.now()
  await strip(page).getByRole('button', { name: 'Show only the tasks of Performance' }).click()
  await bar(page).getByRole('button', { name: /^Ready, \d+ tasks$/ }).click()
  await bar(page).getByRole('button', { name: /^perf, \d+ tasks$/ }).click()
  await expect(page.getByText('5 tasks', { exact: true })).toBeVisible()
  await bar(page).getByRole('button', { name: 'Save as a view…' }).click()
  const dialog = page.getByRole('dialog', { name: 'Save as a view' })
  // The card's own words are refused: a name is a file name. The popover says why, and nothing is saved.
  await dialog.getByRole('textbox', { name: 'Name' }).fill(CARD_NAME)
  await dialog.getByRole('button', { name: 'Save view' }).click()
  await expect(dialog.getByRole('alert')).toContainText('cannot be a file name')
  expect(await viewNames(world, board)).not.toContain(CARD_NAME)
  expect(await slowSpotsDetected(world, board)).toBe(false)
  await dialog.getByRole('textbox', { name: 'Name' }).fill(SLOW_SPOTS)
  await dialog.getByRole('button', { name: 'Save view' }).click()
  await expect(page.getByRole('heading', { name: SLOW_SPOTS })).toBeVisible()

  await until('the saved view is Slow spots with exactly the five targets', async () => {
    return (await slowSpotsDetected(world, board)) ? true : undefined
  })
  report('S2', startedAt)
  expect(await members(world, board, SLOW_SPOTS)).toEqual([...S2_TARGET].sort())
})

test('S6 scenario 2, the wrong filter: saving with only the epic does not detect', async ({ startWorld, openPage }) => {
  const world = await startWorld(plan)
  const board = await openSlowSpotsBoard(world)
  const page = await openPage(world)
  await expect(page.locator('[data-card]').first()).toBeVisible()

  // The epic through the Epic box this time, and nothing else.
  await bar(page).getByRole('button', { name: 'Epic: Any' }).click()
  await page.getByRole('dialog', { name: 'Epics to show' }).getByRole('checkbox', { name: /Performance/ }).click()
  await page.keyboard.press('Escape')
  await bar(page).getByRole('button', { name: 'Save as a view…' }).click()
  const dialog = page.getByRole('dialog', { name: 'Save as a view' })
  await dialog.getByRole('textbox', { name: 'Name' }).fill(SLOW_SPOTS)
  await dialog.getByRole('button', { name: 'Save view' }).click()
  await expect(page.getByRole('heading', { name: SLOW_SPOTS })).toBeVisible()

  // The view is saved under the name, and it holds all thirteen tasks of E-07, so it is not the end state.
  expect(await viewNames(world, board)).toContain(SLOW_SPOTS)
  const found = await members(world, board, SLOW_SPOTS)
  expect(found).toHaveLength(13)
  expect(found.every((key) => key.startsWith('s6:T-07-'))).toBe(true)
  expect(await slowSpotsDetected(world, board)).toBe(false)
})
