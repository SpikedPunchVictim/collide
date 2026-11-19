import { execFileSync } from 'node:child_process'
import { existsSync, readdirSync, readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, test } from 'vitest'
import { call, copyFixture, EPIC_FILE, fileVersion, makeRepo } from './helpers.ts'

const PLANNING = '.collide/planning'
const TASKS = 'ep-001-aa/tasks'
const pathOf = (root: string, name: string): string => join(root, PLANNING, 'epics', TASKS, name)

const item = (id: string, lines: string): string => `---\nid: ${id}\nkind: task\nstatus: ready\ntitle: "Made-up item"\n${lines}---\n\n## Goal\n\nMade-up content.\n`

const ONE = 'ts-0001-aa-one.md'
const TWO = 'ts-0002-bb-two.md'

/** A repository with two items; `one` is the one the tests write to. */
const repo = (one: string, two = item('ts-0002-bb', 'owners: []\nlinks: []\n')): string =>
  makeRepo({ 'ep-001-aa/epic.md': EPIC_FILE, [`${TASKS}/${ONE}`]: one, [`${TASKS}/${TWO}`]: two })

const standard = (): string => repo(item('ts-0001-aa', 'owners: []\nlinks: []\n'))

/** Sends `apply` for `id` at the file's current version, unless `version` is given. */
async function apply(root: string, op: string, args: Record<string, unknown>, extra: Record<string, unknown> = {}, id = 'ts-0001-aa', file = ONE) {
  const version = Object.hasOwn(extra, 'version') ? extra.version : fileVersion(pathOf(root, file))
  return call(root, 'apply', { op, id, args, ...extra, version })
}

const read = (root: string, file = ONE): string => readFileSync(pathOf(root, file), 'utf8')

const git = (cwd: string, ...args: string[]): string =>
  execFileSync('git', ['-c', 'user.name=Test', '-c', 'user.email=test@example.invalid', '-c', 'commit.gpgsign=false', ...args], {
    cwd,
    encoding: 'utf8',
    env: { ...process.env, GIT_CONFIG_GLOBAL: '/dev/null', GIT_CONFIG_NOSYSTEM: '1' },
  })

describe('initialize declares the write ops', () => {
  test('the ops, each with an argument schema, and no op that moves to one fixed category', async () => {
    const { result } = await call(standard(), 'initialize', { config: {} })
    const ops = result?.ops as Array<Record<string, any>>
    expect(ops.map((op) => op.name)).toEqual([
      'setStatus', 'link', 'claim', 'unclaim', 'create', 'reid', 'renameSection',
    ])
    expect(ops.every((op) => op.movesTo === undefined && op.args.type === 'object')).toBe(true)
    const five = ['ready', 'active', 'review', 'done', 'dropped']
    expect(ops[0]?.args).toMatchObject({
      properties: { status: { enum: five }, category: { enum: five } },
      oneOf: [{ required: ['status'] }, { required: ['category'] }],
      additionalProperties: false,
    })
    expect(ops[1]?.args).toMatchObject({
      required: ['to'],
      properties: { type: { enum: ['blocked-by', 'implements', 'informs', 'includes', 'supersedes'] } },
    })
    expect(ops[2]?.args).toMatchObject({ required: ['claimant'] })
  })
})

