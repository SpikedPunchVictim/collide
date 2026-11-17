import { expect, test } from 'vitest'
import { createHookContext } from '../src/context.ts'
import { defineObserver } from '../src/index.ts'
import type { HookInputOutput } from '../src/index.ts'
import { unimplemented } from '../src/unimplemented.ts'

// The count of stubs per tag, and the registry that maps each tag to its stage, are the repository's: see
// `stub-tags.json` and `pnpm check:stubs`.

test('unimplemented throws an error that names the tag', () => {
  expect(() => unimplemented('sdk: something')).toThrow('unimplemented: sdk: something')
})

const ctx = createHookContext({} as HookInputOutput)

test.each([
  ['sdk: ctx.result', () => ctx.ok()],
  ['sdk: ctx.report', () => ctx.report.metric('tests.passed', 1)],
  ['sdk: defineObserver', () => defineObserver(async () => {})],
])('%s is a stub that throws its own tag', (tag, call) => {
  expect(call).toThrow(`unimplemented: ${tag}`)
})
