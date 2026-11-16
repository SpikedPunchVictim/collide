// The fixtures of a browser test: a world (a temp root, fake Claude and the bridge over them) and Chrome with a profile
// of its own. Both are ended when the test ends, whether it passed or not, and a process of either that is left makes
// the test fail.
import { type Page, expect, test as base } from '@playwright/test'
import { Chrome } from './chrome.ts'
import { type Plan, World } from './world.ts'

interface Fixtures {
  /** Starts a world that plays `plan`. */
  startWorld: (plan: Plan) => Promise<World>
  /** A page of Chrome on the bridge of `world`, with the token the bridge made for this run. */
  openPage: (world: World) => Promise<Page>
}

export const test = base.extend<Fixtures>({
  startWorld: async ({}, use, testInfo) => {
    const worlds: World[] = []
    await use(async (plan) => {
      const world = await World.start(plan)
      worlds.push(world)
      return world
    })
    for (const world of worlds) {
      if (testInfo.status !== testInfo.expectedStatus) console.log(world.diagnostics())
      const left = await world.stop()
      expect(left, 'nothing of the world is left running').toEqual([])
    }
  },

  openPage: async ({}, use) => {
    let chrome: Chrome | null = null
    await use(async (world) => {
      chrome ??= await Chrome.launch()
      const page = await chrome.context.newPage()
      await page.goto(`${world.url}/#token=${world.token}`)
      return page
    })
    if (chrome !== null) {
      const left = await (chrome as Chrome).close()
      expect(left, 'nothing of Chrome is left running').toEqual([])
    }
  },
})

export { expect }