describe('setStatus', () => {
  test('changes exactly the status line, as git diff shows, and leaves no lock or temp file', async () => {
    const root = copyFixture('basic')
    git(root, 'init', '--quiet')
    git(root, 'add', '.')
    git(root, 'commit', '--quiet', '-m', 'fixture')
    const file = '.collide/planning/epics/ep-001-aa-alpha/tasks/ts-0002-bb-second.md'
    const before = readFileSync(join(root, file), 'utf8')
    const status = /^status: .*$/m.exec(before)?.[0]

    const { result } = await call(root, 'apply', { op: 'setStatus', id: 'ts-0002-bb', args: { category: 'active' }, version: fileVersion(join(root, file)) })

    expect(result?.outcome).toBe('ok')
    expect(git(root, 'diff', '--numstat')).toBe(`1\t1\t${file}\n`)
    const diff = git(root, 'diff', '-U0').split('\n').filter((line) => /^[-+]/.test(line) && !/^(---|\+\+\+)/.test(line))
    expect(diff).toEqual([`-${status}`, '+status: active'])
    expect(git(root, 'status', '--porcelain')).toBe(` M ${file}\n`)
  })

  test('answers ok with the new task, its version (the hash on disk), the file touched and the item\'s change', async () => {
    const root = standard()
    const { result } = await apply(root, 'setStatus', { category: 'review' })
    const version = fileVersion(pathOf(root, ONE))
    expect(result).toMatchObject({
      outcome: 'ok',
      version,
      files: [`${PLANNING}/epics/${TASKS}/${ONE}`],
      task: { id: 'ts-0001-aa', key: 'plan:ts-0001-aa', version, state: { category: 'review', status: 'review' }, body: expect.stringContaining('Made-up content') },
      items: [{ key: 'plan:ts-0001-aa', version, from: { category: 'ready', status: 'ready' }, to: { category: 'review', status: 'review' } }],
    })
    expect((await call(root, 'get', { id: 'ts-0001-aa' })).result?.task.version).toBe(version)
  })

  test('keeps a quote, a trailing comment and CRLF line endings, and changes only that line', async () => {
    const text = item('ts-0001-aa', 'owners: []\nlinks: []\n').replace('status: ready', 'status:  "ready"  # to do').replaceAll('\n', '\r\n')
    const root = repo(text)
    await apply(root, 'setStatus', { category: 'done' })
    expect(read(root)).toBe(text.replace('status:  "ready"  # to do', 'status:  "done"  # to do'))
  })

  test('to the status it already has writes nothing and says so: ok, no files, no items', async () => {
    const root = standard()
    const before = fileVersion(pathOf(root, ONE))
    const { result } = await apply(root, 'setStatus', { category: 'ready' })
    expect(result).toMatchObject({ outcome: 'ok', version: before, files: [], items: [] })
    expect(fileVersion(pathOf(root, ONE))).toBe(before)
  })

  test.each([['status'], ['category']])('takes the value as %s', async (name) => {
    const root = standard()
    const before = read(root)
    const { result } = await apply(root, 'setStatus', { [name]: 'active' })
    expect(result).toMatchObject({ outcome: 'ok', task: { state: { category: 'active', status: 'active' } } })
    expect(read(root)).toBe(before.replace('status: ready', 'status: active'))
  })

  test('refuses both status and category, and neither, and writes nothing', async () => {
    const root = standard()
    const before = read(root)
    expect((await apply(root, 'setStatus', { status: 'active', category: 'active' })).result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('either status or category, not both') })
    expect((await apply(root, 'setStatus', {})).result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('give status or category') })
    expect(read(root)).toBe(before)
  })

  test('refuses a value that is not one of the five, and an argument it does not take', async () => {
    const root = standard()
    const before = read(root)
    expect((await apply(root, 'setStatus', { category: 'doing' })).result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('category must be one of') })
    expect((await apply(root, 'setStatus', { status: 'doing' })).result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('status must be one of') })
    expect((await apply(root, 'setStatus', { category: 'done', note: 'x' })).result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('unknown argument "note"') })
    expect(read(root)).toBe(before)
  })

  test('refuses a status line it cannot edit in place, and writes nothing', async () => {
    const text = item('ts-0001-aa', 'owners: []\nlinks: []\n').replace('status: ready', 'status: >-\n  ready')
    const root = repo(text)
    const { result } = await apply(root, 'setStatus', { category: 'done' })
    expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('not in a form the provider edits') })
    expect(read(root)).toBe(text)
  })

  test('works on an epic file the same way', async () => {
    const root = standard()
    const epic = join(root, PLANNING, 'epics/ep-001-aa/epic.md')
    const { result } = await call(root, 'apply', { op: 'setStatus', id: 'ep-001-aa', args: { category: 'done' }, version: fileVersion(epic) })
    expect(result?.outcome).toBe('ok')
    expect(readFileSync(epic, 'utf8')).toBe(EPIC_FILE.replace('status: active', 'status: done'))
  })
})

