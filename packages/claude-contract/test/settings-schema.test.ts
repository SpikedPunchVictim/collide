import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { Ajv } from 'ajv'
import { expect, test } from 'vitest'

// The adapter's generated settings.json against Claude's published schema, offline (ts-0068-wx). The Rust test
// `settings_golden` keeps the golden file equal to what the adapter generates; this test validates the golden file.
const fixtures = fileURLToPath(new URL('../../../host/tests/adapters/claude/fixtures/', import.meta.url))
const read = (name: string): unknown => JSON.parse(readFileSync(`${fixtures}${name}`, 'utf8'))

// The schema is draft-07 and names formats and keywords of its own; none of them is needed to accept or refuse a shape.
const validator = () => new Ajv({ strict: false, allErrors: true, validateFormats: false }).compile(read('claude-code-settings.schema.json') as object)

test('the generated settings.json is valid against the published schema', () => {
  const validate = validator()
  const valid = validate(read('generated-settings.golden.json'))
  expect(validate.errors).toBeNull()
  expect(valid).toBe(true)
})

test('a settings file the schema refuses is refused, so the check can fail', () => {
  const validate = validator()
  expect(validate({ autoMemoryEnabled: 'no' })).toBe(false)
  expect(validate({ hooks: { SessionStart: [{ hooks: [{ type: 'command', command: 5 }] }] } })).toBe(false)
})
