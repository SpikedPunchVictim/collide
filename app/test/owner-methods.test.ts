import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { expect, test } from 'vitest'
import { ASK_METHODS, OWNER_METHODS, SHELL_METHODS, UI_TOOLS_METHODS } from '../src/owner/methods.ts'

const root = join(import.meta.dirname, '..', '..')

/** The method names of `core/src/owner.rs`'s `methods!` list. */
function ownerMethodNames(): string[] {
  const source = readFileSync(join(root, 'core', 'src', 'owner.rs'), 'utf8')
  const list = source.slice(source.indexOf('methods! {'), source.indexOf('/// What [`route`] found.'))
  return [...list.matchAll(/\w+, "([a-z_.]+)", \w+,\s+"[^"]*",/g)].map((match) => match[1]!)
}

test('the table holds every method of the owner protocol except initialize', () => {
  const names = ownerMethodNames()
  expect(names).toContain('initialize')
  expect(Object.keys(OWNER_METHODS).sort()).toEqual(names.filter((name) => name !== 'initialize').sort())
})

test('the window has no initialize', () => {
  const all = [
    ...Object.keys(OWNER_METHODS),
    ...Object.keys(ASK_METHODS),
    ...Object.keys(UI_TOOLS_METHODS),
    ...Object.keys(SHELL_METHODS),
  ]
  expect(all).not.toContain('initialize')
})

test('the ask and ui_tools methods are the ones core names', () => {
  const ask = readFileSync(join(root, 'core', 'src', 'ask.rs'), 'utf8')
  const uiTools = readFileSync(join(root, 'core', 'src', 'ui_tools.rs'), 'utf8')
  for (const name of Object.keys(ASK_METHODS)) expect(ask).toContain(`"${name}"`)
  for (const name of Object.keys(UI_TOOLS_METHODS)) expect(uiTools).toContain(`"${name}"`)
  expect(Object.keys(ASK_METHODS)).toHaveLength(3)
  expect(Object.keys(UI_TOOLS_METHODS)).toHaveLength(3)
})

test('the shell methods are the ones the shell answers, and all of them start with `shell.`', () => {
  const shell = readFileSync(join(root, 'app', 'src-tauri', 'src', 'shell.rs'), 'utf8')
  expect(Object.keys(SHELL_METHODS)).toEqual(['shell.open_external'])
  for (const name of Object.keys(SHELL_METHODS)) {
    expect(name.startsWith('shell.')).toBe(true)
    expect(shell).toContain(`pub const OPEN_EXTERNAL: &str = "${name}";`)
  }
  expect(shell).toContain('const SHELL_PREFIX: &str = "shell.";')
})