describe('version', () => {
  test('a stale version answers conflict with the current task and writes nothing', async () => {
    const root = standard()
    const stale = fileVersion(pathOf(root, ONE))
    await apply(root, 'setStatus', { category: 'active' })
    const after = read(root)
    const { result } = await apply(root, 'setStatus', { category: 'done' }, { version: stale })
    expect(result).toMatchObject({ outcome: 'conflict', current: { id: 'ts-0001-aa', version: fileVersion(pathOf(root, ONE)), state: { category: 'active' } } })
    expect(read(root)).toBe(after)
  })

  test.each([['missing', undefined], ['null', null], ['empty', '']])('a %s version is refused, not written over whatever is there', async (_name, version) => {
    const root = standard()
    const before = read(root)
    const { result } = await call(root, 'apply', { op: 'setStatus', id: 'ts-0001-aa', args: { category: 'done' }, version })
    expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('needs the version you last read') })
    expect(read(root)).toBe(before)
  })

  test('an edit made outside the provider since the caller read is a conflict too', async () => {
    const root = standard()
    const stale = fileVersion(pathOf(root, ONE))
    writeFileSync(pathOf(root, ONE), `${read(root)}\nAdded by hand.\n`)
    const { result } = await apply(root, 'setStatus', { category: 'done' }, { version: stale })
    expect(result?.outcome).toBe('conflict')
  })
})

describe('refusals before any write', () => {
  test.each([
    ['an unknown op', { op: 'rename', id: 'ts-0001-aa', args: {} }, 'unknown op "rename"'],
    ['an unknown id', { op: 'setStatus', id: 'ts-9999-zz', args: { category: 'done' } }, 'there is no item ts-9999-zz'],
    ['no id', { op: 'setStatus', args: { category: 'done' } }, 'needs the id'],
    ['a named checkout', { op: 'setStatus', id: 'ts-0001-aa', args: { category: 'done' }, checkout: '/elsewhere' }, 'checkout'],
  ])('%s is refused', async (_name, params, message) => {
    const root = standard()
    const before = read(root)
    const { result } = await call(root, 'apply', { ...params, version: fileVersion(pathOf(root, ONE)) })
    expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining(message) })
    expect(read(root)).toBe(before)
  })

  test('before initialize it is an error reply', async () => {
    const { callUninitialized } = await import('./helpers.ts')
    expect((await callUninitialized('apply', { op: 'setStatus', id: 'ts-0001-aa', args: { category: 'done' }, version: 'x' })).error).toMatchObject({ code: -32603 })
  })
})

