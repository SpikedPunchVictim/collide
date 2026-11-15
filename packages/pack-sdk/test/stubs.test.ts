import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { expect, test } from 'vitest'
import { countStubs } from '../scripts/stubs.ts'
import { defineHook, defineObserver } from '../src/index.ts'
import { unimplemented } from '../src/unimplemented.ts'

const pkgDir = join(import.meta.dirname, '..')
const registry: Record<string, string> = JSON.parse(readFileSync(join(pkgDir, 'stub-tags.json'), 'utf8'))

test('unimplemented throws an error that names the tag', () => {
  expect(() => unimplemented('sdk: something')).toThrow('unimplemented: sdk: something')
})

test.each([
  ['sdk: defineHook', () => defineHook(async () => {})],
  ['sdk: defineObserver', () => defineObserver(async () => {})],
])('%s is a stub that throws its own tag', (tag, call) => {
  expect(call).toThrow(`unimplemented: ${tag}`)
})

test('every stub in src carries a tag of the form "sdk: <name>"', () => {
  const counts = countStubs(join(pkgDir, 'src'))
  expect(Object.keys(counts).length).toBeGreaterThan(0)
  for (const tag of Object.keys(counts)) expect(tag).toMatch(/^sdk: [A-Za-z][A-Za-z.]*$/)
})

test('the stubs in src and the tags in stub-tags.json are the same set', () => {
  const counts = countStubs(join(pkgDir, 'src'))
  expect(Object.keys(counts).sort()).toEqual(Object.keys(registry).sort())
})

test('every registered tag names the stage that fills it', () => {
  for (const [tag, stage] of Object.entries(registry)) {
    expect(stage, tag).toMatch(/^Stage \d+[a-z]?: \S/)
  }
})

test('defineProvider is no longer a stub: its tag is gone from the source and the registry', () => {
  expect(countStubs(join(pkgDir, 'src'))['sdk: defineProvider']).toBeUndefined()
  expect(registry['sdk: defineProvider']).toBeUndefined()
})

test('the counter sees an unregistered tag and refuses a tag that is not a string literal', () => {
  const dir = mkdtempSync(join(tmpdir(), 'pack-sdk-stubs-'))
  try {
    writeFileSync(join(dir, 'a.ts'), "export const a = () => unimplemented('sdk: nope')\nexport const b = () => unimplemented(\"sdk: nope\")\n")
    expect(countStubs(dir)).toEqual({ 'sdk: nope': 2 })
    writeFileSync(join(dir, 'c.ts'), 'export const c = (tag: string) => unimplemented(tag)\n')
    expect(() => countStubs(dir)).toThrow('string literal')
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})
