import {
  chmodSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  renameSync,
  symlinkSync,
  writeFileSync,
} from 'node:fs'
import { dirname, join } from 'node:path'
import { describe, expect, test } from 'vitest'
import { idsInPaths, takenIds } from '../src/ids.ts'
import { BODY_CAP } from '../src/read-file.ts'
import { commitAll, connect, copyFixture, EPIC_FILE, fileVersion, frontmatter, git, tempDir } from './helpers.ts'

const ROOT = '.collide/planning'
const SPIKES = `${ROOT}/spikes`
const SPIKE = 'sp-0001-aa'
const SPIKE_DIR = `${SPIKES}/${SPIKE}-demo`
const EPIC = 'ep-001-aa'

type Client = ReturnType<typeof connect>
type Row = Record<string, any>

/** A spike record: its frontmatter, with `extra` lines before the closing fence, and a one-heading body. */
const record = (id: string, status = 'planned', extra = ''): string => {
  const fields = `id: ${id}\nkind: spike\nstatus: ${status}\ntitle: "Made-up spike"\nowners: []\nlinks: []\n${extra}`
  return `---\n${fields}---\n\n# Made-up spike\n`
}

/** Writes `files` (paths from the repository root) into a fresh temp directory, and makes it a git repository. */
function repoOf(files: Record<string, string>, { git: makeGit = true }: { git?: boolean } = {}): string {
  const dir = tempDir()
  for (const [path, text] of Object.entries({ 'README.md': 'fixture\n', ...files })) {
    mkdirSync(dirname(join(dir, path)), { recursive: true })
    writeFileSync(join(dir, path), text)
  }
  if (makeGit) {
    git(dir, 'init', '--quiet')
    commitAll(dir, 'fixture')
  }
  return dir
}

const EPIC_FILES = { [`${ROOT}/epics/ep-001-aa-alpha/epic.md`]: EPIC_FILE }

/** Runs `body` with a client that `initialize` has given `root` (and `layout`, and a pack data folder). */
async function withClient<T>(
  root: string,
  body: (client: Client) => Promise<T>,
  options: { layout?: unknown; packData?: string | null } = {},
): Promise<T> {
  const client = connect({ packData: options.packData ?? null })
  try {
    const config = options.layout === undefined ? {} : { layout: options.layout }
    const init = await client.request('initialize', { config, root })
    if (init.error !== undefined) throw new Error(`initialize failed: ${JSON.stringify(init.error)}`)
    return await body(client)
  } finally {
    await client.close()
  }
}

const list = async (client: Client) =>
  (await client.request('list')).result as { tasks: Row[]; invalid: Row[] }
const apply = async (client: Client, params: Record<string, unknown>) => (await client.request('apply', params)).result
const create = (client: Client, args: Record<string, unknown>) =>
  apply(client, { op: 'create', args: { seed: 1, ...args } })
const versionOf = async (client: Client, id: string): Promise<string> =>
  (await client.request('get', { id })).result?.task.version
/** One op on the item `id`, written against the version that is on disk. */
const step = async (client: Client, id: string, op: string, args: Record<string, unknown>) =>
  apply(client, { op, id, version: await versionOf(client, id), args })

const listOf = (root: string, options: Parameters<typeof withClient>[2] = {}) =>
  withClient(root, (client) => list(client), options)
const idsOf = (listing: { tasks: Row[] }): string[] => listing.tasks.map((task) => task.id).sort()
const read = (root: string, path: string): string => readFileSync(join(root, path), 'utf8')

/** The reply to `initialize` for a repository with no items. */
async function declared(): Promise<Row | undefined> {
  const client = connect()
  try {
    return (await client.request('initialize', { config: {}, root: repoOf({}, { git: false }) })).result
  } finally {
    await client.close()
  }
}

describe('initialize declares the spike kind', () => {
  test('with its five statuses and a setStatus of its own, as a proposal has', async () => {
    const spike = ((await declared())?.kinds as Row[]).find((kind) => kind.name === 'spike')
    expect(spike?.statuses).toEqual([
      { name: 'planned', category: 'ready' },
      { name: 'running', category: 'active' },
      { name: 'results', category: 'review' },
      { name: 'concluded', category: 'done' },
      { name: 'dropped', category: 'dropped' },
    ])
    expect(spike?.ops).toEqual([
      {
        name: 'setStatus',
        args: expect.objectContaining({
          properties: { status: { enum: ['planned', 'running', 'results', 'concluded', 'dropped'] } },
          required: ['status'],
        }),
      },
    ])
  })

  test('the link type informs, outward and not blocking, and link and create take it', async () => {
    const init = await declared()
    expect(init?.links).toContainEqual({ name: 'informs', dir: 'out', blocking: false, inverse: 'informed by' })
    expect((init?.links as Row[]).filter((link) => link.blocking).map((link) => link.name)).toEqual(['blocked-by'])
    const ops = init?.ops as Row[]
    expect(ops.find((op) => op.name === 'link')?.args.properties.type).toEqual({
      enum: ['blocked-by', 'implements', 'informs', 'includes', 'supersedes'],
    })
    expect(ops.find((op) => op.name === 'create')?.args.properties.kind).toEqual({
      enum: ['task', 'epic', 'proposal', 'spike', 'question'],
    })
  })
})