describe('claim and unclaim', () => {
  test('claim puts the claimant on an empty owners list, changing only that line', async () => {
    const root = standard()
    const before = read(root)
    const { result } = await apply(root, 'claim', { claimant: 'claude-sonnet' })
    expect(result).toMatchObject({ outcome: 'ok', task: { owners: ['claude-sonnet'] }, files: [expect.stringContaining(ONE)] })
    expect(read(root)).toBe(before.replace('owners: []', 'owners: [claude-sonnet]'))
  })

  test('claim by another claimant on an item that shows an owner is refused, and writes nothing', async () => {
    const root = repo(item('ts-0001-aa', 'owners: [claude-sonnet]\nlinks: []\n'))
    const before = read(root)
    const { result } = await apply(root, 'claim', { claimant: 'someone-else' })
    expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('already shows an owner (claude-sonnet)') })
    expect(read(root)).toBe(before)
  })

  test('claim by the claimant that already owns the item is ok and writes nothing', async () => {
    const root = repo(item('ts-0001-aa', 'owners: [claude-sonnet]\nlinks: []\n'))
    const before = read(root)
    const { result } = await apply(root, 'claim', { claimant: 'claude-sonnet' })
    expect(result).toMatchObject({ outcome: 'ok', files: [], items: [], task: { owners: ['claude-sonnet'] } })
    expect(read(root)).toBe(before)
  })

  test.each([
    ['a number', '123', '["123"]'],
    ['a word YAML reads as a flag', 'true', '["true"]'],
    ['a name with a space', 'Ada L', '["Ada L"]'],
    ['a name with a colon and a hash', 'a: b #c', '["a: b #c"]'],
    ['an address', 'ada@example.invalid', '["ada@example.invalid"]'],
  ])('claim writes %s so that it reads back as the same text', async (_name, claimant, written) => {
    const root = standard()
    const { result } = await apply(root, 'claim', { claimant })
    expect(result).toMatchObject({ outcome: 'ok', task: { owners: [claimant] } })
    expect(read(root)).toContain(`owners: [${written.slice(1, -1)}]`)
  })

  test.each([[''], [' padded '], ['two\nlines'], ['x'.repeat(201)]])('claim refuses the claimant %j', async (claimant) => {
    const root = standard()
    const before = read(root)
    expect((await apply(root, 'claim', { claimant })).result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('claimant must be') })
    expect(read(root)).toBe(before)
  })

  test('unclaim clears a flow list, keeping the rest of the line', async () => {
    const root = repo(item('ts-0001-aa', 'owners: [a, b]  # who\nlinks: []\n'))
    const { result } = await apply(root, 'unclaim', {})
    expect(result).toMatchObject({ outcome: 'ok', task: { owners: [] } })
    expect(read(root)).toContain('\nowners: []  # who\nlinks: []\n')
  })

  test('unclaim clears a block list and removes its entry lines', async () => {
    const text = item('ts-0001-aa', 'owners:\n  - a\n  - b\nlinks: []\n')
    const root = repo(text)
    const { result } = await apply(root, 'unclaim', {})
    expect(result).toMatchObject({ outcome: 'ok', task: { owners: [] } })
    expect(read(root)).toBe(text.replace('owners:\n  - a\n  - b\n', 'owners: []\n'))
  })

  test('unclaim on an item with no owner writes nothing', async () => {
    const root = standard()
    const { result } = await apply(root, 'unclaim', {})
    expect(result).toMatchObject({ outcome: 'ok', files: [], items: [] })
  })

  test('a claim, then an unclaim, then a claim by someone else', async () => {
    const root = standard()
    expect((await apply(root, 'claim', { claimant: 'a' })).result?.outcome).toBe('ok')
    expect((await apply(root, 'claim', { claimant: 'b' })).result?.outcome).toBe('refused')
    expect((await apply(root, 'unclaim', {})).result?.outcome).toBe('ok')
    expect((await apply(root, 'claim', { claimant: 'b' })).result).toMatchObject({ outcome: 'ok', task: { owners: ['b'] } })
  })

  test('claim on an item with no owners line is refused rather than inserting one', async () => {
    const root = repo(item('ts-0001-aa', 'links: []\n'))
    const before = read(root)
    expect((await apply(root, 'claim', { claimant: 'a' })).result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('no "owners:" line') })
    expect(read(root)).toBe(before)
  })
})

