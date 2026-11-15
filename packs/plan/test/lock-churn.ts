// Run by lock.test.ts as its own process: takes and releases the lock the given number of times.
// Usage: node lock-churn.ts <lock file> <times>
import { setTimeout as sleep } from 'node:timers/promises'
import { withLock } from '../src/lock.ts'

const [lockPath, times] = process.argv.slice(2) as [string, string]
for (let index = 0; index < Number(times); index++) await withLock(lockPath, () => sleep(3))
