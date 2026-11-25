// The task drawer in Chrome over a fake owner (ts-0184-hv): what a card opens, the folds that hold for the next task,
// the width the person drags, steps with the arrow keys and resets with a double-click, a link that opens its task,
// Escape, and the daemon's events. The page plays a scripted world (`test/support/worlds.ts`, `field-drawer`); what is
// on the page is read by role and name, as a person reads it.
import { type Locator, type Page } from '@playwright/test'
import { ELSEWHERE, PWNED } from '../test/support/hostile.ts'
import { expect, test } from './support/fixtures.ts'

const WORLD = 'field-drawer'

/** The card of a task on the board. A link in the drawer has the same words, so it is found by what a card is. */
function card(page: Page, title: string) {
  return page.locator('[data-card]', { hasText: title })
}

function drawerOf(page: Page) {
  return page.getByRole('complementary', { name: 'Task drawer' })
}

function edgeOf(page: Page) {
  return page.getByRole('separator', { name: 'Task drawer width' })
}

/** The drawer's width, which the person sets. The edge is beside it, so it is not in this. */
async function widthOf(drawer: Locator): Promise<number> {
  const box = await drawer.boundingBox()
  return Math.round(box!.width)
}

async function open(openFake: (world: string) => Promise<Page>, title = 'Design the board') {
  const page = await openFake(WORLD)
  await card(page, title).click()
  await expect(drawerOf(page).getByRole('heading', { level: 2, name: title })).toBeVisible()
  return page
}

/** Drags the edge by `by` px, to the left when negative. The edge is held where it is grabbed. */
async function drag(page: Page, by: number) {
  const box = (await edgeOf(page).boundingBox())!
  const x = box.x + box.width / 2
  const y = box.y + 40
  await page.mouse.move(x, y)
  await page.mouse.down()
  await page.mouse.move(x + by / 2, y, { steps: 4 })
  await page.mouse.move(x + by, y, { steps: 4 })
  await page.mouse.up()
}

function partBar(drawer: Locator, name: string) {
  return drawer.getByRole('button', { name, exact: true })
}

test('a card opens the drawer with its parts in order, and Close shuts it', async ({ openFake }) => {
  const page = await open(openFake)
  const drawer = drawerOf(page)
  await expect(drawer.getByRole('heading', { level: 3 })).toHaveText([
    /^Description/,
    /^Waits on/,
    /^Unblocks/,
    /^Part of/,
    /^In each copy of the project/,
    /^Details for engineers/,
  ])
  await expect(drawer).toContainText('Read only. Edit the file, or ask an agent; the board follows it.')
  await expect(drawer).toContainText('Draw the board.')
  await expect(card(page, 'Design the board')).toHaveAttribute('aria-current', 'true')
  await drawer.getByRole('button', { name: 'Close' }).click()
  await expect(drawer).toHaveCount(0)
  await expect(card(page, 'Design the board')).toBeFocused()
})

test('a scratch task has no Waits on or Unblocks, and a tracker task shows its fields by their words', async ({
  openFake,
}) => {
  const page = await open(openFake, 'Try the thing')
  const drawer = drawerOf(page)
  await expect(drawer).toContainText('Read only. Change it in scratch.')
  await expect(partBar(drawer, 'Waits on')).toHaveCount(0)
  await expect(partBar(drawer, 'Unblocks')).toHaveCount(0)

  await card(page, 'Fix the login').click()
  await expect(drawer.getByRole('heading', { level: 2, name: 'Fix the login' })).toBeVisible()
  await expect(drawer).toContainText('Priority level')
  await expect(drawer).toContainText('Story points')
  await expect(drawer.getByText('not set')).toHaveCount(2)
})

test('Escape closes the drawer, and the link window takes its own Escape first', async ({ openFake }) => {
  const page = await open(openFake, 'A hostile body')
  const drawer = drawerOf(page)
  await drawer.getByRole('button', { name: 'the docs' }).click()
  const asking = page.getByRole('dialog', { name: 'Open this link in your browser?' })
  await expect(asking).toBeVisible()
  await page.keyboard.press('Escape')
  await expect(asking).toHaveCount(0)
  await expect(drawer).toBeVisible()
  await page.keyboard.press('Escape')
  await expect(drawer).toHaveCount(0)
})