describe('list reads each spike record', () => {
  test("over Collide's ten records and one in results, each is a spike with no part-of", async () => {
    const root = copyFixture('collide-spikes')
    const { tasks, invalid } = await listOf(root)
    expect(invalid).toEqual([])
    expect(tasks).toHaveLength(11)
    expect(tasks.every((task) => task.kind === 'spike')).toBe(true)
    expect(tasks.flatMap((task) => task.links)).toEqual([])
    const one = tasks.find((task) => task.id === 'sp-0006-kq')
    expect(one).toMatchObject({
      key: 'plan:sp-0006-kq',
      aliases: ['S7'],
      title: 'S7: `AgentAdapter` against ACP (paper check)',
      path: `${SPIKES}/sp-0006-kq-agentadapter-acp/record.md`,
      state: { status: 'concluded', category: 'done' },
    })
    const results = tasks.find((task) => task.id === 'sp-0011-cx')
    expect(results?.state).toEqual({ status: 'results', category: 'review' })
    expect(tasks.filter((task) => task.state.status === 'concluded')).toHaveLength(10)
  })

  test('get returns the body of the record', async () => {
    const root = copyFixture('collide-spikes')
    const got = await withClient(root, (client) => client.request('get', { id: 'sp-0011-cx' }))
    expect(got.result?.task.body).toContain('# S12: results are in')
  })

  test('a folder with node_modules, work, out and instances lists one item, and a locked subfolder is ok', async () => {
    const files: Record<string, string> = {
      [`${SPIKE_DIR}/record.md`]: record(SPIKE),
      [`${SPIKE_DIR}/instances/opus-a/record.md`]: record('sp-0002-bb'),
      [`${SPIKE_DIR}/instances/opus-a/tasks/ts-0001-aa-x.md`]: frontmatter('ts-0001-aa'),
      [`${SPIKE_DIR}/node_modules/dep/package.json`]: '{}\n',
      [`${SPIKE_DIR}/node_modules/dep/record.md`]: record('sp-0003-cc'),
      [`${SPIKE_DIR}/work/record.md`]: record('sp-0004-dd'),
      [`${SPIKE_DIR}/work/notes.md`]: '# notes\n',
      [`${SPIKE_DIR}/out/record.md`]: record('sp-0005-ee'),
      [`${SPIKE_DIR}/locked/inside.md`]: 'x\n',
    }
    const root = repoOf(files, { git: false })
    const locked = join(root, SPIKE_DIR, 'locked')
    chmodSync(locked, 0o000)
    try {
      const { tasks, invalid } = await listOf(root)
      expect(invalid).toEqual([])
      expect(idsOf({ tasks })).toEqual([SPIKE])
      expect(tasks[0]?.path).toBe(`${SPIKE_DIR}/record.md`)
    } finally {
      chmodSync(locked, 0o755)
    }
  })

  test('a record.md in a folder inside the spikes folder is not read, and the folder is a task.invalid', async () => {
    const root = repoOf(
      {
        [`${SPIKES}/group/${SPIKE}-demo/record.md`]: record(SPIKE),
        [`${SPIKES}/${SPIKE}-demo/record.md`]: record('sp-0002-bb'),
      },
      { git: false },
    )
    const { tasks, invalid } = await listOf(root)
    expect(idsOf({ tasks })).toEqual(['sp-0002-bb'])
    expect(invalid).toEqual([
      {
        path: `${SPIKES}/group`,
        checkout: null,
        reason: expect.stringContaining('must start with a spike id such as sp-0005-rt'),
      },
    ])
  })

  test.each(['misc', 'Sp-0001-aa', 'sp-1-aa', 'sp-0001-a', 'sp-0001-aab', 'ts-0001-aa-x', 'spike-0001-aa'])(
    'a folder named %s is a task.invalid that names the rule, and its record is not read',
    async (name) => {
      const root = repoOf({ [`${SPIKES}/${name}/record.md`]: record(SPIKE) }, { git: false })
      const { tasks, invalid } = await listOf(root)
      expect(tasks).toEqual([])
      expect(invalid).toEqual([
        { path: `${SPIKES}/${name}`, checkout: null, reason: expect.stringContaining('must start with a spike id') },
      ])
    },
  )

  test('no record.md, a file in the spikes folder, and a record.md that is a link: all are no items', async () => {
    const root = repoOf(
      {
        [`${SPIKES}/README.md`]: 'not a spike\n',
        [`${SPIKES}/${SPIKE}-bare/work/x.md`]: 'x\n',
        [`${SPIKES}/sp-0002-bb-linked/real.md`]: record('sp-0002-bb'),
      },
      { git: false },
    )
    symlinkSync('real.md', join(root, SPIKES, 'sp-0002-bb-linked', 'record.md'))
    const { tasks, invalid } = await listOf(root)
    expect(tasks).toEqual([])
    expect(invalid).toEqual([])
  })

  test('a spike folder that cannot be opened is a task.invalid for that folder, and the scan goes on', async () => {
    const root = repoOf(
      {
        [`${SPIKES}/${SPIKE}-shut/record.md`]: record(SPIKE),
        [`${SPIKES}/sp-0002-bb-open/record.md`]: record('sp-0002-bb'),
      },
      { git: false },
    )
    const shut = join(root, SPIKES, `${SPIKE}-shut`)
    chmodSync(shut, 0o000)
    try {
      const { tasks, invalid } = await listOf(root)
      expect(idsOf({ tasks })).toEqual(['sp-0002-bb'])
      expect(invalid).toEqual([
        { path: `${SPIKES}/${SPIKE}-shut/record.md`, checkout: null, reason: expect.stringContaining('EACCES') },
      ])
    } finally {
      chmodSync(shut, 0o755)
    }
  })

  test('a status that is not one of the five is a task.invalid that lists them', async () => {
    const root = repoOf({ [`${SPIKE_DIR}/record.md`]: record(SPIKE, 'open') }, { git: false })
    const { tasks, invalid } = await listOf(root)
    expect(tasks).toEqual([])
    expect(invalid).toEqual([
      {
        path: `${SPIKE_DIR}/record.md`,
        checkout: null,
        reason: 'status must be one of planned, running, results, concluded, dropped for a spike, not "open"',
      },
    ])
  })

  test('a task status on a spike is a task.invalid too, since a spike has its own five words', async () => {
    const root = repoOf({ [`${SPIKE_DIR}/record.md`]: record(SPIKE, 'ready') }, { git: false })
    const { invalid } = await listOf(root)
    expect(invalid).toEqual([expect.objectContaining({ reason: expect.stringContaining('planned, running') })])
  })

  const bomb = [
    'a: &a ["x", "x", "x", "x", "x", "x", "x", "x", "x"]',
    'b: &b [*a, *a, *a, *a, *a, *a, *a, *a, *a]',
    'c: &c [*b, *b, *b, *b, *b, *b, *b, *b, *b]',
    'd: &d [*c, *c, *c, *c, *c, *c, *c, *c, *c]',
    'e: &e [*d, *d, *d, *d, *d, *d, *d, *d, *d]',
    'f: &f [*e, *e, *e, *e, *e, *e, *e, *e, *e]',
    'g: &g [*f, *f, *f, *f, *f, *f, *f, *f, *f]',
    'h: &h [*g, *g, *g, *g, *g, *g, *g, *g, *g]',
    'i: [*h, *h, *h, *h, *h, *h, *h, *h, *h]',
    '',
  ].join('\n')

  test.each([
    ['a hash that is a number', record(SPIKE, 'planned', 'preregistered: 1e10\n'), 'quote a value that looks like'],
    ['a duplicate key', record(SPIKE, 'planned', 'status: running\n'), 'not valid YAML'],
    ['an alias bomb', record(SPIKE, 'planned', bomb), 'not valid YAML'],
    ['a record over the body cap', `${record(SPIKE)}${'x'.repeat(BODY_CAP + 1)}`, 'over the'],
    ['a record with no frontmatter', '# no fence\n', 'no frontmatter'],
    ['a part-of link', record(SPIKE).replace('links: []', 'links:\n  - { type: part-of, to: ep-001-aa }'), 'part-of'],
  ])('%s in a record is a task.invalid for the record', async (_name, text, reason) => {
    const files = { [`${SPIKE_DIR}/record.md`]: text, [`${SPIKES}/sp-0002-bb-fine/record.md`]: record('sp-0002-bb') }
    const root = repoOf(files, { git: false })
    const { tasks, invalid } = await listOf(root)
    expect(idsOf({ tasks })).toEqual(['sp-0002-bb'])
    expect(invalid).toEqual([
      { path: `${SPIKE_DIR}/record.md`, checkout: null, reason: expect.stringContaining(reason) },
    ])
  })

  test('an unquoted no is the text no, as YAML 1.2 reads it, and never a flag', async () => {
    const text = record(SPIKE, 'planned', 'alias: no\nreviewed: yes\n')
    const root = repoOf({ [`${SPIKE_DIR}/record.md`]: text }, { git: false })
    const { tasks, invalid } = await listOf(root)
    expect(invalid).toEqual([])
    expect(tasks[0]).toMatchObject({ aliases: ['no'], fields: { reviewed: 'yes' } })
  })

  test('two records with one id: the second is set aside, as for any item', async () => {
    const root = repoOf(
      { [`${SPIKES}/sp-0001-aa-one/record.md`]: record(SPIKE), [`${SPIKES}/sp-0001-aa-two/record.md`]: record(SPIKE) },
      { git: false },
    )
    const { tasks, invalid } = await listOf(root)
    expect(idsOf({ tasks })).toEqual([SPIKE])
    expect(invalid).toEqual([
      expect.objectContaining({
        path: `${SPIKES}/sp-0001-aa-two/record.md`,
        reason: expect.stringContaining('already used'),
      }),
    ])
  })

  test('an informs link is kept as written, and one that names no item is counted as a broken link', async () => {
    const informs = (to: string): string =>
      record(SPIKE).replace('links: []', `links:\n  - { type: informs, to: ${to} }`)
    const root = repoOf({ ...EPIC_FILES, [`${SPIKE_DIR}/record.md`]: informs(EPIC) }, { git: false })
    const client = connect()
    try {
      await client.request('initialize', { config: {}, root })
      const { tasks } = await list(client)
      expect(tasks.find((task) => task.id === SPIKE)?.links).toEqual([{ type: 'informs', to: EPIC }])
      writeFileSync(join(root, SPIKE_DIR, 'record.md'), informs('ep-009-zz'))
      const reply = await client.request('list')
      expect(reply.logs).toEqual(['plan: 2 items, 0 invalid, 0 with an unknown kind, 1 with a broken link'])
    } finally {
      await client.close()
    }
  })
})

