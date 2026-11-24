// The Field over a real daemon (ts-0181-ft): a temp repository with a pack that declares eight statuses (the host's
// corpus fixture) and plan tasks, in a world of its own (`support/world.ts`). The page is read by role and name. What
// the daemon holds is read through the owner's protocol, and the command line's answer is what `collide tasks` prints,
// run by the test with the environment of the world and nothing else, never the person's.
import { type Page } from '@playwright/test'
import { execFileSync } from 'node:child_process'
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { expect, test } from './support/fixtures.ts'
import { firstPhase } from './support/recordings.ts'
import { COLLIDE_EXE, type Plan, SOURCE_ROOT, type World, git } from './support/world.ts'

const PROVIDER = readFileSync(join(SOURCE_ROOT, 'host/tests/fixtures/corpus_pack_provider.mjs'), 'utf8')

const WITHIN = 60_000

const EIGHT = [
  ['idea', 'backlog'],
  ['triage', 'backlog'],
  ['queued', 'ready'],
  ['doing', 'active'],
  ['in_review', 'review'],
  ['qa', 'review'],
  ['done', 'done'],
  ['dropped', 'dropped'],
] as const

/** The `declare` setting of the eight-status pack: the statuses, with `renamed` in place of a name. */
function declaration(renamed: Record<string, string> = {}): string {
  return JSON.stringify({
    statuses: EIGHT.map(([name, category]) => ({ name: renamed[name] ?? name, category })),
    categoryStatus: { ready: 'queued', active: 'doing', review: 'in_review' },
  })
}

const PACK = [
  'apiVersion: collide/v0',
  'name: octo',
  'version: 1.0.0',
  'providers:',
  '  eight: { script: providers/eight.mjs }',
  'config:',
  `  declare: { type: string, default: '${declaration()}' }`,
  '',
].join('\n')

const EPIC = '---\nid: ep-001-aa\nkind: epic\nstatus: active\ntitle: "Made-up epic"\nowners: []\nlinks: []\n---\n'

function planTask(id: string, status: string, title: string): string {
  return `---\nid: ${id}\nkind: task\nstatus: ${status}\ntitle: "${title}"\nowners: []\nlinks: []\n---\n\n## Goal\n`
}

const PLAN_TASKS = [
  ['ts-0001-aa', 'ready', 'Write the parser'],
  ['ts-0002-bb', 'active', 'Design the board'],
  ['ts-0003-cc', 'active', 'Build the Field'],
  ['ts-0004-dd', 'review', 'Plan the drawer'],
  ['ts-0005-ee', 'done', 'Ship the shell'],
  ['ts-0006-ff', 'review', 'Review the wire'],
] as const

const EIGHT_TASKS = [
  ['1', 'idea', 'Think about it'],
  ['2', 'qa', 'Check it twice'],
  ['3', 'in_review', 'Check it once'],
  ['4', 'done', 'Finished work'],
  ['5', 'queued', 'Waits its turn'],
] as const

/** The MVP view of `fixtures/views/edge-mvp.yaml`, over this repository's own tasks and with the same kinds of rule. */
const MVP = [
  'name: edge-mvp',
  "where: \"status == 'ready' || status == 'active'\"",
  'include: [plan:ts-0004-dd, github:acme/edge#212]',
  'exclude: [plan:ts-0002-bb]',
  'layout: { default: board, groupBy: category, sort: { by: id, dir: asc } }',
  '',
].join('\n')

const OCTO_ONLY = `name: octo-only\ninclude: [${EIGHT_TASKS.map(([short]) => `eight:${short}`).join(', ')}]\n`

function write(world: World, path: string, text: string) {
  const file = join(world.repo, path)
  mkdirSync(dirname(file), { recursive: true })
  writeFileSync(file, text)
}

