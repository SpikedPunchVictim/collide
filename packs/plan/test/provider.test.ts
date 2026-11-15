import { readdirSync, renameSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, test } from 'vitest'
import { BODY_CAP, FRONTMATTER_CAP } from '../src/read-file.ts'
import { call, callUninitialized, connect, copyFixture, EPIC_FILE, fixture, frontmatter, makeRepo } from './helpers.ts'

const EPICS = '.collide/planning/epics'

const idsOf = (result: Record<string, any> | undefined): string[] => (result?.tasks as Array<{ id: string }>).map((task) => task.id).sort()
const taskOf = (result: Record<string, any> | undefined, id: string): Record<string, any> => (result?.tasks as Array<Record<string, any>>).find((task) => task.id === id)!
const linksOf = (result: Record<string, any> | undefined, id: string) => taskOf(result, id).links

describe('initialize', () => {
  test('declares the kinds task and epic, the five categories as statuses, and blocked-by as the one blocking link', async () => {
    const { result } = await call(fixture('basic'), 'initialize', { config: {} })
    const categories = ['ready', 'active', 'review', 'done', 'dropped'].map((name) => ({ name, category: name }))
    expect(result?.kinds).toEqual([
      { name: 'task', statuses: categories },
      { name: 'epic', statuses: categories },
    ])
    expect(result?.statuses).toEqual(categories)
    expect(result?.links.filter((link: any) => link.blocking)).toEqual([{ name: 'blocked-by', dir: 'out', blocking: true }])
    expect(result?.idPattern).toBe('^[a-z]{2}-[0-9]{3,}-[a-z]{2}$')
    expect(result?.capabilities).toEqual({ watch: false, create: true, collections: false, preview: false })
  })
})

describe('list over a tree of items', () => {
  test('lists the epic files and the task files directly in tasks/, and reads nothing else', async () => {
    const { result, invalid } = await call(fixture('basic'), 'list')
    expect(idsOf(result)).toEqual(['ep-001-aa', 'ep-002-bb', 'ts-0001-aa', 'ts-0002-bb', 'ts-0003-cc', 'ts-0004-dd'])
    expect(invalid).toEqual([])
  })

  test('reports the epic from the folder as a part-of link, and keeps the stored blocked-by links', async () => {
    const { result } = await call(fixture('basic'), 'list')
    expect(linksOf(result, 'ts-0002-bb')).toEqual([
      { type: 'part-of', to: 'ep-001-aa' },
      { type: 'blocked-by', to: 'ts-0001-aa' },
    ])
    expect(linksOf(result, 'ts-0003-cc')).toEqual([
      { type: 'part-of', to: 'ep-002-bb' },
      { type: 'blocked-by', to: 'ts-0001-aa' },
      { type: 'blocked-by', to: 'ts-0002-bb' },
    ])
    expect(linksOf(result, 'ep-001-aa')).toEqual([])
  })

  test('maps a status to its own category, and carries kind, alias, key, path and a content hash as the version', async () => {
    const { result } = await call(fixture('basic'), 'list')
    expect(taskOf(result, 'ts-0003-cc')).toMatchObject({
      key: 'plan:ts-0003-cc',
      kind: 'task',
      state: { category: 'review', status: 'review' },
      path: `${EPICS}/ep-002-bb-beta/tasks/ts-0003-cc-third.md`,
    })
    expect(taskOf(result, 'ts-0002-bb').aliases).toEqual(['A2'])
    expect(taskOf(result, 'ts-0002-bb').version).toMatch(/^[0-9a-f]{64}$/)
  })

  test('leaves the body out of list and returns it from get', async () => {
    const { result } = await call(fixture('basic'), 'list')
    expect(taskOf(result, 'ts-0001-aa')).not.toHaveProperty('body')
    const got = await call(fixture('basic'), 'get', { id: 'ts-0001-aa' })
    expect(got.result?.task.body).toContain('Made-up content for ts-0001-aa.')
  })

  test('counts an item with an unknown kind or a broken link, and still lists it', async () => {
    const { result, invalid, logs } = await call(fixture('basic'), 'list')
    expect(taskOf(result, 'ts-0004-dd')).toMatchObject({ kind: 'widget' })
    expect(linksOf(result, 'ts-0004-dd')).toContainEqual({ type: 'blocked-by', to: 'ts-0999-zz' })
    expect(invalid).toEqual([])
    expect(logs).toEqual(['plan: 6 items, 0 invalid, 1 with an unknown kind, 1 with a broken link'])
  })

  test('is empty when the repository has no planning folder', async () => {
    const { result, invalid } = await call(makeRepo({}), 'list')
    expect(result).toEqual({ tasks: [], invalid: [] })
    expect(invalid).toEqual([])
  })
})