describe('link', () => {
  const links = (result: Record<string, any> | undefined) => (result?.task.links as Array<{ type: string; to: string }>).filter((link) => link.type !== 'part-of')

  test('adds a blocked-by entry under an existing list, indented like the others, changing only the links lines', async () => {
    const text = item('ts-0001-aa', 'owners: []\nlinks:\n  - { type: blocked-by, to: ts-0002-bb }\n')
    const root = repo(text, item('ts-0002-bb', 'owners: []\nlinks: []\n'))
    makeRepoItem(root, 'ts-0003-cc')
    const { result } = await apply(root, 'link', { to: 'ts-0003-cc' })
    expect(links(result)).toEqual([
      { type: 'blocked-by', to: 'ts-0002-bb' },
      { type: 'blocked-by', to: 'ts-0003-cc' },
    ])
    expect(read(root)).toBe(text.replace('to: ts-0002-bb }\n', 'to: ts-0002-bb }\n  - { type: blocked-by, to: ts-0003-cc }\n'))
  })

  test('turns an empty list into a block with the one entry', async () => {
    const root = standard()
    const before = read(root)
    const { result } = await apply(root, 'link', { to: 'ts-0002-bb' })
    expect(links(result)).toEqual([{ type: 'blocked-by', to: 'ts-0002-bb' }])
    expect(read(root)).toBe(before.replace('links: []\n', 'links:\n  - { type: blocked-by, to: ts-0002-bb }\n'))
  })

  test('adds after the last entry when the list header has trailing space and a blank line follows the entries', async () => {
    const text = item('ts-0001-aa', 'owners: []\nlinks: \n    - { type: blocked-by, to: ts-0002-bb }\n\n')
    const root = repo(text)
    makeRepoItem(root, 'ts-0003-cc')
    await apply(root, 'link', { to: 'ts-0003-cc' })
    expect(read(root)).toBe(text.replace('ts-0002-bb }\n\n', 'ts-0002-bb }\n    - { type: blocked-by, to: ts-0003-cc }\n\n'))
  })

  test('the new link shows in list, and the epic is still reported from the folder', async () => {
    const root = standard()
    await apply(root, 'link', { to: 'ts-0002-bb' })
    const task = (await call(root, 'list')).result?.tasks.find((candidate: { id: string }) => candidate.id === 'ts-0001-aa')
    expect(task.links).toEqual([
      { type: 'part-of', to: 'ep-001-aa' },
      { type: 'blocked-by', to: 'ts-0002-bb' },
    ])
  })

  test('a link that is already there is ok and writes nothing', async () => {
    const text = item('ts-0001-aa', 'owners: []\nlinks:\n  - { type: blocked-by, to: ts-0002-bb }\n')
    const root = repo(text)
    const { result } = await apply(root, 'link', { to: 'ts-0002-bb' })
    expect(result).toMatchObject({ outcome: 'ok', files: [], items: [] })
    expect(read(root)).toBe(text)
  })

  test('a link to the item itself is refused', async () => {
    const root = standard()
    expect((await apply(root, 'link', { to: 'ts-0001-aa' })).result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('cannot be blocked by itself') })
  })

  test('a link to an id no item has is refused', async () => {
    const root = standard()
    const before = read(root)
    expect((await apply(root, 'link', { to: 'ts-0099-zz' })).result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('no item ts-0099-zz') })
    expect(read(root)).toBe(before)
  })

  test('a type other than blocked-by, and a target that is not an id, are refused', async () => {
    const root = standard()
    expect((await apply(root, 'link', { to: 'ts-0002-bb', type: 'part-of' })).result?.outcome).toBe('refused')
    expect((await apply(root, 'link', { to: 'two' })).result?.outcome).toBe('refused')
  })

  test.each([
    ['a one-line list with entries', 'owners: []\nlinks: [{ type: blocked-by, to: ts-0002-bb }]\n'],
    ['an entry over several lines', 'owners: []\nlinks:\n  - type: blocked-by\n    to: ts-0002-bb\n'],
    ['no links line', 'owners: []\n'],
  ])('%s is refused, and the file is left as it was', async (_name, lines) => {
    const text = item('ts-0001-aa', lines)
    const root = repo(text)
    makeRepoItem(root, 'ts-0003-cc')
    const { result } = await apply(root, 'link', { to: 'ts-0003-cc' })
    expect(result?.outcome).toBe('refused')
    expect(read(root)).toBe(text)
  })
})

describe('what a write leaves behind', () => {
  test('no lock file, claim file or temp file, and the scan reads a leftover temp file as nothing', async () => {
    const root = standard()
    await apply(root, 'setStatus', { category: 'active' })
    writeFileSync(join(root, PLANNING, 'epics', TASKS, '.ts-0001-aa-one.md.1-ab.collide-tmp'), '---\nid: not valid')
    const { invalid, result } = await call(root, 'list')
    expect(invalid).toEqual([])
    expect(result?.tasks).toHaveLength(3)
    expect(readdirSync(join(root, PLANNING)).sort()).toEqual(['epics'])
  })

  test('the next write on a file removes the temp file a killed writer left for it', async () => {
    const root = standard()
    const leftover = join(root, PLANNING, 'epics', TASKS, `.${ONE}.1-ab.collide-tmp`)
    writeFileSync(leftover, 'half')
    await apply(root, 'setStatus', { category: 'active' })
    expect(existsSync(leftover)).toBe(false)
  })
})

/** Adds an item `id` (with no links) to the repository's first epic. */
function makeRepoItem(root: string, id: string): void {
  writeFileSync(join(root, PLANNING, 'epics', TASKS, `${id}-extra.md`), item(id, 'owners: []\nlinks: []\n'))
}
