// The run order and the graph over a real daemon (ts-0187-hb): a temp repository with plan tasks that wait on each
// other and a view with a closure, in a world of its own (`support/world.ts`). What the page draws is compared with
// what `collide tasks --view to-the-wire --json` reports, run by the test with the environment of the world and nothing
// else, never the person's. The goal picker over the plan's epics is in `fake-graph.spec.ts`: the plan pack the bridge
// runs is the staged one, which reports no collections until it is built again.
import { type Page } from '@playwright/test'
import { execFileSync } from 'node:child_process'
import { mkdirSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { expect, test } from './support/fixtures.ts'
import { firstPhase } from './support/recordings.ts'
import { COLLIDE_EXE, type Plan, type World, git } from './support/world.ts'

const WITHIN = 60_000

const EPIC = '---\nid: ep-001-aa\nkind: epic\nstatus: active\ntitle: "Made-up epic"\nowners: []\nlinks: []\n---\n'

function planTask(id: string, status: string, title: string, blockedBy: string[]): string {
  const links = blockedBy.map((to) => `  - { type: blocked-by, to: ${to} }`).join('\n')
  const block = blockedBy.length === 0 ? 'links: []' : `links:\n${links}`
  return `---\nid: ${id}\nkind: task\nstatus: ${status}\ntitle: "${title}"\nowners: []\n${block}\n---\n\n## Goal\n`
}

/** The parser holds up the design and the build, the design holds up the build, and the build holds up the review. */
const TASKS = [
  ['ts-0001-aa', 'ready', 'Write the parser', []],
  ['ts-0002-bb', 'ready', 'Design the board', ['ts-0001-aa']],
  ['ts-0003-cc', 'active', 'Build the Field', ['ts-0001-aa', 'ts-0002-bb', 'ts-0005-ee']],
  ['ts-0004-dd', 'ready', 'Review the wire', ['ts-0003-cc']],
  ['ts-0005-ee', 'done', 'Ship the shell', []],
  ['ts-0006-ff', 'ready', 'Not on the way', []],
] as const satisfies readonly (readonly [string, string, string, string[]])[]

/** The lines the graph draws: one for each blocking link between two tasks of the walk. The finished task has none. */
const LINES = ['ts-0001-aa>ts-0002-bb', 'ts-0001-aa>ts-0003-cc', 'ts-0002-bb>ts-0003-cc', 'ts-0003-cc>ts-0004-dd']

const VIEW = [
  'name: to-the-wire',
  'closure: { goal: plan:ts-0004-dd, follow: depends_on }',
  'layout: { default: graph }',
  '',
].join('\n')

function write(world: World, path: string, text: string) {
  const file = join(world.repo, path)
  mkdirSync(dirname(file), { recursive: true })
  writeFileSync(file, text)
}

function fill(world: World) {
  write(world, '.collide/planning/epics/ep-001-aa-alpha/epic.md', EPIC)
  for (const [id, status, title, blockedBy] of TASKS) {
    const file = `.collide/planning/epics/ep-001-aa-alpha/tasks/${id}-work.md`
    write(world, file, planTask(id, status, title, [...blockedBy]))
  }
  write(world, '.collide/views/to-the-wire.yaml', VIEW)
  git(world.repo, 'add', '.')
  git(world.repo, 'commit', '-q', '-m', 'the fixture')
}

interface Listing {
  readonly tasks: { key: string }[]
  readonly refreshing: boolean
  readonly view?: { steps: string[][]; cycle: string[] }
}

/** Opens the repository as a board, trusts its line as the owner does in the app, and waits for the plan to list. */
async function openBoard(world: World): Promise<string> {
  const opened = await world.rpc<{ board: { id: string } }>('board.open', { root: world.repo, init_git: false })
  const board = opened.board.id
  const review = await world.rpc<{ trust: { hash: string } }>('board.trust_review', { board, line: 'main' })
  await world.rpc('board.trust', { board, line: 'main', hash: review.trust.hash, hosts: [], dev_folders: [] })
  const end = Date.now() + WITHIN
  for (;;) {
    const listed = await world.rpc<Listing>('task.list', { board })
    // The plan lists its epic as a task too.
    if (!listed.refreshing && listed.tasks.length === TASKS.length + 1) return board
    const keys = JSON.stringify(listed.tasks.map((task) => task.key))
    if (Date.now() > end) throw new Error(`the plan did not list: ${keys}`)
    await new Promise((resolve) => setTimeout(resolve, 100))
  }
}

function start(startWorld: (plan: Plan) => Promise<World>) {
  return startWorld({ first: firstPhase([]), path: [dirname(process.execPath)] })
}

/** What the command prints for the view, as a person runs it in the repository. */
function printed(world: World): Listing {
  const text = execFileSync(COLLIDE_EXE, ['tasks', '--view', 'to-the-wire', '--json'], {
    cwd: world.repo,
    env: { PATH: '/usr/bin:/bin', HOME: world.root, COLLIDE_DAEMON_ROOT: world.root },
    encoding: 'utf8',
    timeout: WITHIN,
  })
  return JSON.parse(text) as Listing
}

async function chooseView(page: Page) {
  await page.getByRole('button', { name: 'All tasks', exact: true }).click()
  await page.getByRole('button', { name: /^to-the-wire/ }).click()
  await expect(page.getByRole('heading', { name: 'to-the-wire' })).toBeVisible()
}

const short = (key: string) => key.replace('plan:', '')

test('a view with a closure opens in the graph, as the command reports its steps, with a line for each link', async ({
  startWorld,
  openPage,
}) => {
  const world = await start(startWorld)
  fill(world)
  await openBoard(world)
  const reported = printed(world)
  const steps = reported.view!.steps
  const named = [['ts-0001-aa'], ['ts-0002-bb'], ['ts-0003-cc'], ['ts-0004-dd']]
  expect(steps.map((step) => step.map(short))).toEqual(named)

  const page = await openPage(world)
  await chooseView(page)
  await expect(page.getByRole('group', { name: 'Layout' }).getByRole('button', { name: 'Graph' })).toHaveAttribute(
    'aria-pressed',
    'true',
  )
  await expect(page.getByRole('group', { name: 'Goal' })).toContainText('Review the wire')
  await expect(page.getByText('4 open tasks in 4 steps.')).toBeVisible()

  // The tasks are in columns, one for each step the command reports and in its order.
  const columns = await page
    .locator('[data-card]')
    .evaluateAll((all) =>
      all.map((each) => [(each as HTMLElement).style.left, (each.getAttribute('data-card') ?? '').split('|')[0]!]),
    )
  const lefts = [...new Set(columns.map(([left]) => left))].sort((a, b) => parseInt(a!) - parseInt(b!))
  expect(lefts.map((left) => columns.filter(([at]) => at === left).map(([, key]) => key))).toEqual(steps)

  const lines = await page
    .locator('svg path[data-from]')
    .evaluateAll((all) => all.map((each) => `${each.getAttribute('data-from')}>${each.getAttribute('data-to')}`))
  expect(lines.map((line) => line.replaceAll('plan:', '')).sort()).toEqual(LINES)
})

test('the run order has the same steps, and a task opens in the drawer', async ({ startWorld, openPage }) => {
  const world = await start(startWorld)
  fill(world)
  await openBoard(world)
  const steps = printed(world).view!.steps

  const page = await openPage(world)
  await chooseView(page)
  await page.getByRole('group', { name: 'Layout' }).getByRole('button', { name: 'Run order' }).click()
  await expect(page.getByText(`${steps.flat().length} open tasks in ${steps.length} steps.`)).toBeVisible()
  const shown = await page
    .getByRole('list', { name: 'Run order' })
    .locator('> li')
    .evaluateAll((rows) =>
      rows.map((row) =>
        [...row.querySelectorAll('[data-card]')].map((each) => (each.getAttribute('data-card') ?? '').split('|')[0]!),
      ),
    )
  expect(shown).toEqual(steps)

  await page.locator('[data-card]', { hasText: 'Build the Field' }).click()
  await expect(page.getByRole('complementary', { name: 'Task drawer' })).toContainText('Build the Field')
})

test('the daemon walks the same steps for the view as an inline closure, which is how a goal is asked for', async ({
  startWorld,
}) => {
  const world = await start(startWorld)
  fill(world)
  const board = await openBoard(world)
  const steps = printed(world).view!.steps
  const view = { name: 'x', closure: { goal: 'plan:ts-0004-dd', follow: 'depends_on' } }
  const answer = await world.rpc<Listing>('task.list', { board, view: { type: 'inline', view }, limit: 1 })
  expect(answer.view?.steps).toEqual(steps)
  // A goal that is no task and no collection is refused with the words the page shows.
  const refused = { ...view, closure: { goal: 'plan:ts-9999-zz', follow: 'depends_on' } }
  const asked = world.rpc('task.list', { board, view: { type: 'inline', view: refused } })
  await expect(asked).rejects.toThrow(/matches no task/)
})
