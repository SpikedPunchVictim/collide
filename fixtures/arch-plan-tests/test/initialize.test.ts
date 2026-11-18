import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { Ajv2020 } from 'ajv/dist/2020.js'
import { describe, expect, test } from 'vitest'
import { connect, REPO, tree } from './helpers.ts'

const initialize = async () => {
  const client = connect()
  try {
    return (await client.request('initialize', { config: {}, root: tree('basic') })).result as Record<string, any>
  } finally {
    await client.close()
  }
}

describe('initialize', () => {
  test('is ref-scoped, has the id patterns and the labelId template field', async () => {
    const result = await initialize()
    expect(result.scope).toBe('ref')
    expect(result.idPattern).toBe('^T-[0-9]{2}-[0-9]{3}-[a-z0-9]+(-[a-z0-9]+)*$')
    expect(result.shortIdPattern).toBe('^T-[0-9]{2}-[0-9]{3}$')
    expect(result.templateFields).toEqual([{ name: 'labelId', pattern: '^t\\d{2}-\\d{3}$' }])
    expect(new RegExp(result.templateFields[0].pattern).test('t03-001')).toBe(true)
    expect(new RegExp(result.shortIdPattern).test('T-03-001')).toBe(true)
    expect(new RegExp(result.idPattern).test('T-03-001-survey-the-quay')).toBe(true)
  })

  test('declares the six statuses in order, each with its category', async () => {
    const result = await initialize()
    expect(result.statuses).toEqual([
      { name: 'specified', category: 'backlog' },
      { name: 'ready', category: 'ready' },
      { name: 'doing', category: 'active' },
      { name: 'in_review', category: 'review' },
      { name: 'done', category: 'done' },
      { name: 'dropped', category: 'dropped' },
    ])
  })

  test('says which status each category writes: active writes doing, review writes in_review', async () => {
    const result = await initialize()
    expect(result.categoryStatus).toEqual({
      backlog: 'specified',
      ready: 'ready',
      active: 'doing',
      review: 'in_review',
      done: 'done',
      dropped: 'dropped',
    })
  })

  test('declares the fields and the blocked-by link', async () => {
    const result = await initialize()
    expect(result.fields).toEqual([
      { id: 'proposal', type: 'string', filterable: true },
      { id: 'epic', type: 'string', filterable: true },
    ])
    expect(result.links).toEqual([{ name: 'blocked-by', dir: 'out', blocking: true }])
  })

  test('declares setStatus and create, and the capabilities: watch, create, preview, collections', async () => {
    const result = await initialize()
    expect(result.ops.map((op: { name: string }) => op.name)).toEqual(['setStatus', 'create'])
    expect(result.capabilities).toEqual({ watch: true, create: true, collections: true, preview: true })
  })

  const argsSchema = async (name: string) => {
    const result = await initialize()
    const op = result.ops.find((candidate: { name: string }) => candidate.name === name)
    return new Ajv2020({ strict: false, allErrors: true }).compile(op.args)
  }

  test.each([
    [{ status: 'doing' }, true],
    [{ category: 'active' }, true],
    [{}, false],
    [{ status: 'doing', category: 'active' }, false],
    [{ status: 'started' }, false],
    [{ category: 'blocked' }, false],
    [{ status: 'doing', force: true }, false],
  ])('the setStatus args schema takes one of status or category: %j is %s', async (args, valid) => {
    expect((await argsSchema('setStatus'))(args)).toBe(valid)
  })

  test.each([
    [{ proposal: 'P03', title: 'Chart', epic: 'E-07' }, true],
    [{ proposal: 'P03', title: 'Chart', epic: 'E-07-harbor-plan', dependsOn: ['T-03-001'] }, true],
    [{ title: 'Chart', epic: 'E-07' }, false],
    [{ proposal: 'P03', epic: 'E-07' }, false],
    [{ proposal: 'P03', title: 'Chart' }, false],
    [{ proposal: '3', title: 'Chart', epic: 'E-07' }, false],
    [{ proposal: 'P03', title: 'Chart', epic: 'harbor' }, false],
    [{ proposal: 'P03', title: '', epic: 'E-07' }, false],
    [{ proposal: 'P03', title: 'Chart', epic: 'E-07', dependsOn: ['harbor'] }, false],
    [{ proposal: 'P03', title: 'Chart', epic: 'E-07', owner: 'me' }, false],
  ])('the create args schema: %j is %s', async (args, valid) => {
    expect((await argsSchema('create'))(args)).toBe(valid)
  })

  test('validates against the generated InitializeResult schema', async () => {
    const schema = JSON.parse(readFileSync(join(REPO, 'schemas', 'InitializeResult.output.json'), 'utf8'))
    const validate = new Ajv2020({ strict: false, allErrors: true }).compile(schema)
    const result = await initialize()
    expect(validate(result), JSON.stringify(validate.errors)).toBe(true)
  })

  test('the schema check can fail: a result with an unknown category does not validate', async () => {
    const schema = JSON.parse(readFileSync(join(REPO, 'schemas', 'InitializeResult.output.json'), 'utf8'))
    const validate = new Ajv2020({ strict: false, allErrors: true }).compile(schema)
    const result = await initialize()
    expect(validate({ ...result, statuses: [{ name: 'doing', category: 'busy' }] })).toBe(false)
  })

  test('a root that is missing or not absolute is an error reply, and list before initialize is one too', async () => {
    const client = connect()
    try {
      expect((await client.request('list')).error).toMatchObject({ code: -32603 })
      expect((await client.request('initialize', { config: {} })).error?.message).toMatch(/root/)
      expect((await client.request('initialize', { config: {}, root: 'relative/path' })).error?.message).toMatch(/root/)
    } finally {
      await client.close()
    }
  })
})
