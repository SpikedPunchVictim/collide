import { existsSync, readdirSync, readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, test } from 'vitest'
import { PACK } from './helpers.ts'

const files = (dir: string): string[] =>
  readdirSync(dir, { withFileTypes: true }).flatMap((entry) =>
    entry.isDirectory() ? files(join(dir, entry.name)).map((name) => join(entry.name, name)) : [entry.name],
  )

describe('the Orchard pack folder loads as a repo pack', () => {
  test('holds no package.json and no node_modules', () => {
    expect(existsSync(join(PACK, 'package.json'))).toBe(false)
    expect(existsSync(join(PACK, 'node_modules'))).toBe(false)
    expect(files(PACK).filter((name) => name.includes('node_modules') || name.endsWith('package.json'))).toEqual([])
  })

  test('requires SDK 0.2, the one that supplies yaml', () => {
    expect(readFileSync(join(PACK, 'pack.yaml'), 'utf8')).toMatch(/sdk: ">=0\.2 <0\.3"/)
  })

  test('imports only the SDK, node built-ins and its own files', () => {
    const imports = files(join(PACK, 'providers')).flatMap((name) =>
      [...readFileSync(join(PACK, 'providers', name), 'utf8').matchAll(/^(?:import|export) .*? from '([^']+)'/gm)].map(
        (match) => [name, match[1] as string] as const,
      ),
    )
    const allowed = (from: string) =>
      /^(node:|\.\/|\.\.\/)/.test(from) || from === '@collide/pack-sdk' || from === '@collide/pack-sdk/yaml'
    const stray = imports.filter(([, from]) => !allowed(from))
    expect(stray).toEqual([])
    expect(imports.length).toBeGreaterThan(5)
  })

  test('the provider script the manifest names exists', () => {
    expect(readFileSync(join(PACK, 'pack.yaml'), 'utf8')).toMatch(/script: providers\/plan\.ts/)
    expect(existsSync(join(PACK, 'providers', 'plan.ts'))).toBe(true)
  })
})
