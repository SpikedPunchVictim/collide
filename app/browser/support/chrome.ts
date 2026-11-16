// The installed Google Chrome, headless, with a temp profile of its own. No browser is downloaded: the tests use the
// Chrome that is on the machine, never the person's own profile, and fail with a message when there is none.
import { type BrowserContext, chromium } from '@playwright/test'
import { mkdtempSync, rmSync } from 'node:fs'
import { join } from 'node:path'
import { processesNaming } from './world.ts'

const PROFILE_PREFIX = 'cdb-chrome-'

/** Every name but the loopback ones is unresolvable, so a page can reach nothing off the machine. */
const LOOPBACK_ONLY = 'MAP * ~NOTFOUND, EXCLUDE 127.0.0.1, EXCLUDE localhost'

export class Chrome {
  private constructor(
    readonly context: BrowserContext,
    private readonly profile: string,
  ) {}

  static async launch(): Promise<Chrome> {
    const profile = mkdtempSync(join('/tmp', PROFILE_PREFIX))
    try {
      const context = await chromium.launchPersistentContext(profile, {
        channel: 'chrome',
        headless: true,
        args: [
          `--host-resolver-rules=${LOOPBACK_ONLY}`,
          '--disable-background-networking',
          '--disable-component-update',
          '--disable-sync',
          '--no-first-run',
          '--no-default-browser-check',
        ],
      })
      return new Chrome(context, profile)
    } catch (failure) {
      rmSync(profile, { recursive: true, force: true })
      throw new Error(
        'Google Chrome could not be started. These tests drive the Chrome that is installed on the machine ' +
          '(channel "chrome") and never download a browser, so Chrome has to be installed. ' +
          `Playwright said: ${failure instanceof Error ? failure.message : String(failure)}`,
      )
    }
  }

  /** Closes Chrome and removes its profile. Gives what of Chrome is left, which is nothing when it ended. */
  async close(): Promise<string[]> {
    await this.context.close().catch(() => undefined)
    // Chrome's helpers end a moment after the browser does.
    const until = Date.now() + 5_000
    let left = processesNaming(this.profile)
    while (left.length > 0 && Date.now() < until) {
      await new Promise((resolve) => setTimeout(resolve, 50))
      left = processesNaming(this.profile)
    }
    rmSync(this.profile, { recursive: true, force: true })
    return left.map((p) => `${p.pid} ${p.command}`)
  }
}