describe('create of a spike', () => {
  test('writes spikes/<id>-<slug>/record.md as planned, with one heading, and the item loads', async () => {
    const root = repoOf(EPIC_FILES)
    const { created, listing } = await withClient(root, async (client) => ({
      created: await create(client, { kind: 'spike', title: 'Planning seams: can records merge?' }),
      listing: await list(client),
    }))
    const path = `${SPIKES}/sp-0001-ra-planning-seams-can-records-merge/record.md`
    expect(created).toMatchObject({
      outcome: 'ok',
      files: [path],
      version: fileVersion(join(root, path)),
      task: { id: 'sp-0001-ra', kind: 'spike', path, state: { status: 'planned', category: 'ready' }, links: [] },
      items: [{ key: 'plan:sp-0001-ra', from: null, to: { status: 'planned', category: 'ready' } }],
    })
    expect(read(root, path)).toBe(
      '---\nid: sp-0001-ra\nkind: spike\nstatus: planned\ntitle: "Planning seams: can records merge?"\n' +
        'owners: []\nlinks: []\n---\n\n# Planning seams: can records merge?\n',
    )
    expect(listing.invalid).toEqual([])
    expect(idsOf(listing)).toContain('sp-0001-ra')
    expect(git(root, 'status', '--porcelain')).toBe(`?? ${SPIKES}/\n`)
  })

  test('refuses an epic, because a spike is part of no epic, and a kind that is none of the five', async () => {
    const root = repoOf(EPIC_FILES)
    const replies = await withClient(root, async (client) => [
      await create(client, { kind: 'spike', epic: EPIC, title: 'In an epic' }),
      await create(client, { kind: 'widget', title: 'Not a kind' }),
    ])
    expect(replies[0]).toMatchObject({
      outcome: 'refused',
      message: expect.stringContaining('a spike is part of no epic'),
    })
    expect(replies[1]).toMatchObject({
      outcome: 'refused',
      message: expect.stringContaining('kind must be "task", "epic", "proposal", "question" or "spike"'),
    })
    expect(git(root, 'status', '--porcelain')).toBe('')
  })

  test('its number is free across every ref: another branch, a remote ref and the working tree', async () => {
    const root = repoOf({ ...EPIC_FILES, [`${SPIKES}/sp-0002-bb-here/record.md`]: record('sp-0002-bb') })
    git(root, 'checkout', '--quiet', '-b', 'other')
    mkdirSync(join(root, SPIKES, 'sp-0007-cc-there'), { recursive: true })
    writeFileSync(join(root, SPIKES, 'sp-0007-cc-there', 'record.md'), record('sp-0007-cc'))
    commitAll(root, 'a spike on another branch')
    git(root, 'checkout', '--quiet', 'main')
    git(root, 'checkout', '--quiet', '-b', 'third')
    mkdirSync(join(root, SPIKES, 'sp-0008-ee-fetched'), { recursive: true })
    writeFileSync(join(root, SPIKES, 'sp-0008-ee-fetched', 'record.md'), record('sp-0008-ee'))
    commitAll(root, 'a spike that only a remote has')
    git(root, 'update-ref', 'refs/remotes/origin/third', 'third')
    git(root, 'checkout', '--quiet', 'main')
    git(root, 'branch', '--quiet', '-D', 'third')
    expect(existsSync(join(root, SPIKES, 'sp-0007-cc-there'))).toBe(false)
    mkdirSync(join(root, SPIKES, 'sp-0009-dd-loose'), { recursive: true })
    writeFileSync(join(root, SPIKES, 'sp-0009-dd-loose', 'record.md'), record('sp-0009-dd'))

    expect(await takenIds(root)).toEqual(new Set([EPIC, 'sp-0002-bb', 'sp-0009-dd', 'sp-0007-cc', 'sp-0008-ee']))
    const made = await withClient(root, (client) => create(client, { kind: 'spike', title: 'Next one' }))
    expect(made?.task.id).toMatch(/^sp-0010-[a-z]{2}$/)
  })

  test('two spikes made in a row are numbered one and two', async () => {
    const root = repoOf(EPIC_FILES)
    const ids = await withClient(root, async (client) => [
      (await create(client, { kind: 'spike', title: 'One', seed: 5 }))?.task.id,
      (await create(client, { kind: 'spike', title: 'Two', seed: 5 }))?.task.id,
    ])
    expect(ids[0]).toMatch(/^sp-0001-/)
    expect(ids[1]).toMatch(/^sp-0002-/)
  })

  test('does not read inside the spike folders it finds the ids of', async () => {
    const root = repoOf(EPIC_FILES)
    const folder = join(root, SPIKES, 'sp-0003-cc-deep')
    mkdirSync(join(folder, 'work', 'sp-0050-zz-buried'), { recursive: true })
    writeFileSync(join(folder, 'record.md'), record('sp-0003-cc'))
    chmodSync(join(folder, 'work'), 0o000)
    try {
      const made = await withClient(root, (client) => create(client, { kind: 'spike', title: 'Beside it' }))
      expect(made?.task.id).toMatch(/^sp-0004-/)
    } finally {
      chmodSync(join(folder, 'work'), 0o755)
    }
  })
})

