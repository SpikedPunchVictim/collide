import { execFileSync } from 'node:child_process'
import {
  cpSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  realpathSync,
  rmSync,
  statSync,
  symlinkSync,
  writeFileSync,
} from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterAll, beforeAll, expect, test } from 'vitest'

const pkgDir = join(import.meta.dirname, '..')
const tsc = join(pkgDir, 'node_modules', '.bin', 'tsc')
const pkg = JSON.parse(readFileSync(join(pkgDir, 'package.json'), 'utf8'))

/** The module specifiers a compiled JavaScript file imports, re-exports, `import()`s or `require`s. */
function specifiers(js: string): string[] {
  const found: string[] = []
  const patterns = [
    /\b(?:import|export)\s[^'"`;]*?\sfrom\s*['"]([^'"]+)['"]/g,
    /\bimport\s*['"]([^'"]+)['"]/g,
    /\bimport\s*\(\s*['"]([^'"]+)['"]\s*\)/g,
    /\brequire\s*\(\s*['"]([^'"]+)['"]\s*\)/g,
  ]
  for (const pattern of patterns) {
    for (const match of js.matchAll(pattern)) found.push(match[1]!)
  }
  return found
}

const allowed = (specifier: string) => specifier.startsWith('./') || specifier.startsWith('../') || specifier.startsWith('node:')
// The one package the SDK supplies (0.6), imported by `yaml.js` and by no other file.
const supplied = 'yaml'

function jsFiles(dir: string): string[] {
  return readdirSync(dir, { recursive: true, encoding: 'utf8' })
    .filter((file) => file.endsWith('.js'))
    .map((file) => join(dir, file))
}

// The build runs once into a temp directory, with the package's own build config, so these tests need no `dist/`.
let out: string
beforeAll(() => {
  out = mkdtempSync(join(tmpdir(), 'pack-sdk-build-'))
  execFileSync(tsc, ['-p', join(pkgDir, 'tsconfig.build.json'), '--outDir', out], { cwd: pkgDir })
  writeFileSync(join(out, 'package.json'), '{"private": true, "type": "module"}')
})
afterAll(() => rmSync(out, { recursive: true, force: true }))

