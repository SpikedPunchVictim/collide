// Run by lock.test.ts as its own process: reads the lock file in a loop until a time, and reports on stdout how
// often it found the file and every content it found that was not a whole pid.
// Usage: node lock-reader.ts <lock file> <end time in ms since epoch>
import { readFileSync } from 'node:fs'

const [lockPath, endAt] = process.argv.slice(2) as [string, string]
let held = 0
const bad = new Set<string>()
while (Date.now() < Number(endAt)) {
  try {
    const text = readFileSync(lockPath, 'utf8')
    held++
    if (!/^[0-9]+\n$/.test(text)) bad.add(text)
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error
  }
}
console.log(JSON.stringify({ held, bad: [...bad] }))