describe('setStatus on a spike', () => {
  const states = ['running', 'results', 'concluded']

  test('moves it through planned, running, results and concluded, one status line at a time', async () => {
    const root = repoOf({ [`${SPIKE_DIR}/record.md`]: record(SPIKE) })
    const replies = await withClient(root, async (client) => {
      const out: Row[] = []
      for (const status of states) out.push((await step(client, SPIKE, 'setStatus', { status })) as Row)
      return out
    })
    expect(replies.map((reply) => reply.task.state)).toEqual([
      { status: 'running', category: 'active' },
      { status: 'results', category: 'review' },
      { status: 'concluded', category: 'done' },
    ])
    expect(replies.map((reply) => reply.files)).toEqual(Array(3).fill([`${SPIKE_DIR}/record.md`]))
    expect(git(root, 'diff', '--numstat')).toBe(`1\t1\t${SPIKE_DIR}/record.md\n`)
    expect(read(root, `${SPIKE_DIR}/record.md`)).toBe(record(SPIKE, 'concluded'))
  })

  test.each(['planned', 'running', 'results', 'concluded', 'dropped'])('dropped is reached from %s', async (from) => {
    const root = repoOf({ [`${SPIKE_DIR}/record.md`]: record(SPIKE, from) })
    const reply = await withClient(root, (client) => step(client, SPIKE, 'setStatus', { status: 'dropped' }))
    expect(reply).toMatchObject({ outcome: 'ok', task: { state: { status: 'dropped', category: 'dropped' } } })
  })

  test.each(['open', 'ready', 'draft'])('%s is refused, and the refusal lists the five', async (status) => {
    const root = repoOf({ [`${SPIKE_DIR}/record.md`]: record(SPIKE) })
    const reply = await withClient(root, (client) => step(client, SPIKE, 'setStatus', { status }))
    expect(reply).toMatchObject({ outcome: 'refused' })
    const five = 'planned, running, results, concluded, dropped'
    expect(reply?.message).toContain(five)
    expect(git(root, 'status', '--porcelain')).toBe('')
  })

  test('a category is refused: a spike has its own statuses, so give status', async () => {
    const root = repoOf({ [`${SPIKE_DIR}/record.md`]: record(SPIKE) })
    const reply = await withClient(root, (client) => step(client, SPIKE, 'setStatus', { category: 'active' }))
    expect(reply).toMatchObject({ outcome: 'refused', message: expect.stringContaining('give status') })
  })

  test('a task still takes the five categories and a proposal its six', async () => {
    const taskFile = `${ROOT}/epics/ep-001-aa-alpha/tasks/ts-0001-aa-x.md`
    const root = repoOf({ ...EPIC_FILES, [taskFile]: frontmatter('ts-0001-aa') })
    const reply = await withClient(root, (client) => step(client, 'ts-0001-aa', 'setStatus', { status: 'planned' }))
    expect(reply).toMatchObject({
      outcome: 'refused',
      message: expect.stringContaining('ready, active, review, done, dropped'),
    })
  })
})