/** The repository: the pack, its tasks, the plan's tasks and the two views, committed. */
function fill(world: World) {
  write(world, '.collide/packs/octo/pack.yaml', PACK)
  write(
    world,
    '.collide/packs/octo/providers/eight.mjs',
    PROVIDER.replaceAll('__SOURCE__', 'eight').replaceAll('__WATCH__', 'false').replaceAll('__PAGE__', '100'),
  )
  write(world, '.collide/board.yaml', 'packs:\n  - source: repo:.collide/packs/octo\n')
  write(world, '.collide/planning/epics/ep-001-aa-alpha/epic.md', EPIC)
  for (const [id, status, title] of PLAN_TASKS) {
    write(world, `.collide/planning/epics/ep-001-aa-alpha/tasks/${id}-work.md`, planTask(id, status, title))
  }
  for (const [short, status, title] of EIGHT_TASKS) {
    write(world, `tasks/${short}.json`, `${JSON.stringify({ short, title, status, version: 'v1' })}\n`)
  }
  write(world, '.collide/views/edge-mvp.yaml', MVP)
  write(world, '.collide/views/octo-only.yaml', OCTO_ONLY)
  git(world.repo, 'add', '.')
  git(world.repo, 'commit', '-q', '-m', 'the fixture')
}

interface Listing {
  readonly invalid?: unknown[]
  readonly tasks: { key: string }[]
  readonly refreshing: boolean
  readonly sources: { name: string; statuses: { name: string }[] }[]
}

/** Calls `read` until it gives something, for as long as the deadline allows. */
async function until<T>(
  what: string,
  read: () => Promise<T | undefined | false>,
  seen: () => string = () => '',
): Promise<T> {
  const end = Date.now() + WITHIN
  for (;;) {
    const found = await read()
    if (found) return found
    if (Date.now() > end) throw new Error(`timed out after ${WITHIN} ms waiting for: ${what}. Last seen: ${seen()}`)
    await new Promise((resolve) => setTimeout(resolve, 100))
  }
}

/** Opens the repository as a board and trusts its line, as the owner does in the app. Returns the board's id. */
async function openBoard(world: World): Promise<string> {
  const opened = await world.rpc<{ board: { id: string } }>('board.open', { root: world.repo, init_git: false })
  const board = opened.board.id
  const review = await world.rpc<{ trust: { hash: string } }>('board.trust_review', { board, line: 'main' })
  await world.rpc('board.trust', { board, line: 'main', hash: review.trust.hash, hosts: [], dev_folders: [] })
  let last: Listing | undefined
  await until(
    'every source has listed',
    async () => {
      last = await world.rpc<Listing>('task.list', { board })
      // The plan lists its epic as a task too, and a board's own scratch source is always there.
      const names = last.sources.map((source) => source.name)
      return !last.refreshing &&
        last.tasks.length === PLAN_TASKS.length + EIGHT_TASKS.length + 1 &&
        names.includes('eight') &&
        names.includes('plan')
        ? last
        : undefined
    },
    () =>
      JSON.stringify({
        tasks: last?.tasks.map((task) => task.key),
        invalid: last?.invalid,
        sources: last?.sources.map((source) => source.name),
      }),
  )
  return board
}

function start(startWorld: (plan: Plan) => Promise<World>) {
  return startWorld({ first: firstPhase([]), path: [dirname(process.execPath)] })
}

function column(page: Page, name: string) {
  return page.getByRole('region', { name, exact: true })
}

/** The columns that are open, in order. */
function columns(page: Page): Promise<(string | null)[]> {
  return page
    .locator('section[data-column]')
    .evaluateAll((all) => all.map((each) => each.getAttribute('aria-label')))
}

async function chooseView(page: Page, name: string) {
  await page.getByRole('button', { name: 'All tasks', exact: true }).click()
  await page.getByRole('button', { name: new RegExp(`^${name}`) }).click()
  await expect(page.getByRole('heading', { name })).toBeVisible()
}

