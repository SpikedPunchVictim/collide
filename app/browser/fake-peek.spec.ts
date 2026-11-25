// The id peek in Chrome over a fake owner (ts-0189-fg; note 34): an id in a task's text is a link, the pointer over it
// shows the task, a click keeps it, an id inside it opens in the same window with Back, and Escape or a click away
// closes it. The page plays the `field-peek` world (`test/support/changes-world.ts`): `Write the parser` names
// `ts-0002-bb`, which names `ts-0003-cc`, whose long body ends by naming `ts-0001-aa`. What is on the page is read by
// role and name, as a person reads it.
import { type Page } from '@playwright/test'
import { expect, test } from './support/fixtures.ts'

const WORLD = 'field-peek'

const drawerOf = (page: Page) => page.getByRole('complementary', { name: 'Task drawer' })
const peekOf = (page: Page, id: string) => page.getByRole('dialog', { name: `Task ${id}` })
const idIn = (scope: ReturnType<typeof drawerOf>, id: string) => scope.getByRole('button', { name: id, exact: true })

async function open(openFake: (world: string) => Promise<Page>) {
  const page = await openFake(WORLD)
  await page.locator('[data-card]', { hasText: 'Write the parser' }).click()
  await expect(drawerOf(page).getByRole('heading', { level: 2, name: 'Write the parser' })).toBeVisible()
  // The ids are links once task.get has found them.
  await expect(idIn(drawerOf(page), 'ts-0002-bb')).toBeVisible()
  return page
}

test('an id of a task is a link, and one that no task has stays text', async ({ openFake }) => {
  const page = await open(openFake)
  const drawer = drawerOf(page)
  await expect(idIn(drawer, 'ts-0003-cc')).toBeVisible()
  await expect(idIn(drawer, 'ts-9999-zz')).toHaveCount(0)
  await expect(drawer).toContainText('ts-9999-zz that is no task')
  await expect(drawer).toContainText('version 1.2')
  await expect(idIn(drawer, 'ts-0004-dd')).toHaveCount(0)
  // A link to the web in the same text is still a link to the web: the id inside its words is not an id.
  await expect(drawer.getByRole('button', { name: 'ts-0005-ee as link text' })).toBeVisible()
})

test('the pointer over an id shows the task: its state, title, what it waits on and its body', async ({ openFake }) => {
  const page = await open(openFake)
  await idIn(drawerOf(page), 'ts-0002-bb').hover()
  const peek = peekOf(page, 'ts-0002-bb')
  await expect(peek).toBeVisible()
  await expect(peek).toContainText('Design the board')
  await expect(peek).toContainText('ready')
  await expect(peek).toContainText('Waits on Write the parser (ts-0001-aa).')
  await expect(peek).toContainText('Draw the board, then ask about')
})

test('the peek goes when the pointer leaves, unless a click kept it', async ({ openFake }) => {
  const page = await open(openFake)
  await idIn(drawerOf(page), 'ts-0002-bb').hover()
  await expect(peekOf(page, 'ts-0002-bb')).toBeVisible()
  await page.mouse.move(2, 2)
  await expect(peekOf(page, 'ts-0002-bb')).toHaveCount(0)

  await idIn(drawerOf(page), 'ts-0002-bb').click()
  await expect(peekOf(page, 'ts-0002-bb')).toBeVisible()
  await page.mouse.move(2, 2)
  // Longer than the peek takes to go when it is not kept.
  await page.waitForTimeout(800)
  await expect(peekOf(page, 'ts-0002-bb')).toBeVisible()
})

test('an id inside the peek opens in the same window, and Back returns', async ({ openFake }) => {
  const page = await open(openFake)
  await idIn(drawerOf(page), 'ts-0002-bb').click()
  const first = peekOf(page, 'ts-0002-bb')
  await idIn(first, 'ts-0003-cc').click()
  await expect(peekOf(page, 'ts-0002-bb')).toHaveCount(0)
  const second = peekOf(page, 'ts-0003-cc')
  await expect(second).toContainText('Build the Field')
  await expect(second.getByRole('button', { name: 'Back' })).toBeVisible()
  // Its long body scrolls inside the window, and names the first task again.
  const body = second.locator('[data-peek-body]')
  expect(await body.evaluate((element) => element.scrollHeight > element.clientHeight)).toBe(true)
  await body.evaluate((element) => element.scrollTo(0, element.scrollHeight))
  await expect(idIn(second, 'ts-0001-aa')).toBeInViewport()
  await second.getByRole('button', { name: 'Back' }).click()
  await expect(peekOf(page, 'ts-0002-bb')).toBeVisible()
  await expect(peekOf(page, 'ts-0003-cc')).toHaveCount(0)
})

test('Escape closes the peek and not the drawer, and the next Escape closes the drawer', async ({ openFake }) => {
  const page = await open(openFake)
  await idIn(drawerOf(page), 'ts-0002-bb').click()
  await expect(peekOf(page, 'ts-0002-bb')).toBeVisible()
  await page.keyboard.press('Escape')
  await expect(peekOf(page, 'ts-0002-bb')).toHaveCount(0)
  await expect(drawerOf(page)).toBeVisible()
  await page.keyboard.press('Escape')
  await expect(drawerOf(page)).toHaveCount(0)
})

test('a click away closes a kept peek', async ({ openFake }) => {
  const page = await open(openFake)
  await idIn(drawerOf(page), 'ts-0002-bb').click()
  await expect(peekOf(page, 'ts-0002-bb')).toBeVisible()
  await drawerOf(page).getByRole('heading', { level: 2 }).click()
  await expect(peekOf(page, 'ts-0002-bb')).toHaveCount(0)
  await expect(drawerOf(page)).toBeVisible()
})

test('a task is asked of task.get once however often its id is shown', async ({ openFake }) => {
  const page = await open(openFake)
  const gets = () => page.evaluate(() => window.collideFake.calls().filter((call) => call === 'task.get').length)
  const through = async () => {
    await idIn(drawerOf(page), 'ts-0002-bb').click()
    await idIn(peekOf(page, 'ts-0002-bb'), 'ts-0003-cc').click()
    // The long body names the first task again, and every word of it that could be an id is asked about once.
    await expect(idIn(peekOf(page, 'ts-0003-cc'), 'ts-0001-aa')).toBeVisible()
    await peekOf(page, 'ts-0003-cc').getByRole('button', { name: 'Close' }).click()
  }
  await through()
  const before = await gets()
  await through()
  await through()
  expect(await gets()).toBe(before)
})