describe('link informs', () => {
  const files = { ...EPIC_FILES, [`${SPIKE_DIR}/record.md`]: record(SPIKE) }

  test('from a spike to an epic writes the one links line, and the link is listed', async () => {
    const root = repoOf(files)
    const { reply, listing } = await withClient(root, async (client) => ({
      reply: await step(client, SPIKE, 'link', { to: EPIC, type: 'informs' }),
      listing: await list(client),
    }))
    expect(reply).toMatchObject({ outcome: 'ok', files: [`${SPIKE_DIR}/record.md`] })
    expect(read(root, `${SPIKE_DIR}/record.md`)).toBe(
      record(SPIKE).replace('links: []', 'links:\n  - { type: informs, to: ep-001-aa }'),
    )
    expect(listing.tasks.find((task) => task.id === SPIKE)?.links).toEqual([{ type: 'informs', to: EPIC }])
    expect(listing.invalid).toEqual([])
  })

  test('twice changes nothing the second time, and it does not block', async () => {
    const root = repoOf(files)
    const second = await withClient(root, async (client) => {
      await step(client, SPIKE, 'link', { to: EPIC, type: 'informs' })
      return step(client, SPIKE, 'link', { to: EPIC, type: 'informs' })
    })
    expect(second).toMatchObject({ outcome: 'ok', files: [], items: [] })
  })

  test.each([
    ['an item that is not there', { to: 'ep-009-zz' }, 'there is no item ep-009-zz'],
    ['itself', { to: SPIKE }, 'cannot inform itself'],
    ['a feature', { to: `${EPIC}#F1` }, 'informs names an item, not a feature'],
  ])('to %s is refused', async (_name, args, message) => {
    const root = repoOf(files)
    const reply = await withClient(root, (client) => step(client, SPIKE, 'link', { ...args, type: 'informs' }))
    expect(reply).toMatchObject({ outcome: 'refused', message: expect.stringContaining(message) })
    expect(git(root, 'status', '--porcelain')).toBe('')
  })
})