test('a fold holds for the next task and for a reload, and Details for engineers starts folded', async ({
  openFake,
}) => {
  const page = await open(openFake)
  const drawer = drawerOf(page)
  await expect(partBar(drawer, 'Details for engineers')).toHaveAttribute('aria-expanded', 'false')
  await expect(partBar(drawer, 'Description')).toHaveAttribute('aria-expanded', 'true')
  await expect(drawer.getByText('Draw the board.')).toBeVisible()

  await partBar(drawer, 'Description').click()
  await expect(partBar(drawer, 'Description')).toHaveAttribute('aria-expanded', 'false')
  await expect(drawer.getByText('Draw the board.')).toHaveCount(0)
  await partBar(drawer, 'Details for engineers').click()
  await expect(drawer.getByText('Source', { exact: true })).toBeVisible()

  await card(page, 'Build the Field').click()
  await expect(drawer.getByRole('heading', { level: 2, name: 'Build the Field' })).toBeVisible()
  await expect(partBar(drawer, 'Description')).toHaveAttribute('aria-expanded', 'false')
  await expect(drawer.getByText('Build it.')).toHaveCount(0)
  await expect(partBar(drawer, 'Details for engineers')).toHaveAttribute('aria-expanded', 'true')

  await page.reload()
  await card(page, 'Build the Field').click()
  await expect(partBar(drawer, 'Description')).toHaveAttribute('aria-expanded', 'false')
  await expect(partBar(drawer, 'Details for engineers')).toHaveAttribute('aria-expanded', 'true')
  await expect(partBar(drawer, 'Waits on')).toHaveAttribute('aria-expanded', 'true')
})

test('a link in Waits on opens that task in the drawer', async ({ openFake }) => {
  const page = await open(openFake, 'Build the Field')
  const drawer = drawerOf(page)
  await drawer.getByRole('button', { name: /Design the board/ }).click()
  await expect(drawer.getByRole('heading', { level: 2, name: 'Design the board' })).toBeVisible()
  await expect(card(page, 'Design the board')).toHaveAttribute('aria-current', 'true')
  await expect(card(page, 'Build the Field')).toHaveAttribute('aria-current', 'false')
  // Its own Waits on leads on, and Unblocks leads back.
  await drawer.getByRole('button', { name: /Write the parser/ }).click()
  await expect(drawer.getByRole('heading', { level: 2, name: 'Write the parser' })).toBeVisible()
  await expect(drawer).toContainText('Nothing. It can start.')
  await drawer.getByRole('button', { name: /Design the board/ }).click()
  await expect(drawer.getByRole('heading', { level: 2, name: 'Design the board' })).toBeVisible()
})

test('a collided id shows both copies and no "differs from main"', async ({ openFake }) => {
  const page = await open(openFake, 'Same id, one place')
  const drawer = drawerOf(page)
  await expect(drawer).toContainText('Same id, another task')
  await expect(drawer).toContainText('Same id, another place')
  await expect(drawer).toContainText('b/ts-0009-ii.md')
  await expect(drawer).toContainText('shown on the board')
  await expect(drawer).not.toContainText('differs from main')
})

test('a task whose copies differ says which copy and how', async ({ openFake }) => {
  const page = await open(openFake, 'Review the wire')
  const drawer = drawerOf(page)
  await expect(drawer).toContainText('Run r-121')
  await expect(drawer).toContainText('differs from main')
  await expect(drawer).toContainText('same as main')
})

test('the edge is dragged, and the width holds for the next task and for a reload', async ({ openFake }) => {
  const page = await open(openFake)
  const drawer = drawerOf(page)
  const edge = edgeOf(page)
  expect(await widthOf(drawer)).toBe(460)
  await expect(edge).toHaveAttribute('aria-valuenow', '460')
  await expect(edge).toHaveAttribute('aria-valuemin', '360')

  await drag(page, -100)
  expect(await widthOf(drawer)).toBe(560)
  await expect(edge).toHaveAttribute('aria-valuenow', '560')
  await drag(page, 40)
  expect(await widthOf(drawer)).toBe(520)

  await card(page, 'Build the Field').click()
  await expect(drawer.getByRole('heading', { level: 2, name: 'Build the Field' })).toBeVisible()
  expect(await widthOf(drawer)).toBe(520)

  await page.reload()
  await card(page, 'Build the Field').click()
  await expect(drawer.getByRole('heading', { level: 2, name: 'Build the Field' })).toBeVisible()
  expect(await widthOf(drawer)).toBe(520)
})

test('the drawer stays between 360 px and 70% of the window', async ({ openFake }) => {
  const page = await open(openFake)
  const drawer = drawerOf(page)
  const room = page.viewportSize()!.width
  await drag(page, -2000)
  expect(await widthOf(drawer)).toBe(Math.round(room * 0.7))
  await expect(edgeOf(page)).toHaveAttribute('aria-valuemax', String(Math.round(room * 0.7)))
  await drag(page, 2000)
  expect(await widthOf(drawer)).toBe(360)
  // Held by the keys too.
  const edge = edgeOf(page)
  await edge.focus()
  await page.keyboard.press('ArrowRight')
  expect(await widthOf(drawer)).toBe(360)
  for (let times = 0; times < 20; times += 1) await page.keyboard.press('Shift+ArrowLeft')
  expect(await widthOf(drawer)).toBe(Math.round(room * 0.7))
})

