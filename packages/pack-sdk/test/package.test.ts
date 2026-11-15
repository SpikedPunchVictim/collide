import { execFileSync } from 'node:child_process'
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs'
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

test('the package lists no runtime dependencies', () => {
  expect(Object.keys(pkg.dependencies ?? {})).toEqual([])
  expect(Object.keys(pkg.peerDependencies ?? {})).toEqual([])
  expect(Object.keys(pkg.optionalDependencies ?? {})).toEqual([])
})

test('the compiled JavaScript imports only relative paths and node: builtins', () => {
  const files = jsFiles(out)
  expect(files.length).toBeGreaterThan(0)
  for (const file of files) {
    const bad = specifiers(readFileSync(file, 'utf8')).filter((s) => !allowed(s))
    expect(bad, file).toEqual([])
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

test('package.json points main, types and exports at dist', () => {
  expect(pkg.main).toBe('dist/index.js')
  expect(pkg.types).toBe('dist/index.d.ts')
  expect(pkg.exports).toEqual({ '.': { types: './dist/index.d.ts', default: './dist/index.js' } })
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