describe('reid of a spike', () => {
  test('renames the folder, rewrites the links to it, and never lists inside the folder', async () => {
    const taskFile = `${ROOT}/epics/ep-001-aa-alpha/tasks/ts-0001-aa-x.md`
    const root = repoOf({
      ...EPIC_FILES,
      [taskFile]: frontmatter('ts-0001-aa').replace('links: []', `links:\n  - { type: blocked-by, to: ${SPIKE} }`),
      [`${SPIKE_DIR}/record.md`]: record(SPIKE),
      [`${SPIKE_DIR}/work/notes.md`]: 'notes\n',
    })
    const shut = join(root, SPIKE_DIR, 'work', 'shut')
    mkdirSync(shut)
    chmodSync(shut, 0o000)
    let reply: Row | undefined
    try {
      reply = (await withClient(root, (client) => apply(client, { op: 'reid', id: SPIKE, args: { seed: 3 } }))) as Row
    } finally {
      // The folder has the new name by now, when the op worked.
      for (const folder of readdirSync(join(root, SPIKES))) {
        const locked = join(root, SPIKES, folder, 'work', 'shut')
        if (existsSync(locked)) chmodSync(locked, 0o755)
      }
    }
    expect(reply).toMatchObject({ outcome: 'ok' })
    const newId = reply.task.id as string
    expect(newId).toMatch(/^sp-0001-[a-z]{2}$/)
    expect(newId).not.toBe(SPIKE)
    expect(reply.task.path).toBe(`${SPIKES}/${newId}-demo/record.md`)
    expect([...(reply.files as string[])].sort()).toEqual(
      [taskFile, `${SPIKE_DIR}/record.md`, `${SPIKES}/${newId}-demo/record.md`].sort(),
    )
    expect(existsSync(join(root, SPIKE_DIR))).toBe(false)
    expect(readdirSync(join(root, SPIKES, `${newId}-demo`)).sort()).toEqual(['record.md', 'work'])
    expect(read(root, taskFile)).toContain(`to: ${newId}`)
    expect(read(root, `${SPIKES}/${newId}-demo/record.md`)).toContain(`id: ${newId}`)
  })
})