test('the columns are the declared statuses of one source, and the six categories of a mixed board', async ({
  startWorld,
  openPage,
}) => {
  const world = await start(startWorld)
  fill(world)
  await openBoard(world)
  const page = await openPage(world)

  // The whole board has two sources, so its columns are the categories; Done and Dropped start folded.
  await expect(column(page, 'Backlog')).toBeVisible()
  expect(await columns(page)).toEqual(['Backlog', 'Ready', 'Active', 'In review'])
  await expect(page.getByRole('button', { name: /^Done\s*\d+$/ })).toBeVisible()

  // A view over the eight-status source alone has that source's own statuses, in its order, each with its category.
  await chooseView(page, 'octo-only')
  await expect(column(page, 'Idea')).toBeVisible()
  expect(await columns(page)).toEqual(['Idea', 'Triage', 'Queued', 'Doing', 'In_review', 'Qa'])
  await expect(column(page, 'Idea')).toContainText('counts as backlog')
  await expect(column(page, 'Qa')).toContainText('counts as in review')
  await expect(page.getByRole('button', { name: /^Done\s*1$/ })).toBeVisible()
  await expect(page.getByRole('button', { name: /^Dropped\s*0$/ })).toBeVisible()
  await expect(column(page, 'Qa').getByRole('button', { name: /Check it twice/ })).toBeVisible()
})

test('a rename in the declaration restarts the source and changes the columns', async ({ startWorld, openPage }) => {
  const world = await start(startWorld)
  fill(world)
  const board = await openBoard(world)
  const page = await openPage(world)
  await chooseView(page, 'octo-only')
  await expect(column(page, 'Triage')).toBeVisible()

  await world.rpc('config.set', {
    key: 'octo.declare',
    value: declaration({ triage: 'sorting' }),
    layer: 'user',
    board,
  })
  await until('the source declares the new name', async () => {
    const list = await world.rpc<Listing>('task.list', { board })
    const eight = list.sources.find((source) => source.name === 'eight')
    return eight?.statuses.some((status) => status.name === 'sorting') && !list.refreshing ? list : undefined
  })

  // The Field reads when a board or a view is chosen, so a person looks again.
  await page.reload()
  await chooseView(page, 'octo-only')
  await expect(column(page, 'Sorting')).toBeVisible()
  expect(await columns(page)).toEqual(['Idea', 'Sorting', 'Queued', 'Doing', 'In_review', 'Qa'])
  await expect(column(page, 'Triage')).toHaveCount(0)
  await expect(column(page, 'Qa').getByRole('button', { name: /Check it twice/ })).toBeVisible()
})

test('the MVP view shows the tasks that `collide tasks --view edge-mvp --json` lists', async ({
  startWorld,
  openPage,
}) => {
  const world = await start(startWorld)
  fill(world)
  await openBoard(world)

  // The command, run as a person runs it in the repository, with the environment of the world and nothing else.
  const printed = execFileSync(COLLIDE_EXE, ['tasks', '--view', 'edge-mvp', '--json'], {
    cwd: world.repo,
    env: { PATH: '/usr/bin:/bin', HOME: world.root, COLLIDE_DAEMON_ROOT: world.root },
    encoding: 'utf8',
    timeout: WITHIN,
  })
  const document = JSON.parse(printed) as { tasks: { key: string }[] }
  const expected = document.tasks.map((task) => task.key).sort()
  expect(expected.length, 'the view holds tasks').toBeGreaterThan(0)
  expect(expected).toContain('plan:ts-0004-dd')
  expect(expected).not.toContain('plan:ts-0002-bb')

  const page = await openPage(world)
  await chooseView(page, 'edge-mvp')
  await expect(page.getByText(`${expected.length} task${expected.length === 1 ? '' : 's'}`)).toBeVisible()
  const shown = await page
    .locator('[data-card]')
    .evaluateAll((all) => all.map((each) => (each.getAttribute('data-card') ?? '').split('|')[0]!))
  expect(shown.sort()).toEqual(expected)
  // The view names an item that is no task, and the line above the board says so.
  await expect(page.getByRole('status', { name: 'About this view' })).toContainText('github:acme/edge#212')
})