// What a materialized pack sees (0.6): a folder with Collide's own `package.json`, the pack under it, and
// `node_modules/@collide/pack-sdk` linked to a compiled SDK copy that holds `yaml` in its own `node_modules`. The
// copy is made from the compiled build and the installed `yaml`, as `scripts/build-packs.ts` does. The workspace
// link is not used.
const hookSource = `
import { defineHook } from '@collide/pack-sdk'
import * as yamlExport from '@collide/pack-sdk/yaml'
const { parse, YAMLParseError } = yamlExport
await defineHook(async (input) => {
  let thrown = null
  try {
    parse('a: [')
  } catch (error) {
    thrown = error instanceof YAMLParseError
  }
  return {
    status: 'ok',
    outputs: { hook: input.hook, doc: parse('a: [1, 2]\\nb: x\\n'), names: Object.keys(yamlExport).sort(), thrown },
  }
})
`
type HookResultFile = { status: string; outputs: Record<string, unknown> }
let ranFromPlainNode: { code: number | null; stderr: string; result: HookResultFile }
beforeAll(() => {
  const root = mkdtempSync(join(tmpdir(), 'pack-sdk-staged-'))
  try {
    const sdk = join(root, 'sdk')
    mkdirSync(join(sdk, 'node_modules'), { recursive: true })
    cpSync(join(pkgDir, 'package.json'), join(sdk, 'package.json'))
    cpSync(out, join(sdk, 'dist'), { recursive: true })
    cpSync(realpathSync(join(pkgDir, 'node_modules', 'yaml')), join(sdk, 'node_modules', 'yaml'), { recursive: true })
    const entry = join(root, 'entry')
    mkdirSync(join(entry, 'node_modules', '@collide'), { recursive: true })
    mkdirSync(join(entry, 'pack'))
    writeFileSync(join(entry, 'package.json'), '{"private": true, "type": "module"}')
    symlinkSync(sdk, join(entry, 'node_modules', '@collide', 'pack-sdk'))
    writeFileSync(join(entry, 'pack', 'hook.ts'), hookSource)
    const resultPath = join(root, 'result.json')
    let code: number | null = 0
    let stderr = ''
    try {
      execFileSync(process.execPath, [join(entry, 'pack', 'hook.ts')], {
        env: { HOME: root, COLLIDE_RESULT: resultPath },
        input: JSON.stringify({ hook: 'gate' }),
        stdio: ['pipe', 'pipe', 'pipe'],
      })
    } catch (error) {
      const failure = error as { status: number | null; stderr: Buffer }
      code = failure.status
      stderr = failure.stderr.toString()
    }
    let result = { status: 'none', outputs: {} }
    try {
      result = JSON.parse(readFileSync(resultPath, 'utf8'))
    } catch {}
    ranFromPlainNode = { code, stderr, result }
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
})

test('a hook run by plain node through the SDK link imports the compiled dist and leaves its result', () => {
  expect(ranFromPlainNode.stderr).not.toContain('Error')
  expect(ranFromPlainNode.code).toBe(0)
  expect(ranFromPlainNode.result.status).toBe('ok')
  expect(ranFromPlainNode.result.outputs.hook).toBe('gate')
})

test('@collide/pack-sdk/yaml resolves through the link to the yaml in the SDK copy and parses a document', () => {
  expect(ranFromPlainNode.result.outputs.doc).toEqual({ a: [1, 2], b: 'x' })
  expect(ranFromPlainNode.result.outputs.thrown).toBe(true)
})

test('@collide/pack-sdk/yaml exports parse and YAMLParseError and nothing more', () => {
  expect(ranFromPlainNode.result.outputs.names).toEqual(['YAMLParseError', 'parse'])
})

test('the package lists one runtime dependency, yaml 2.9.1, and nothing else', () => {
  expect(pkg.dependencies).toEqual({ yaml: '2.9.1' })
  expect(Object.keys(pkg.peerDependencies ?? {})).toEqual([])
  expect(Object.keys(pkg.optionalDependencies ?? {})).toEqual([])
})

test('the compiled JavaScript imports only relative paths and node: builtins, and yaml from yaml.js alone', () => {
  const files = jsFiles(out)
  expect(files.length).toBeGreaterThan(0)
  for (const file of files) {
    const bad = specifiers(readFileSync(file, 'utf8')).filter((s) => !allowed(s))
    expect(bad, file).toEqual(file === join(out, 'yaml.js') ? [supplied] : [])
  }
})

test('the import scanner flags a bare package and a bare builtin, and passes node: and relative paths', () => {
  const js = [
    "import { x } from 'lodash'",
    "export * from 'yaml'",
    "import fs from 'fs'",
    "const y = await import('zod')",
    "const z = require('left-pad')",
    "import { a } from 'node:fs'",
    "export * from './b.js'",
    "import '../c.js'",
  ].join('\n')
  expect(specifiers(js).filter((s) => !allowed(s))).toEqual(['lodash', 'yaml', 'fs', 'zod', 'left-pad'])
})

test('the compiled output has declarations, no .ts specifiers, and loads under Node as ES modules', () => {
  expect(statSync(join(out, 'index.js')).isFile()).toBe(true)
  expect(statSync(join(out, 'index.d.ts')).isFile()).toBe(true)
  for (const file of jsFiles(out)) {
    expect(specifiers(readFileSync(file, 'utf8')).filter((s) => s.endsWith('.ts')), file).toEqual([])
  }
  const names = execFileSync(
    process.execPath,
    ['--input-type=module', '-e', `import * as sdk from ${JSON.stringify(join(out, 'index.js'))}; console.log(Object.keys(sdk).sort().join())`],
    { encoding: 'utf8' },
  ).trim()
  expect(names).toBe('defineHook,defineObserver,defineProvider,name')
})

test('package.json points main, types and exports at dist, with the yaml subpath beside the root', () => {
  expect(pkg.main).toBe('dist/index.js')
  expect(pkg.types).toBe('dist/index.d.ts')
  expect(pkg.exports).toEqual({
    '.': { types: './dist/index.d.ts', default: './dist/index.js' },
    './yaml': { types: './dist/yaml.d.ts', default: './dist/yaml.js' },
  })
})

test('the version is at least 0.2.0, the first with @collide/pack-sdk/yaml, so a pack can require it', () => {
  const [major, minor] = pkg.version.split('.').map(Number)
  expect(major! > 0 || minor! >= 2).toBe(true)
})

test('only erasable syntax compiles: an enum is refused under the package config', () => {
  const dir = mkdtempSync(join(tmpdir(), 'pack-sdk-erasable-'))
  try {
    mkdirSync(join(dir, 'src'))
    writeFileSync(join(dir, 'src', 'e.ts'), 'export enum E { A }\n')
    writeFileSync(
      join(dir, 'tsconfig.json'),
      JSON.stringify({ extends: join(pkgDir, 'tsconfig.json'), compilerOptions: { types: [] }, include: ['src'] }),
    )
    let output = ''
    try {
      execFileSync(tsc, ['-p', dir, '--noEmit'], { encoding: 'utf8', stdio: 'pipe' })
    } catch (error) {
      output = String((error as { stdout?: string }).stdout)
    }
    expect(output).toMatch(/TS1294/)
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})