describe('idsInPaths and takenIds know the spike folder', () => {
  test('a spike folder names its id, whatever is in it, and a file beside the folders names none', () => {
    const ids = idsInPaths([
      `${SPIKES}/sp-0003-xx-demo`,
      `${SPIKES}/sp-0004-yy-demo/record.md`,
      `${SPIKES}/sp-0005-zz-demo/work/node_modules/x.md`,
      `${SPIKES}/README.md`,
      `${SPIKES}/misc/record.md`,
      'docs/spikes/sp-0006-ww-elsewhere',
    ])
    expect([...ids].sort()).toEqual(['sp-0003-xx', 'sp-0004-yy', 'sp-0005-zz'])
  })

  test('only the spikes folders it is given count', () => {
    const paths = ['docs/spikes/sp-0006-ww-moved', `${SPIKES}/sp-0003-xx-old`]
    expect([...idsInPaths(paths, [], ['docs/spikes'])]).toEqual(['sp-0006-ww'])
    expect([...idsInPaths(paths, [], ['docs/spikes', SPIKES])].sort()).toEqual(['sp-0003-xx', 'sp-0006-ww'])
  })

  test('takenIds reads the private root\'s spikes from disk, and a private spike is not in any ref', async () => {
    const root = repoOf({})
    const data = tempDir()
    mkdirSync(join(data, 'planning', 'spikes', 'sp-0008-pp-mine'), { recursive: true })
    expect([...(await takenIds(root, undefined, join(data, 'planning')))]).toEqual(['sp-0008-pp'])
  })
})

describe('layout.paths.spike and previous roots', () => {
  const moved = { paths: { spike: 'docs/decisions' } }

  test('create puts the folder under paths.spike, and list reads it there and in the old default place', async () => {
    const root = repoOf({ ...EPIC_FILES, [`${SPIKES}/sp-0002-bb-old/record.md`]: record('sp-0002-bb') })
    const { made, listing } = await withClient(
      root,
      async (client) => ({
        made: await create(client, { kind: 'spike', title: 'Moved' }),
        listing: await list(client),
      }),
      { layout: moved },
    )
    expect(made?.task.path).toMatch(/^docs\/decisions\/sp-0003-[a-z]{2}-moved\/record\.md$/)
    expect(made?.files).toEqual([made?.task.path])
    expect(idsOf(listing)).toEqual([EPIC, 'sp-0002-bb', made?.task.id].sort())
    expect(listing.invalid).toEqual([])
  })

  test('a spike under paths.spike takes writes when the planning root itself is not there', async () => {
    const root = repoOf({ 'docs/decisions/sp-0001-aa-x/record.md': record(SPIKE) })
    expect(existsSync(join(root, ROOT))).toBe(false)
    const reply = await withClient(root, (client) => step(client, SPIKE, 'setStatus', { status: 'running' }), {
      layout: moved,
    })
    expect(reply).toMatchObject({ outcome: 'ok', files: ['docs/decisions/sp-0001-aa-x/record.md'] })
  })

  test('the number is free across the paths.spike folder, the old place and a previous root', async () => {
    const root = repoOf({
      ...EPIC_FILES,
      'docs/decisions/sp-0004-dd-new/record.md': record('sp-0004-dd'),
      [`${SPIKES}/sp-0006-ff-old/record.md`]: record('sp-0006-ff'),
      'docs/plan/spikes/sp-0008-hh-prev/record.md': record('sp-0008-hh'),
    })
    const layout = { ...moved, root: 'docs/plan-now', previous: ['docs/plan', ROOT] }
    const made = await withClient(root, (client) => create(client, { kind: 'spike', title: 'Next' }), { layout })
    expect(made?.task.id).toMatch(/^sp-0009-/)
    expect(made?.task.path).toMatch(/^docs\/decisions\/sp-0009-/)
  })

  test('a previous root holding a spike lists it and writes in place; new spikes go to the current root', async () => {
    const root = repoOf({ 'docs/plan/spikes/sp-0002-bb-old/record.md': record('sp-0002-bb') })
    const layout = { root: 'docs/plan-now', previous: ['docs/plan'] }
    const out = await withClient(
      root,
      async (client) => ({
        listing: await list(client),
        status: await step(client, 'sp-0002-bb', 'setStatus', { status: 'results' }),
        made: await create(client, { kind: 'spike', title: 'Fresh' }),
      }),
      { layout },
    )
    expect(out.listing.tasks.map((task) => task.path)).toEqual(['docs/plan/spikes/sp-0002-bb-old/record.md'])
    expect(out.status).toMatchObject({ outcome: 'ok', files: ['docs/plan/spikes/sp-0002-bb-old/record.md'] })
    expect(out.made?.task.path).toMatch(/^docs\/plan-now\/spikes\/sp-0003-[a-z]{2}-fresh\/record\.md$/)
  })
})