describe('the epic comes from the folder', () => {
  test('a task file moved into another epic folder is listed under that epic, with its links unchanged', async () => {
    const repo = copyFixture('basic')
    const before = await call(repo, 'list')
    const from = join(repo, EPICS, 'ep-002-bb-beta/tasks/ts-0003-cc-third.md')
    renameSync(from, join(repo, EPICS, 'ep-001-aa-alpha/tasks/ts-0003-cc-third.md'))

    const after = await call(repo, 'list')
    const stored = (links: Array<{ type: string }>) => links.filter((link) => link.type !== 'part-of')
    expect(linksOf(before.result, 'ts-0003-cc')[0]).toEqual({ type: 'part-of', to: 'ep-002-bb' })
    expect(linksOf(after.result, 'ts-0003-cc')[0]).toEqual({ type: 'part-of', to: 'ep-001-aa' })
    expect(stored(linksOf(after.result, 'ts-0003-cc'))).toEqual(stored(linksOf(before.result, 'ts-0003-cc')))
    expect(after.invalid).toEqual([])
  })

  test('a part-of key or link in frontmatter is a task.invalid, so the two can never disagree', async () => {
    const { invalid } = await call(fixture('hazards'), 'list')
    expect(invalid).toContainEqual(expect.objectContaining({ path: expect.stringContaining('ts-0108-aa-part-of.md'), reason: expect.stringContaining('part-of') }))
    const repo = makeRepo({ 'ep-001-aa-x/tasks/ts-0001-aa-x.md': frontmatter('ts-0001-aa').replace('links: []', 'links:\n  - { type: part-of, to: ep-009-zz }') })
    const linked = await call(repo, 'list')
    expect(linked.invalid).toEqual([expect.objectContaining({ reason: expect.stringContaining('part-of') })])
  })

  test('a folder directly in epics/ that does not start with an epic id is a task.invalid, and none of its files is read', async () => {
    const repo = makeRepo({ 'misc/tasks/ts-0001-aa-x.md': frontmatter('ts-0001-aa') })
    const { result, invalid } = await call(repo, 'list')
    expect(result?.tasks).toEqual([])
    expect(invalid).toEqual([{ path: `${EPICS}/misc`, checkout: null, reason: expect.stringContaining('epic id') }])
  })
})

