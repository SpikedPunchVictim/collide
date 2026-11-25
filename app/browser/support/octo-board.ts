// The repository the Field's bridge specs share (ts-0181-ft, ts-0190-tz): a temp git repository with a pack that
// declares eight statuses (the host's corpus fixture), the plan's tasks and two views, opened as a board of a world
// (`world.ts`) and trusted, as the owner does in the app.
import { execFileSync } from 'node:child_process'
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { firstPhase } from './recordings.ts'
import { COLLIDE_EXE, type Plan, SOURCE_ROOT, type World, git } from './world.ts'

const PROVIDER = readFileSync(join(SOURCE_ROOT, 'host/tests/fixtures/corpus_pack_provider.mjs'), 'utf8')

export const WITHIN = 60_000

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
export function declaration(renamed: Record<string, string> = {}): string {
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

export const PLAN_TASKS = [
  ['ts-0001-aa', 'ready', 'Write the parser'],
  ['ts-0002-bb', 'active', 'Design the board'],
  ['ts-0003-cc', 'active', 'Build the Field'],
  ['ts-0004-dd', 'review', 'Plan the drawer'],
  ['ts-0005-ee', 'done', 'Ship the shell'],
  ['ts-0006-ff', 'review', 'Review the wire'],
] as const

export const EIGHT_TASKS = [
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

export function write(world: World, path: string, text: string) {
  const file = join(world.repo, path)
  mkdirSync(dirname(file), { recursive: true })
  writeFileSync(file, text)
}

/** The repository: the pack, its tasks (with the `labels` given by short id), the plan's tasks and views, committed. */
export function fill(world: World, labels: Readonly<Record<string, readonly string[]>> = {}) {
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
    const task = { short, title, status, version: 'v1', labels: labels[short] }
    write(world, `tasks/${short}.json`, `${JSON.stringify(task)}\n`)
  }
  write(world, '.collide/views/edge-mvp.yaml', MVP)
  write(world, '.collide/views/octo-only.yaml', OCTO_ONLY)
  git(world.repo, 'add', '.')
  git(world.repo, 'commit', '-q', '-m', 'the fixture')
}

export interface Listing {
  readonly invalid?: unknown[]
  readonly tasks: { key: string }[]
  readonly refreshing: boolean
  readonly sources: { name: string; statuses: { name: string }[] }[]
}

/** Calls `read` until it gives something, for as long as the deadline allows. */
export async function until<T>(
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

/** Trusts the line as it now is. A commit to the Collide folder changes what is trusted, so it is done again then. */
export async function trust(world: World, board: string): Promise<void> {
  const review = await world.rpc<{ trust: { hash: string } }>('board.trust_review', { board, line: 'main' })
  await world.rpc('board.trust', { board, line: 'main', hash: review.trust.hash, hosts: [], dev_folders: [] })
}

/** Opens the repository as a board and trusts its line, as the owner does in the app. Returns the board's id. */
export async function openBoard(world: World): Promise<string> {
  const opened = await world.rpc<{ board: { id: string } }>('board.open', { root: world.repo, init_git: false })
  const board = opened.board.id
  await trust(world, board)
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

export function start(startWorld: (plan: Plan) => Promise<World>) {
  return startWorld({ first: firstPhase([]), path: [dirname(process.execPath)] })
}
