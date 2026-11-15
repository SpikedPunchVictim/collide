// The Node half of `pnpm gen`: turns the JSON Schemas that the Rust half wrote into TypeScript.
// Keep the pairs in step with test/generated.test.ts.
import { mkdirSync, readdirSync, rmSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { generate } from './generate.ts'

const sdk = join(import.meta.dirname, '..')
const root = join(sdk, '..', '..')

const pairs = [
  { schemas: join(root, 'schemas'), generated: join(sdk, 'src', 'generated') },
  { schemas: join(root, 'core', 'tests', 'fixtures', 'schemas'), generated: join(sdk, 'test', 'fixtures', 'generated') },
]

for (const { schemas, generated } of pairs) {
  const files = await generate(schemas)
  mkdirSync(generated, { recursive: true })
  for (const stale of readdirSync(generated).filter((name) => name.endsWith('.ts'))) {
    rmSync(join(generated, stale))
  }
  for (const [name, text] of Object.entries(files)) {
    writeFileSync(join(generated, name), text)
  }
}
