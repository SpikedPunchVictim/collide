// Run by lock.test.ts as its own process: waits for a start time, takes the lock, holds it, and logs what it did.
// Usage: node lock-worker.ts <lock file> <log file> <start time in ms since epoch> <hold ms>
import { appendFileSync } from 'node:fs'
import { setTimeout as sleep } from 'node:timers/promises'
import { withLock } from '../src/lock.ts'

const [lockPath, logPath, startAt, holdMs] = process.argv.slice(2) as [string, string, string, string]
const log = (line: string): void => appendFileSync(logPath, `${line}\n`)

while (Date.now() < Number(startAt)) await sleep(1)
await withLock(
  lockPath,
  async () => {
    log(`enter ${process.pid}`)
    await sleep(Number(holdMs))
    log(`exit ${process.pid}`)
  },
  { onTakeover: (pid) => log(`takeover ${pid}`) },
)