test('the arrow keys on the edge move it by 40 px, or 120 px with Shift, and a double-click resets it', async ({
  openFake,
}) => {
  const page = await open(openFake)
  const drawer = drawerOf(page)
  const edge = edgeOf(page)
  await edge.focus()
  await page.keyboard.press('ArrowLeft')
  expect(await widthOf(drawer)).toBe(500)
  await page.keyboard.press('ArrowLeft')
  expect(await widthOf(drawer)).toBe(540)
  await page.keyboard.press('ArrowRight')
  expect(await widthOf(drawer)).toBe(500)
  await page.keyboard.press('Shift+ArrowLeft')
  expect(await widthOf(drawer)).toBe(620)
  await page.keyboard.press('Shift+ArrowRight')
  expect(await widthOf(drawer)).toBe(500)
  // Other keys do nothing to it.
  await page.keyboard.press('ArrowUp')
  expect(await widthOf(drawer)).toBe(500)

  await edge.dblclick()
  expect(await widthOf(drawer)).toBe(460)
  await expect(edge).toHaveAttribute('aria-valuenow', '460')
  // A reset is kept too.
  await page.reload()
  await card(page, 'Design the board').click()
  expect(await widthOf(drawer)).toBe(460)
})

test('below 900 px the drawer lies over the board', async ({ openFake }) => {
  const page = await open(openFake)
  const drawer = drawerOf(page)
  const wrap = drawer.locator('xpath=..')
  await expect(wrap).not.toHaveCSS('position', 'fixed')
  await page.setViewportSize({ width: 800, height: 700 })
  await expect(wrap).toHaveCSS('position', 'fixed')
  const box = (await drawer.boundingBox())!
  expect(Math.round(box.x + box.width)).toBe(800)
  expect(Math.round(box.width)).toBeLessThanOrEqual(460)
  await expect(edgeOf(page)).toBeHidden()
  await page.setViewportSize({ width: 1280, height: 720 })
  await expect(wrap).not.toHaveCSS('position', 'fixed')
})

test('a window made narrower holds the width within its limit, and wider gives it back', async ({ openFake }) => {
  const page = await open(openFake)
  const drawer = drawerOf(page)
  await drag(page, -300)
  expect(await widthOf(drawer)).toBe(760)
  await page.setViewportSize({ width: 1000, height: 700 })
  await expect.poll(() => widthOf(drawer)).toBe(700)
  await page.setViewportSize({ width: 1280, height: 720 })
  await expect.poll(() => widthOf(drawer)).toBe(760)
})

test('an event for the task reads it again, and the drawer shows what the file now says', async ({ openFake }) => {
  const page = await open(openFake)
  const drawer = drawerOf(page)
  await expect(drawer).toContainText('Draw the board.')
  const gets = () => page.evaluate(() => window.collideFake.calls().filter((method) => method === 'task.get').length)
  const before = await gets()
  await page.evaluate(() =>
    window.collideFake.edit('plan:ts-0002-bb', { body: 'Draw the board again.', version: 'v2' }),
  )
  await page.evaluate(() => window.collideFake.announce('plan', 'plan:ts-0002-bb'))
  await expect(drawer).toContainText('Draw the board again.')
  expect(await gets()).toBeGreaterThan(before)
  // The drawer stayed where it was: the same task, and the width and folds as they were.
  await expect(drawer.getByRole('heading', { level: 2, name: 'Design the board' })).toBeVisible()
  await expect(page.getByRole('alert')).toHaveCount(0)
})

test('the hostile body shows as text in the drawer, and nothing leaves the page or runs', async ({ openFake }) => {
  const requests: string[] = []
  const dialogs: string[] = []
  const page = await openFake(WORLD, (made) => {
    made.on('request', (request) => requests.push(request.url()))
    made.on('dialog', (dialog) => {
      dialogs.push(dialog.message())
      void dialog.dismiss()
    })
  })
  await card(page, 'A hostile body').click()
  const drawer = drawerOf(page)
  await expect(drawer.getByRole('heading', { level: 4, name: 'Steps' })).toBeVisible()
  await expect(drawer).toContainText("<script>window.__pwned = 'script'</script>")
  await expect(drawer.locator('script, img')).toHaveCount(0)
  await expect(drawer.getByRole('button', { name: 'script link' })).toHaveCount(0)
  expect(await page.evaluate((flag) => (window as unknown as Record<string, unknown>)[flag], PWNED)).toBeUndefined()
  expect(requests.filter((url) => url.startsWith(ELSEWHERE))).toEqual([])
  expect(dialogs).toEqual([])
})
