// Builds what the browser tests run, once: the daemon, the shim and the bridge (cargo), and the page for the bridge
// and the page over a fake owner (vite, each to a folder of its own that is removed afterwards). The bridge is built
// with the feature that only the test build turns on, so nothing of it is in the app.
import { spawnSync } from 'node:child_process'
import { mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

const APP = join(import.meta.dirname, '..')
const REPO_ROOT = join(APP, '..')
const PAGE_PREFIX = 'collide-bridge-page-'
const FAKE_PREFIX = 'collide-fake-page-'

function run(what: string, command: string, args: string[], cwd: string) {
  const done = spawnSync(command, args, { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] })
  if (done.error !== undefined) {
    throw new Error(`${what}: could not run \`${command}\`: ${done.error.message}. Is it on the PATH?`)
  }
  if (done.status !== 0) throw new Error(`${what} failed:\n${done.stdout}\n${done.stderr}`)
}

export default function globalSetup(): () => void {
  run(
    'cargo build of the bridge and the daemon',
    'cargo',
    [
      'build', '--quiet',
      '--package', 'collide-host', '--package', 'collide-app',
      '--bin', 'collide', '--bin', 'collide-daemon', '--bin', 'collide-bridge',
      '--features', 'collide-app/test-bridge',
    ],
    REPO_ROOT,
  )
  const page = mkdtempSync(join(tmpdir(), PAGE_PREFIX))
  process.env.COLLIDE_BRIDGE_PAGE = page
  run(
    'vite build of the bridge page',
    join(APP, 'node_modules', '.bin', 'vite'),
    ['build', '--config', 'vite.bridge.config.ts', '--outDir', page, '--emptyOutDir', '--logLevel', 'warn'],
    APP,
  )
  const fake = mkdtempSync(join(tmpdir(), FAKE_PREFIX))
  process.env.COLLIDE_FAKE_PAGE = fake
  run(
    'vite build of the fake-owner page',
    join(APP, 'node_modules', '.bin', 'vite'),
    ['build', '--config', 'vite.fake.config.ts', '--outDir', fake, '--emptyOutDir', '--logLevel', 'warn'],
    APP,
  )
  return () => {
    rmSync(page, { recursive: true, force: true })
    rmSync(fake, { recursive: true, force: true })
  }
}
