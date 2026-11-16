// The browser tests (ts-0082-fp): the thin loop in the installed Chrome, over the test-only bridge. `pnpm test` runs
// them after the unit tests. Nothing is downloaded: the browser is the Chrome that is on the machine.
import { defineConfig } from '@playwright/test'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

export default defineConfig({
  testDir: './browser',
  testMatch: '*.spec.ts',
  globalSetup: './browser/global-setup.ts',
  // One world at a time: each has a daemon, and the plan's own tests time out under load.
  workers: 1,
  fullyParallel: false,
  retries: 0,
  forbidOnly: true,
  timeout: 90_000,
  expect: { timeout: 30_000 },
  reporter: 'list',
  // Playwright empties this folder at the start of a run, so it is one of its own and not in the repository.
  outputDir: join(tmpdir(), 'collide-playwright-output'),
})
