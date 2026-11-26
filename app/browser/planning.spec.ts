// The Planning tab over a real daemon (ts-0195-ju): a temp repository with plan items, and a proposal that is only on
// a branch, checked out as a worktree in the same temp root. The page's counts are compared with what the command line
// prints for the same board: `collide tasks --json` and `collide sections --json`, each run by the test with the
// environment of the world and nothing else (never the person's), and git config files that are empty.
import { type Page } from '@playwright/test'
import { execFileSync } from 'node:child_process'
import { mkdirSync, realpathSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { expect, test } from './support/fixtures.ts'
import { type Listing, WITHIN, start, trust, until, write } from './support/octo-board.ts'
import { COLLIDE_EXE, type World, git } from './support/world.ts'

const BRANCH = 'plan/ui-first'
const EPIC_DIR = '.collide/planning/epics/ep-001-aa-alpha'
const EPIC = '---\nid: ep-001-aa\nkind: epic\nstatus: active\ntitle: "Made-up epic"\nowners: []\nlinks: []\n---\n'

const proposal = (id: string, title: string, status: string, features: string) =>
  `---\nid: ${id}\nkind: proposal\nstatus: ${status}\ntitle: "${title}"\nowners: []\nlinks: []\n---\n\n` +
  `## Features\n\n${features}`

const task = (id: string, status: string, title: string, links: string) =>
  `---\nid: ${id}\nkind: task\nstatus: ${status}\ntitle: "${title}"\nowners: []\nlinks:${links}\n---\n\n## Goal\n`

const ON_MAIN = proposal(
  'pr-0001-aa',
  'Board UI first',
  'draft',
  '### Field\n\n- **F1 Columns**: one.\n- **F2 Saved views**: two.\n\n### Settings\n\n- **F3 Settings page**: three.\n',
)

// Only on the branch. Its `Setting` is near the `Settings` above, its `field` is the `Field` above in another case,
// and its third feature is before any heading, so it has no section.
const ON_BRANCH = proposal(
  'pr-0002-bb',
  'Plan from the branch',
  'draft',
  '- **F3 Loose end**: three.\n\n### Setting\n\n- **F1 Settings view**: one.\n\n### field\n\n- **F2 Filters**: two.\n',
)

interface Printed {
  tasks: {
    key: string
    kind?: string | null
    state: { category: string }
    collections: string[]
    features?: { section: string }[]
  }[]
  collections: { id: string; title: string }[]
  branch_only: { key: string; checkout: string; branch?: string }[]
}

interface PrintedSections {
  sections: { name: string; featureCount: number }[]
  nearDuplicates: { names: string[] }[]
}

/** The command, run as a person runs it in the repository, with the environment of the world and nothing else. */
function collide(world: World, ...args: string[]): unknown {
  const empty = join(world.root, 'empty.gitconfig')
  writeFileSync(empty, '')
  const printed = execFileSync(COLLIDE_EXE, args, {
    cwd: world.repo,
    env: {
      PATH: '/usr/bin:/bin',
      HOME: world.root,
      COLLIDE_DAEMON_ROOT: world.root,
      GIT_CONFIG_GLOBAL: empty,
      GIT_CONFIG_SYSTEM: empty,
    },
    encoding: 'utf8',
    timeout: WITHIN,
  })
  return JSON.parse(printed)
}

/** The repository and its worktree: the epic, two tasks and a proposal on main, and a proposal on the branch only. */
async function fill(world: World): Promise<{ board: string; worktree: string }> {
  write(world, `${EPIC_DIR}/epic.md`, EPIC)
  write(world, `${EPIC_DIR}/proposals/pr-0001-aa-board.md`, ON_MAIN)
  write(
    world,
    `${EPIC_DIR}/tasks/ts-0001-aa-columns.md`,
    task('ts-0001-aa', 'done', 'Draw the columns', '\n  - { type: implements, to: pr-0001-aa#F1 }'),
  )
  write(world, `${EPIC_DIR}/tasks/ts-0002-bb-views.md`, task('ts-0002-bb', 'ready', 'Save a view', ' []'))
  git(world.repo, 'add', '.')
  git(world.repo, 'commit', '-q', '-m', 'the plan')
  const worktree = join(world.root, 'wt-plan')
  git(world.repo, 'worktree', 'add', '-q', '-b', BRANCH, worktree)
  const file = join(worktree, EPIC_DIR, 'proposals', 'pr-0002-bb-branch.md')
  mkdirSync(dirname(file), { recursive: true })
  writeFileSync(file, ON_BRANCH)
  git(worktree, 'add', '--all')
  git(worktree, 'commit', '-q', '-m', 'the proposal on the branch')

  const opened = await world.rpc<{ board: { id: string } }>('board.open', { root: world.repo, init_git: false })
  const board = opened.board.id
  await trust(world, board)
  await until('the proposal on the branch is listed', async () => {
    const list = await world.rpc<Listing>('task.list', { board })
    const keys = list.tasks.map((each) => each.key)
    return !list.refreshing && keys.includes('plan:pr-0002-bb') && keys.includes('plan:ts-0002-bb') ? list : undefined
  })
  return { board, worktree: realpathSync(worktree) }
}

async function openPlanning(page: Page) {
  await expect(page.getByRole('region', { name: 'Field' })).toBeVisible()
  await page.keyboard.press('4')
  await expect(page.getByRole('heading', { name: 'Planning', level: 1 })).toBeVisible()
}

test('a branch-only proposal has the chip, and the counts match the command line', async ({
  startWorld,
  openPage,
}) => {
  const world = await start(startWorld)
  const { worktree } = await fill(world)
  const tasks = collide(world, 'tasks', '--json') as Printed
  const sections = collide(world, 'sections', '--json') as PrintedSections
  const proposals = tasks.tasks.filter((each) => each.kind === 'proposal')
  expect(proposals.map((each) => each.key).sort()).toEqual(['plan:pr-0001-aa', 'plan:pr-0002-bb'])
  expect(tasks.branch_only).toEqual([{ key: 'plan:pr-0002-bb', checkout: worktree, branch: BRANCH }])

  const page = await openPage(world)
  await openPlanning(page)

  // The chip: the proposal that is only on the branch has it, and the one on main does not.
  const row = (key: string) => page.locator(`[data-proposal="${key}"]`)
  await expect(row('plan:pr-0002-bb')).toContainText(`only on branch ${BRANCH}`)
  await expect(row('plan:pr-0001-aa')).not.toContainText('only on branch')
  await expect(page.locator('[data-proposal]')).toHaveCount(proposals.length)

  // Each proposal's feature and section counts are those of the command line's listing.
  for (const each of proposals) {
    const features = each.features ?? []
    const named = new Set(features.map((each) => each.section.trim().toLowerCase()).filter((name) => name !== ''))
    const words = `${features.length} ${features.length === 1 ? 'feature' : 'features'}`
    await expect(row(each.key)).toContainText(`${named.size} ${named.size === 1 ? 'section' : 'sections'}, ${words}`)
  }

  // The sections and their feature counts are `collide sections --json`'s, and the rows add up.
  const total = sections.sections.reduce((all, each) => all + each.featureCount, 0)
  expect(total, 'the fixture has features').toBeGreaterThan(0)
  await expect(page.locator('tr[data-feature]')).toHaveCount(total)
  for (const section of sections.sections) {
    const name = section.name === '' ? 'Unsorted' : section.name
    await expect(page.locator(`tr[data-section="${name}"]`)).toHaveCount(section.featureCount)
  }
  for (const group of sections.nearDuplicates) {
    for (const name of group.names) {
      await expect(page.locator(`tr[data-section="${name}"]`).first()).toContainText('looks like')
    }
  }

  // The epic tiles are the collections the command line lists, counted as the strip counts them. A plan pack staged
  // before epics were collections lists none, and the checks below would test nothing.
  expect(tasks.collections.length, 'the command line lists the epic of the fixture').toBeGreaterThan(0)
  await expect(page.locator('[data-epic]')).toHaveCount(tasks.collections.length)
  for (const collection of tasks.collections) {
    const inEpic = tasks.tasks.filter((each) => each.collections.includes(collection.id))
    const mine = new Map(inEpic.map((each) => [each.key, each]))
    const done = [...mine.values()].filter((each) => each.state.category === 'done').length
    await expect(page.locator(`[data-epic="${collection.id}"]`)).toContainText(`${done} of ${mine.size} done`)
  }
})