describe('a spike in the private root', () => {
  test('is made, listed, changed and renamed like any private item, and the repository is untouched', async () => {
    const root = repoOf(EPIC_FILES)
    const data = tempDir()
    const out = await withClient(
      root,
      async (client) => {
        const made = (await create(client, { kind: 'spike', title: 'Private idea', root: 'private' })) as Row
        const id = made.task.id as string
        const moved = (await step(client, id, 'setStatus', { status: 'running' })) as Row
        const linked = (await step(client, id, 'link', { to: EPIC, type: 'informs' })) as Row
        const listing = await list(client)
        const renamed = (await apply(client, { op: 'reid', id, args: { seed: 2 } })) as Row
        return { made, moved, linked, listing, renamed }
      },
      { packData: data },
    )
    expect(out.made).toMatchObject({ outcome: 'ok', files: [], task: { kind: 'spike', fields: { root: 'private' } } })
    expect(out.made.task.path).toMatch(/^private:spikes\/sp-0001-[a-z]{2}-private-idea\/record\.md$/)
    expect(out.moved).toMatchObject({ outcome: 'ok', files: [], task: { state: { status: 'running' } } })
    expect(out.linked).toMatchObject({ outcome: 'ok', files: [] })
    expect(out.listing.tasks.find((task) => task.id === out.made.task.id)).toMatchObject({
      fields: { root: 'private' },
      links: [{ type: 'informs', to: EPIC }],
    })
    expect(out.renamed).toMatchObject({ outcome: 'ok', files: [] })
    expect(out.renamed.task.id).not.toBe(out.made.task.id)
    expect(readdirSync(join(data, 'planning', 'spikes'))).toEqual([`${out.renamed.task.id}-private-idea`])
    expect(git(root, 'status', '--porcelain')).toBe('')
  })

  test('a private record is listed with the shared ones, each with its own path', async () => {
    const root = repoOf({ [`${SPIKE_DIR}/record.md`]: record(SPIKE) })
    const data = tempDir()
    mkdirSync(join(data, 'planning', 'spikes', 'sp-0002-bb-mine'), { recursive: true })
    writeFileSync(join(data, 'planning', 'spikes', 'sp-0002-bb-mine', 'record.md'), record('sp-0002-bb'))
    const { tasks } = await listOf(root, { packData: data })
    expect(tasks.map((task) => task.path).sort()).toEqual([
      `${SPIKE_DIR}/record.md`,
      'private:spikes/sp-0002-bb-mine/record.md',
    ])
  })
})

describe('a spike record moved by hand', () => {
  test('a folder renamed to another id still loads, and the id comes from the record', async () => {
    const root = repoOf({ [`${SPIKE_DIR}/record.md`]: record(SPIKE) }, { git: false })
    renameSync(join(root, SPIKE_DIR), join(root, SPIKES, 'sp-0002-bb-renamed'))
    const { tasks } = await listOf(root)
    expect(idsOf({ tasks })).toEqual([SPIKE])
  })
})