describe('only the named paths are read', () => {
  test('spikes/, proposals/, questions/, a nested folder and the READMEs are present and ignored', async () => {
    const { result, invalid } = await call(fixture('basic'), 'list')
    expect(idsOf(result)).not.toContain('ts-0091-aa')
    expect(idsOf(result)).not.toContain('ts-0090-aa')
    expect((result?.tasks as Array<{ path: string }>).filter((task) => /spikes|proposals|questions|README|sub\//.test(task.path))).toEqual([])
    expect(invalid).toEqual([])
  })
})

describe('items that are not valid', () => {
  const invalidOf = async () => (await call(fixture('hazards'), 'list')).invalid
  const reasonFor = (invalid: Array<Record<string, any>>, file: string): string => invalid.find((entry) => entry.path.endsWith(file))?.reason

  test('the valid item beside the hazards is still listed', async () => {
    const { result } = await call(fixture('hazards'), 'list')
    expect(idsOf(result)).toContain('ts-0101-aa')
  })

  test('two files with one id: the first stays, the second is a task.invalid that names the first', async () => {
    const { result, invalid } = await call(fixture('hazards'), 'list')
    expect(result?.tasks.filter((task: any) => task.id === 'ts-0102-aa')).toHaveLength(1)
    expect(taskOf(result, 'ts-0102-aa').title).toBe('Duplicate id, first')
    expect(reasonFor(invalid, 'ts-0102-aa-two.md')).toContain('already used by')
    expect(reasonFor(invalid, 'ts-0102-aa-two.md')).toContain('ts-0102-aa-one.md')
  })

  test('an id that does not match the pattern is a task.invalid that says so', async () => {
    const reason = reasonFor(await invalidOf(), 'ts-12-aa-short-number.md')
    expect(reason).toContain('does not match ^[a-z]{2}-[0-9]{3,}-[a-z]{2}$')
  })

  test('an unquoted hash as the id is a task.invalid', async () => {
    expect(reasonFor(await invalidOf(), 'ts-0103-aa-hash-id.md')).toContain('id must be a string, not a number')
  })

  test('an unquoted hash in a field is a task.invalid, and the same hash quoted stays a string', async () => {
    const { result, invalid } = await call(fixture('hazards'), 'list')
    expect(reasonFor(invalid, 'ts-0104-aa-hash-field.md')).toContain('preregistered')
    expect(idsOf(result)).not.toContain('ts-0104-aa')
    expect(idsOf(result)).toContain('ts-0105-aa')
    const got = await call(fixture('hazards'), 'get', { id: 'ts-0105-aa' })
    expect(got.result?.task.fields).toEqual({ preregistered: '1e10' })
  })

  test('a duplicate key is a task.invalid', async () => {
    expect(reasonFor(await invalidOf(), 'ts-0106-aa-duplicate-key.md')).toMatch(/not valid YAML.*unique/i)
  })

  test('an alias bomb is a task.invalid, and the provider keeps answering', async () => {
    const { result, invalid } = await call(fixture('hazards'), 'list')
    expect(reasonFor(invalid, 'ts-0107-aa-alias-bomb.md')).toMatch(/alias/i)
    expect(idsOf(result)).toContain('ts-0101-aa')
  })

  test('a status that is not one of the five categories is a task.invalid that lists them', async () => {
    expect(reasonFor(await invalidOf(), 'ts-0109-aa-bad-status.md')).toContain('ready, active, review, done, dropped')
  })

  test('a missing closing fence, a ---js fence and a file with no frontmatter are each a task.invalid', async () => {
    const invalid = await invalidOf()
    expect(reasonFor(invalid, 'ts-0110-aa-unclosed.md')).toContain('closing line')
    expect(reasonFor(invalid, 'ts-0111-aa-js-fence.md')).toContain('no frontmatter')
    expect(reasonFor(invalid, 'ts-0112-aa-plain.md')).toContain('no frontmatter')
  })

  test('every invalid item names its path from the repository root, and checkout is null', async () => {
    const invalid = await invalidOf()
    expect(invalid.length).toBe(11)
    for (const entry of invalid) {
      expect(entry.path).toMatch(new RegExp(`^${EPICS}/ep-001-aa-hazards/tasks/.*\\.md$`))
      expect(entry.checkout).toBeNull()
    }
  })

  test('get answers no task, and no error, for an invalid item and for an id nobody has', async () => {
    for (const id of ['ts-0104-aa', 'ts-9999-zz']) {
      const reply = await call(fixture('hazards'), 'get', { id })
      expect(reply).toMatchObject({ result: {}, error: undefined })
      expect(reply.result).not.toHaveProperty('task')
    }
  })
})

describe('the root', () => {
  test('list and get fail with a clear error until initialize has run', async () => {
    for (const [method, params] of [['list', {}], ['get', { id: 'ts-0001-aa' }]] as const) {
      const reply = await callUninitialized(method, params)
      expect(reply.result).toBeUndefined()
      expect(reply.error).toMatchObject({ code: -32603, message: expect.stringContaining('send initialize') })
    }
  })

  test('initialize refuses a missing or relative root', async () => {
    for (const params of [{ config: {} }, { config: {}, root: 'basic' }, { config: {}, root: 7 }]) {
      const reply = await callUninitialized('initialize', params)
      expect(reply.error).toMatchObject({ code: -32603, message: expect.stringContaining('absolute path') })
    }
  })

  test('the shared root is read from root, not from the working directory', async () => {
    const { result } = await call(fixture('basic'), 'list')
    expect(process.cwd()).not.toBe(fixture('basic'))
    expect(result?.tasks).toHaveLength(6)
  })

  test('a second initialize moves the provider to the new root', async () => {
    const client = connect()
    await client.request('initialize', { config: {}, root: fixture('basic') })
    expect((await client.request('list')).result?.tasks).toHaveLength(6)
    await client.request('initialize', { config: {}, root: makeRepo({}) })
    expect((await client.request('list')).result?.tasks).toEqual([])
    await client.close()
  })
})

describe('invalid items are in the list reply', () => {
  test('list returns them beside the tasks, each as { path, checkout, reason }, and get adds nothing', async () => {
    const { result } = await call(fixture('hazards'), 'list')
    expect(Object.keys(result ?? {}).sort()).toEqual(['invalid', 'tasks'])
    expect(result?.invalid).toHaveLength(11)
    expect(result?.invalid[0]).toEqual({ path: expect.any(String), checkout: null, reason: expect.any(String) })
    const got = await call(fixture('hazards'), 'get', { id: 'ts-0101-aa' })
    expect(Object.keys(got.result ?? {})).toEqual(['task'])
  })

  test('a tree with nothing invalid has an empty invalid list', async () => {
    const { result } = await call(fixture('basic'), 'list')
    expect(result?.invalid).toEqual([])
  })
})

describe('size caps', () => {
  const taskPath = 'ep-001-aa-x/tasks/ts-0001-aa-x.md'

  test('a frontmatter block over its cap is a task.invalid, beside a long body, and is not parsed', async () => {
    const block = `# ${'x'.repeat(100)}\n`.repeat(Math.ceil(FRONTMATTER_CAP / 100) + 1)
    const repo = makeRepo({ [taskPath]: `${frontmatter('ts-0001-aa', block)}${'a long body\n'.repeat(1000)}` })
    const { result, invalid } = await call(repo, 'list')
    expect(result?.tasks).toEqual([])
    expect(invalid).toEqual([expect.objectContaining({ reason: expect.stringMatching(/frontmatter block.*over the 65536-byte cap/) })])
  })

  test('a frontmatter block of 64 KiB is over the cap even when its closing line is far beyond it', async () => {
    const repo = makeRepo({ [taskPath]: `---\nid: ts-0001-aa\n${'# comment\n'.repeat(20000)}---\n` })
    const { invalid } = await call(repo, 'list')
    expect(invalid).toEqual([expect.objectContaining({ reason: expect.stringContaining('over the 65536-byte cap') })])
  })

  test('a frontmatter block just under its cap is valid', async () => {
    const padding = '#'.repeat(FRONTMATTER_CAP - frontmatter('ts-0001-aa').length - 10)
    const repo = makeRepo({ [taskPath]: `${frontmatter('ts-0001-aa', `${padding}\n`)}` })
    const { result, invalid } = await call(repo, 'list')
    expect(invalid).toEqual([])
    expect(idsOf(result)).toEqual(['ts-0001-aa'])
  })

  test('a long body under the body cap is valid, and get returns all of it', async () => {
    const body = 'x'.repeat(BODY_CAP - 10)
    const repo = makeRepo({ [taskPath]: `${frontmatter('ts-0001-aa')}${body}` })
    const { result, invalid } = await call(repo, 'list')
    expect(invalid).toEqual([])
    expect(idsOf(result)).toEqual(['ts-0001-aa'])
    const got = await call(repo, 'get', { id: 'ts-0001-aa' })
    expect(got.result?.task.body).toBe(`\n${body}`)
  })

  test('a body over the body cap is a task.invalid, and a body that is not YAML-safe never matters', async () => {
    const body = 'key: [unclosed\n&a *a\n'.repeat(Math.ceil(BODY_CAP / 20) + 1)
    const repo = makeRepo({ [taskPath]: `${frontmatter('ts-0001-aa')}${body}`, 'ep-001-aa-x/tasks/ts-0002-aa-y.md': `${frontmatter('ts-0002-aa')}key: [unclosed\n&a *a\n` })
    const { result, invalid } = await call(repo, 'list')
    expect(invalid).toEqual([expect.objectContaining({ path: expect.stringContaining('ts-0001-aa-x.md'), reason: expect.stringMatching(/body is \d+ bytes, over the 262144-byte cap/) })])
    expect(idsOf(result)).toEqual(['ts-0002-aa'])
  })

  test('a file that is not UTF-8 is a task.invalid', async () => {
    const repo = makeRepo({ [taskPath]: `${frontmatter('ts-0001-aa')}�`.replace('�', '') })
    const { invalid } = await call(repo, 'list')
    expect(invalid).toEqual([])
  })

  test('an epic.md is read like any item, and a task file in the same folder lists beside it', async () => {
    const repo = makeRepo({ 'ep-001-aa-x/epic.md': EPIC_FILE, [taskPath]: frontmatter('ts-0001-aa') })
    const { result } = await call(repo, 'list')
    expect(idsOf(result)).toEqual(['ep-001-aa', 'ts-0001-aa'])
  })
})

describe("Collide's own items", () => {
  const root = join(import.meta.dirname, '..', '..', '..')

  /** Every `epic.md` and every `.md` file directly in a `tasks/` folder under this repository's epics/. */
  const expectedFiles = (): string[] =>
    readdirSync(join(root, '.collide/planning'), { recursive: true, encoding: 'utf8' }).filter((path) => /^epics\/[^/]+\/(epic\.md|tasks\/[^/]+\.md)$/.test(path))

  test('list returns every item under epics/ with no task.invalid, and does not read spikes/', async () => {
    const { result, invalid } = await call(root, 'list')
    expect(invalid).toEqual([])
    expect(result?.tasks.length).toBe(expectedFiles().length)
    expect(result?.tasks.length).toBeGreaterThan(40)
    expect((result?.tasks as Array<{ path: string }>).filter((task) => task.path.includes('/spikes/'))).toEqual([])
  })

  test('every one of them is a task or an epic, and every blocked-by link has a target', async () => {
    const { result, logs } = await call(root, 'list')
    expect(new Set((result?.tasks as Array<{ kind: string }>).map((task) => task.kind))).toEqual(new Set(['task', 'epic']))
    expect(logs).toEqual([])
  })
})
