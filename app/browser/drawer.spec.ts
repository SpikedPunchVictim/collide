// The task drawer over a real daemon (ts-0184-hv): a temp repository with plan tasks and a scratch task, in a world of
// its own (`support/world.ts`). The drawer shows what `task.get` gives, a plan task and a scratch task, and the hostile
// fixture task of ts-0183-ee shows as text under the bridge page's content security policy, and fetches nothing.
import { type Page } from '@playwright/test'
import { mkdirSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { ELSEWHERE, HOSTILE_BODY, PWNED } from '../test/support/hostile.ts'
import { expect, test } from './support/fixtures.ts'
import { firstPhase } from './support/recordings.ts'
import { type Plan, type World, git } from './support/world.ts'

const EPIC = '---\nid: ep-001-aa\nkind: epic\nstatus: active\ntitle: "Made-up epic"\nowners: []\nlinks: []\n---\n'

const taskFile = (id: string) => `.collide/planning/epics/ep-001-aa-alpha/tasks/${id}-work.md`

function planTask(id: string, status: string, title: string, body: string, blockedBy: string[] = []): string {
  const each = (to: string) => `  - { type: blocked-by, to: ${to} }`
  const links = blockedBy.length === 0 ? ' []' : `\n${blockedBy.map(each).join('\n')}`
  const head = `---\nid: ${id}\nkind: task\nstatus: ${status}\ntitle: "${title}"\nowners: []\nlinks:${links}\n---`
  return `${head}\n\n${body}\n`
}

function write(world: World, path: string, text: string) {
  const file = join(world.repo, path)
  mkdirSync(dirname(file), { recursive: true })
  writeFileSync(file, text)
}

/** The repository: an epic, three plan tasks (the second waits on the first), and the hostile one. Committed. */
function fill(world: World) {
  write(world, '.collide/planning/epics/ep-001-aa-alpha/epic.md', EPIC)
  const parser = planTask('ts-0001-aa', 'ready', 'Write the parser', '## Goal\n\nParse the **files**.')
  write(world, taskFile('ts-0001-aa'), parser)
  write(
    world,
    taskFile('ts-0002-bb'),
    planTask('ts-0002-bb', 'ready', 'Design the board', '## Goal\n\nDraw the board.', ['ts-0001-aa']),
  )
  write(world, taskFile('ts-0003-cc'), planTask('ts-0003-cc', 'active', 'Hostile fixture', HOSTILE_BODY))
  git(world.repo, 'add', '.')
  git(world.repo, 'commit', '-q', '-m', 'the plan')
}

/** Opens the repository as a board and trusts its line, as the owner does in the app. Returns the board's id. */
async function openBoard(world: World): Promise<string> {
  const opened = await world.rpc<{ board: { id: string } }>('board.open', { root: world.repo, init_git: false })
  const board = opened.board.id
  const review = await world.rpc<{ trust: { hash: string } }>('board.trust_review', { board, line: 'main' })
  await world.rpc('board.trust', { board, line: 'main', hash: review.trust.hash, hosts: [], dev_folders: [] })
  return board
}

function start(startWorld: (plan: Plan) => Promise<World>) {
  return startWorld({ first: firstPhase([]), path: [dirname(process.execPath)] })
}

function card(page: Page, title: string) {
  return page.locator('[data-card]', { hasText: title })
}

function drawerOf(page: Page) {
  return page.getByRole('complementary', { name: 'Task drawer' })
}

test('a plan task opens with its parts, read from the daemon', async ({ startWorld, openPage }) => {
  const world = await start(startWorld)
  fill(world)
  await openBoard(world)
  const page = await openPage(world)

  await card(page, 'Design the board').click()
  const drawer = drawerOf(page)
  await expect(drawer.getByRole('heading', { level: 2, name: 'Design the board' })).toBeVisible()
  await expect(drawer).toContainText('Read only. Edit the file, or ask an agent; the board follows it.')
  await expect(drawer).toContainText('Draw the board.')

  // The task it waits on, with its state and its title, and a link that opens it.
  const waits = drawer.locator('[data-part="waits"]')
  await expect(waits).toContainText('Write the parser')
  await expect(waits).toContainText('ready')
  // Its epic, by the epic's own title, and the copy in the main checkout.
  await expect(drawer.locator('[data-part="part-of"]')).toContainText('Made-up epic')
  await expect(drawer.locator('[data-part="copies"]')).toContainText('shown on the board')
  await expect(drawer.locator('[data-part="copies"]')).not.toContainText('differs from main')

  // The details name the file and the board.
  await drawer.getByRole('button', { name: /Details for engineers/ }).click()
  const details = drawer.locator('[data-part="details"]')
  await expect(details).toContainText('plan')
  await expect(details).toContainText('ts-0002-bb')
  await expect(details).toContainText('line main')

  // The link opens the other task, which waits on nothing and is waited on by this one.
  await waits.getByRole('button', { name: /Write the parser/ }).click()
  await expect(drawer.getByRole('heading', { level: 2, name: 'Write the parser' })).toBeVisible()
  await expect(drawer).toContainText('Nothing. It can start.')
  await expect(drawer.locator('[data-part="unblocks"]')).toContainText('Design the board')
  // The fold made on one task holds on the next.
  await expect(drawer.getByRole('button', { name: /Details for engineers/ })).toHaveAttribute('aria-expanded', 'true')
  await expect(page.getByRole('alert')).toHaveCount(0)
})

test('a scratch task opens with no Waits on or Unblocks', async ({ startWorld, openPage }) => {
  const world = await start(startWorld)
  fill(world)
  const board = await openBoard(world)
  await world.rpc('task.apply', {
    board,
    source: 'scratch',
    apply: { op: 'create', args: { prompt: 'Try the scratch thing\n\nA note to myself.' } },
  })
  const page = await openPage(world)

  await card(page, 'Try the scratch thing').click()
  const drawer = drawerOf(page)
  await expect(drawer.getByRole('heading', { level: 2, name: 'Try the scratch thing' })).toBeVisible()
  await expect(drawer).toContainText('Read only. Change it in scratch.')
  await expect(drawer).toContainText('A note to myself.')
  await expect(drawer.getByRole('button', { name: 'Waits on' })).toHaveCount(0)
  await expect(drawer.getByRole('button', { name: 'Unblocks' })).toHaveCount(0)
  await expect(drawer).not.toContainText('Nothing. It can start.')
  await expect(drawer.getByRole('button', { name: 'Description' })).toBeVisible()
})

test('the hostile fixture task shows as text, and fetches nothing, under the content security policy', async ({
  startWorld,
  openPage,
}) => {
  const world = await start(startWorld)
  fill(world)
  await openBoard(world)
  const requests: string[] = []
  const dialogs: string[] = []
  const policyErrors: string[] = []
  const page = await openPage(world, (made) => {
    made.on('request', (request) => requests.push(request.url()))
    made.on('dialog', (dialog) => {
      dialogs.push(dialog.message())
      void dialog.dismiss()
    })
    made.on('console', (message) => {
      if (/Content Security Policy/i.test(message.text())) policyErrors.push(message.text())
    })
  })

  await card(page, 'Hostile fixture').click()
  const drawer = drawerOf(page)
  await expect(drawer.getByRole('heading', { level: 4, name: 'Steps' })).toBeVisible()
  await expect(drawer).toContainText(`<script>window.${PWNED} = 'script'</script>`)
  await expect(drawer).toContainText('link not opened')
  await expect(drawer.locator('script, img')).toHaveCount(0)
  await expect(page.getByRole('link')).toHaveCount(0)
  await expect(drawer.getByRole('button', { name: 'script link' })).toHaveCount(0)
  expect(await page.evaluate((flag) => (window as unknown as Record<string, unknown>)[flag], PWNED)).toBeUndefined()
  expect(requests.filter((url) => url.startsWith(ELSEWHERE))).toEqual([])
  expect(dialogs).toEqual([])
  // Normal use is not refused by the policy either.
  expect(policyErrors).toEqual([])
})
