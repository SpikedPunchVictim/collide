// Counts the tagged stubs under a directory: `node scripts/stubs.ts [dir]` prints one line per tag (0.15).
// The stub-tag registry of ts-0013-nz will take over the checks; this is the SDK's own count until then.
import { readdirSync, readFileSync } from 'node:fs'
import { join } from 'node:path'

const call = /\bunimplemented\(/g
const literal = /\bunimplemented\(\s*(['"])(.+?)\1\s*\)/g

/** Stubs per tag in the `.ts` files under `dir` (generated types and `.d.ts` files carry none). */
export function countStubs(dir: string): Record<string, number> {
  const counts: Record<string, number> = {}
  const files = readdirSync(dir, { recursive: true, encoding: 'utf8' }).filter((f) => f.endsWith('.ts') && !f.endsWith('.d.ts'))
  for (const file of files) {
    const text = readFileSync(join(dir, file), 'utf8')
    const calls = [...text.matchAll(call)].filter((m) => !text.slice(0, m.index).endsWith('function '))
    const tags = [...text.matchAll(literal)].map((m) => m[2]!)
    if (calls.length !== tags.length) throw new Error(`${file}: the tag of a stub must be a string literal`)
    for (const tag of tags) counts[tag] = (counts[tag] ?? 0) + 1
  }
  return counts
}

if (process.argv[1] === import.meta.filename) {
  const counts = countStubs(process.argv[2] ?? join(import.meta.dirname, '..', 'src'))
  for (const [tag, count] of Object.entries(counts).sort()) console.log(`${count}\t${tag}`)
}
