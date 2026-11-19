import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { describe, expect, test } from 'vitest'
import { idsInPaths, nextNumber, SUFFIX_LETTERS, takenIds } from '../src/ids.ts'
import { DEFAULT_LAYOUT } from '../src/layout.ts'
import { applyOp } from '../src/write.ts'
import { call, commitAll, connect, EPIC_FILE, fileVersion, git, tempDir } from './helpers.ts'

const ROOT = '.collide/planning'
const EPICS = `${ROOT}/epics`
const FOLDER = 'ep-001-aa-alpha'
const EPIC = 'ep-001-aa'
const QUESTIONS = `${EPICS}/${FOLDER}/questions`

type Row = Record<string, any>

const doc = (kind: string, id: string, status: string, extra = '', links = 'links: []\n', body = ''): string => {
  const head = `id: ${id}\nkind: ${kind}\nstatus: ${status}\ntitle: "Made-up ${kind}"\nowners: []\n`
  return `---\n${head}${links}${extra}---\n\n${body}`
}
const question = (id: string, status = 'open', extra = '', links?: string): string =>
  doc('question', id, status, extra, links, '## Question\n\n## Decision\n')
const task = (id: string, links?: string): string => doc('task', id, 'ready', '', links, '## Goal\n')
const spike = (id: string, links?: string): string => doc('spike', id, 'planned', '', links, '# Made-up spike\n')
const FEATURES =
  '## Features\n\n### Packs\n- **F1 Trust prompt**: a repo pack asks first.\n- **F2 Pack lock**: pins each pack.\n'
const proposal = (id: string, links?: string, status = 'draft'): string =>
  doc('proposal', id, status, '', links, FEATURES)
const links = (...pairs: Array<[string, string]>): string =>
  `links:\n${pairs.map(([type, to]) => `  - { type: ${type}, to: ${to} }\n`).join('')}`

/** Writes `files` (paths from the repository root) into a fresh temp directory and makes it a git repository. */
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

const EPIC_FILES = { [`${EPICS}/${FOLDER}/epic.md`]: EPIC_FILE }
const read = (root: string, path: string): string => readFileSync(join(root, path), 'utf8')
const idsOf = (tasks: Row[]): string[] => tasks.map((candidate) => candidate.id).sort()
const taskOf = (tasks: Row[], id: string): Row => tasks.find((candidate) => candidate.id === id) as Row

const listed = async (root: string) => {
  const reply = await call(root, 'list')
  return { tasks: reply.result?.tasks as Row[], invalid: reply.invalid, logs: reply.logs }
}

/** The reply to `initialize` for a repository with no items. */
const declared = async (): Promise<Row | undefined> => (await call(repoOf({}, { git: false }), 'initialize')).result

describe('initialize declares the question kind and the planning links', () => {
  test('a question has open, resolved and dropped, and a setStatus that takes open and dropped', async () => {
    const found = ((await declared())?.kinds as Row[]).find((kind) => kind.name === 'question')
    expect(found?.statuses).toEqual([
      { name: 'open', category: 'ready' },
      { name: 'resolved', category: 'done' },
      { name: 'dropped', category: 'dropped' },
    ])
    expect(found?.ops).toEqual([
      {
        name: 'setStatus',
        args: expect.objectContaining({ properties: { status: { enum: ['open', 'dropped'] } }, required: ['status'] }),
      },
    ])
  })

  test('includes and supersedes are outward and block nothing; link and create take them', async () => {
    const init = await declared()
    expect(init?.links).toContainEqual({ name: 'includes', dir: 'out', blocking: false })
    expect(init?.links).toContainEqual({ name: 'supersedes', dir: 'out', blocking: false })
    const blocking = (init?.links as Row[]).filter((candidate) => candidate.blocking)
    expect(blocking.map((candidate) => candidate.name)).toEqual(['blocked-by'])
    const ops = init?.ops as Row[]
    expect(ops.find((op) => op.name === 'link')?.args.properties.type).toEqual({
      enum: ['blocked-by', 'implements', 'informs', 'includes', 'supersedes'],
    })
    expect(ops.find((op) => op.name === 'create')?.args.properties.kind).toEqual({
      enum: ['task', 'epic', 'proposal', 'spike', 'question'],
    })
  })
})

describe('list reads the questions of an epic folder', () => {
  test('each of the three statuses lists as a question with part-of, its category, needs and resolved_by', async () => {
    const root = repoOf({
      ...EPIC_FILES,
      [`${QUESTIONS}/qu-0001-aa-open.md`]: question('qu-0001-aa', 'open', 'needs: human\n'),
      [`${QUESTIONS}/qu-0002-bb-resolved.md`]: question(
        'qu-0002-bb',
        'resolved',
        'needs: evidence\nresolved_by: human:2026-11-18\n',
      ),
      [`${QUESTIONS}/qu-0003-cc-dropped.md`]: question('qu-0003-cc', 'dropped', 'needs: either\n'),
      [`${QUESTIONS}/qu-0004-dd-by-item.md`]: question('qu-0004-dd', 'resolved', 'resolved_by: ts-0001-aa\n'),
    })
    const { tasks, invalid, logs } = await listed(root)
    expect(invalid).toEqual([])
    expect(logs).toEqual([])
    expect(idsOf(tasks)).toEqual([EPIC, 'qu-0001-aa', 'qu-0002-bb', 'qu-0003-cc', 'qu-0004-dd'])
    expect(taskOf(tasks, 'qu-0001-aa')).toMatchObject({
      kind: 'question',
      state: { status: 'open', category: 'ready' },
      links: [{ type: 'part-of', to: EPIC }],
      fields: { needs: 'human' },
      path: `${QUESTIONS}/qu-0001-aa-open.md`,
    })
    expect(taskOf(tasks, 'qu-0002-bb')).toMatchObject({
      state: { status: 'resolved', category: 'done' },
      fields: { needs: 'evidence', resolved_by: 'human:2026-11-18' },
    })
    expect(taskOf(tasks, 'qu-0003-cc')).toMatchObject({
      state: { status: 'dropped', category: 'dropped' },
      fields: { needs: 'either' },
    })
    expect(taskOf(tasks, 'qu-0004-dd').fields).toEqual({ resolved_by: 'ts-0001-aa' })
  })

  test('a question with neither needs nor resolved_by has no fields', async () => {
    const root = repoOf({ ...EPIC_FILES, [`${QUESTIONS}/qu-0001-aa-plain.md`]: question('qu-0001-aa') })
    const found = taskOf((await listed(root)).tasks, 'qu-0001-aa')
    expect(found.fields).toBeUndefined()
  })

  test.each([
    ['resolved with no resolved_by', 'resolved', '', 'resolved_by'],
    ['open with a resolved_by', 'open', 'resolved_by: human:2026-11-18\n', 'resolved_by'],
    ['dropped with a resolved_by', 'dropped', 'resolved_by: human:2026-11-18\n', 'resolved_by'],
    ['a needs that is not human, evidence or either', 'open', 'needs: maybe\n', 'must be one of human, evidence'],
    ['a needs that is a list', 'open', 'needs: [human]\n', 'needs'],
    ['a resolved_by that is not a person-and-date or an item id', 'resolved', 'resolved_by: someone\n', 'resolved_by'],
    ['a resolved_by with a date that is not a date', 'resolved', 'resolved_by: human:soon\n', 'resolved_by'],
    ['a category word as the status', 'ready', '', 'open, resolved, dropped for a question'],
  ])('%s is a task.invalid that names the field', async (_name, status, extra, reason) => {
    const path = `${QUESTIONS}/qu-0009-zz-bad.md`
    const root = repoOf({
      ...EPIC_FILES,
      [path]: question('qu-0009-zz', status, extra),
      [`${QUESTIONS}/qu-0001-aa-fine.md`]: question('qu-0001-aa'),
    })
    const { tasks, invalid } = await listed(root)
    expect(invalid).toEqual([{ path, checkout: null, reason: expect.stringContaining(reason) }])
    expect(idsOf(tasks)).toEqual([EPIC, 'qu-0001-aa'])
  })

  test('a YAML hazard in a question file (alias bomb, duplicate key) is invalid, the others still list', async () => {
    const bomb = [
      'a: &a ["x", "x", "x", "x", "x", "x", "x", "x", "x"]',
      'b: &b [*a, *a, *a, *a, *a, *a, *a, *a, *a]',
      'c: &c [*b, *b, *b, *b, *b, *b, *b, *b, *b]',
      'd: &d [*c, *c, *c, *c, *c, *c, *c, *c, *c]',
      'e: &e [*d, *d, *d, *d, *d, *d, *d, *d, *d]',
      'f: &f [*e, *e, *e, *e, *e, *e, *e, *e, *e]',
      'g: [*f, *f, *f, *f, *f, *f, *f, *f, *f]',
    ].join('\n')
    const root = repoOf({
      ...EPIC_FILES,
      [`${QUESTIONS}/qu-0001-aa-fine.md`]: question('qu-0001-aa'),
      [`${QUESTIONS}/qu-0002-bb-bomb.md`]: question('qu-0002-bb', 'open', `${bomb}\n`),
      [`${QUESTIONS}/qu-0003-cc-twice.md`]: question('qu-0003-cc', 'open', 'needs: human\nneeds: either\n'),
    })
    const { tasks, invalid } = await listed(root)
    expect(idsOf(tasks)).toEqual([EPIC, 'qu-0001-aa'])
    expect(invalid.map((entry) => entry.path).sort()).toEqual([
      `${QUESTIONS}/qu-0002-bb-bomb.md`,
      `${QUESTIONS}/qu-0003-cc-twice.md`,
    ])
    expect(invalid.find((entry) => entry.path.includes('bomb'))?.reason).toMatch(/alias/i)
    expect(invalid.find((entry) => entry.path.includes('twice'))?.reason).toMatch(/unique/i)
  })

  test('a question outside an epic folder, in its root, nested or in a spike folder is not read', async () => {
    const root = repoOf({
      ...EPIC_FILES,
      [`${QUESTIONS}/qu-0001-aa-read.md`]: question('qu-0001-aa'),
      [`${ROOT}/questions/qu-0002-bb-loose.md`]: question('qu-0002-bb'),
      [`${EPICS}/${FOLDER}/qu-0003-cc-in-epic.md`]: question('qu-0003-cc'),
      [`${QUESTIONS}/sub/qu-0004-dd-nested.md`]: question('qu-0004-dd'),
      [`${ROOT}/spikes/sp-0001-aa-demo/record.md`]: spike('sp-0001-aa'),
      [`${ROOT}/spikes/sp-0001-aa-demo/questions/qu-0005-ee-in-spike.md`]: question('qu-0005-ee'),
    })
    const { tasks, invalid } = await listed(root)
    expect(idsOf(tasks)).toEqual([EPIC, 'qu-0001-aa', 'sp-0001-aa'])
    expect(invalid).toEqual([])
  })
})

describe('the ids that questions and spikes hold', () => {
  test('idsInPaths names a file directly in questions/ and a spike folder, and not a nested file', () => {
    const ids = idsInPaths([
      `${EPICS}/ep-002-va-skeleton/epic.md`,
      `${EPICS}/ep-002-va-skeleton/questions/qu-0007-bx-why.md`,
      `${EPICS}/ep-002-va-skeleton/questions/sub/qu-0092-aa-nested.md`,
      `${EPICS}/ep-002-va-skeleton/questions/notes.txt`,
      `${EPICS}/ep-002-va-skeleton/qu-0093-aa-loose.md`,
      `${ROOT}/questions/qu-0094-aa-outside.md`,
      `${ROOT}/spikes/sp-0005-rt-demo/record.md`,
      `${ROOT}/spikes/sp-0005-rt-demo/questions/qu-0095-aa-inside.md`,
    ])
    expect([...ids].sort()).toEqual(['ep-002-va', 'qu-0007-bx', 'sp-0005-rt'])
  })

  test('a question and a spike on another branch, and one in the private root, are taken', async () => {
    const root = repoOf({ ...EPIC_FILES })
    git(root, 'checkout', '--quiet', '-b', 'elsewhere')
    mkdirSync(join(root, QUESTIONS), { recursive: true })
    writeFileSync(join(root, QUESTIONS, 'qu-0007-zz-there.md'), question('qu-0007-zz'))
    mkdirSync(join(root, ROOT, 'spikes/sp-0009-yy-there'), { recursive: true })
    writeFileSync(join(root, ROOT, 'spikes/sp-0009-yy-there/record.md'), spike('sp-0009-yy'))
    commitAll(root, 'elsewhere')
    git(root, 'checkout', '--quiet', 'main')
    const priv = tempDir()
    mkdirSync(join(priv, 'epics', EPIC, 'questions'), { recursive: true })
    writeFileSync(join(priv, 'epics', EPIC, 'questions', 'qu-0012-xx-mine.md'), question('qu-0012-xx'))
    const taken = await takenIds(root, DEFAULT_LAYOUT, priv)
    expect([...taken].filter((id) => /^(qu|sp)-/.test(id)).sort()).toEqual(['qu-0007-zz', 'qu-0012-xx', 'sp-0009-yy'])
    expect(nextNumber(taken, 'qu')).toBe(13)
    expect(nextNumber(taken, 'sp')).toBe(10)
  })
})

describe('create of a question', () => {
  const repoWithEpic = (): string => repoOf({ ...EPIC_FILES })
  const create = (root: string, args: Record<string, unknown>) => call(root, 'apply', { op: 'create', args })

  test('writes <id>-<slug>.md into the epic\'s questions/ folder, open, with its two parts, and it loads', async () => {
    const root = repoWithEpic()
    const title = 'Which store holds the ledger?'
    const { result } = await create(root, { kind: 'question', epic: EPIC, title, seed: 1 })
    const path = `${QUESTIONS}/qu-0001-ra-which-store-holds-the-ledger.md`
    expect(result).toMatchObject({
      outcome: 'ok',
      files: [path],
      task: {
        id: 'qu-0001-ra',
        kind: 'question',
        path,
        state: { status: 'open', category: 'ready' },
        links: [{ type: 'part-of', to: EPIC }],
      },
      items: [{ key: 'plan:qu-0001-ra', from: null, to: { status: 'open', category: 'ready' } }],
    })
    expect(read(root, path)).toBe(
      '---\nid: qu-0001-ra\nkind: question\nstatus: open\ntitle: "Which store holds the ledger?"\n' +
        'owners: []\nlinks: []\n---\n\n## Question\n\n## Decision\n',
    )
    expect(git(root, 'status', '--porcelain')).toBe(`?? ${QUESTIONS}/\n`)
    const after = await listed(root)
    expect(after.invalid).toEqual([])
    expect(idsOf(after.tasks)).toContain('qu-0001-ra')
  })

  test('numbers a question on its own, past the highest question in any ref or the working tree', async () => {
    const root = repoOf({ ...EPIC_FILES, [`${QUESTIONS}/qu-0007-aa-seven.md`]: question('qu-0007-aa') })
    const made = await create(root, { kind: 'question', epic: EPIC, title: 'After seven', seed: 1 })
    expect(made.result?.task.id).toBe('qu-0008-ra')
    const first = await create(root, { epic: EPIC, title: 'A task', seed: 1 })
    expect(first.result?.task.id).toBe('ts-0001-ra')
  })

  test.each([
    [{ kind: 'question', title: 'No epic' }, 'a question needs epic'],
    [{ kind: 'question', epic: 'ep-009-zz', title: 'Unknown epic' }, 'there is no epic ep-009-zz'],
    [{ kind: 'question', epic: 'nonsense', title: 'Bad epic id' }, 'a question needs epic'],
  ])('refuses %j and writes nothing', async (args, message) => {
    const root = repoWithEpic()
    const { result } = await create(root, args)
    expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining(message) })
    expect(git(root, 'status', '--porcelain')).toBe('')
  })

  test('with root private goes under the pack data folder, in a folder named for its epic', async () => {
    const root = repoWithEpic()
    const data = tempDir()
    const client = connect({ packData: data })
    try {
      await client.request('initialize', { config: {}, root })
      const args = { kind: 'question', epic: EPIC, title: 'Mine', seed: 1, root: 'private' }
      const made = (await client.request('apply', { op: 'create', args })).result
      expect(made).toMatchObject({ outcome: 'ok', files: [], task: { id: 'qu-0001-ra', fields: { root: 'private' } } })
      expect(made?.task.path).toBe(`private:epics/${EPIC}/questions/qu-0001-ra-mine.md`)
      expect(existsSync(join(data, 'planning', 'epics', EPIC, 'questions', 'qu-0001-ra-mine.md'))).toBe(true)
      expect(git(root, 'status', '--porcelain')).toBe('')
    } finally {
      await client.close()
    }
  })
})

describe('a taken id is redrawn, for a question and for a spike', () => {
  /** A generator that returns the given letters' draws in order: each letter is the middle of its slot. */
  const rigged = (...suffixes: string[]) => {
    const letters = suffixes.join('').split('')
    return () => () => (SUFFIX_LETTERS.indexOf(letters.shift() ?? 'a') + 0.5) / SUFFIX_LETTERS.length
  }
  const cases = [
    { kind: 'question', prefix: 'qu', args: { kind: 'question', epic: EPIC, title: 'Collide' } },
    { kind: 'spike', prefix: 'sp', args: { kind: 'spike', title: 'Collide' } },
  ]

  test.each(cases)('a $kind: a taken id is drawn first, and the draw repeats', async ({ prefix, args }) => {
    const root = repoOf({ ...EPIC_FILES })
    let looks = 0
    const result = await applyOp(root, { op: 'create', args }, {}, {
      generator: rigged('ab', 'ab', 'cd'),
      // The first look finds number 1 free; a ref that arrives after it holds the id the first draw gives.
      look: async (path, layout, privateDir) => {
        looks++
        const taken = await takenIds(path, layout, privateDir)
        if (looks > 1) taken.add(`${prefix}-0001-ab`)
        return taken
      },
    })
    expect(result).toMatchObject({ outcome: 'ok', task: { id: `${prefix}-0001-cd` } })
    expect(looks).toBe(4)
  })

  test.each(cases)('a $kind: a number with every suffix taken stops with an error', async ({ prefix, args }) => {
    const root = repoOf({ ...EPIC_FILES })
    const letters = SUFFIX_LETTERS.split('')
    const everySuffix = letters.flatMap((a) => letters.map((b) => `${prefix}-0001-${a}${b}`))
    let looks = 0
    const look = async () => new Set(looks++ === 0 ? [] : everySuffix)
    const op = { op: 'create', args: { ...args, seed: 1 } }
    await expect(applyOp(root, op, {}, { look })).rejects.toThrow(`no free suffix for ${prefix}-0001`)
    expect(git(root, 'status', '--porcelain')).toBe('')
  })
})

describe('two branches that each create ten items across the five kinds', () => {
  const KINDS = ['task', 'proposal', 'question', 'spike', 'epic']

  /** Makes two of each kind in `root`, with seeds from `seedBase`, and returns the ids. */
  async function makeTen(root: string, side: string, seedBase: number): Promise<string[]> {
    const made: string[] = []
    for (let round = 0; round < 2; round++) {
      for (const [index, kind] of KINDS.entries()) {
        const solo = kind === 'epic' || kind === 'spike'
        const title = `A ${kind} ${round} on ${side}`
        const args = { kind, ...(solo ? {} : { epic: EPIC }), title, seed: seedBase + round * 5 + index }
        const { result } = await call(root, 'apply', { op: 'create', args })
        expect(result?.outcome, JSON.stringify(result)).toBe('ok')
        made.push(result?.task.id as string)
      }
    }
    return made
  }

  test('merge with no two files sharing an id and no task.invalid, and the next numbers count both', async () => {
    // Two clones that have not fetched from each other, so neither can see the other's items.
    const remote = tempDir()
    git(remote, 'init', '--bare', '--quiet')
    const first = tempDir()
    git(first, 'init', '--quiet')
    git(first, 'remote', 'add', 'origin', remote)
    mkdirSync(join(first, EPICS, FOLDER), { recursive: true })
    writeFileSync(join(first, EPICS, FOLDER, 'epic.md'), EPIC_FILE)
    commitAll(first, 'main')
    git(first, 'push', '--quiet', 'origin', 'HEAD:main')
    const second = tempDir()
    git(second, 'clone', '--quiet', '--branch', 'main', remote, '.')

    git(first, 'checkout', '--quiet', '-b', 'branch-a')
    const a = await makeTen(first, 'a', 100)
    commitAll(first, 'a')
    git(first, 'push', '--quiet', 'origin', 'branch-a')
    git(second, 'checkout', '--quiet', '-b', 'branch-b')
    const b = await makeTen(second, 'b', 200)
    commitAll(second, 'b')
    git(second, 'fetch', '--quiet', 'origin')
    git(second, 'merge', '--quiet', '--no-edit', 'origin/branch-a')

    // Each branch drew the same numbers, so the merge holds twenty distinct ids only because the suffixes differ.
    expect(a.map((id) => id.slice(0, 7))).toEqual(b.map((id) => id.slice(0, 7)))
    expect(a.filter((id) => b.includes(id))).toEqual([])
    expect(a).toHaveLength(10)
    const { tasks, invalid, logs } = await listed(second)
    expect(invalid).toEqual([])
    expect(logs).toEqual([])
    expect(idsOf(tasks)).toEqual([EPIC, ...a, ...b].sort())
    expect(new Set(idsOf(tasks)).size).toBe(21)
    const kinds = tasks.map((candidate) => candidate.kind).sort()
    expect(kinds.filter((kind) => kind === 'question')).toHaveLength(4)
    expect(kinds.filter((kind) => kind === 'spike')).toHaveLength(4)

    // The next create of each kind counts both branches.
    const next = async (kind: string): Promise<string> => {
      const solo = kind === 'epic' || kind === 'spike'
      const args = { kind, ...(solo ? {} : { epic: EPIC }), title: `After the merge ${kind}`, seed: 1 }
      return (await call(second, 'apply', { op: 'create', args })).result?.task.id as string
    }
    expect(await next('question')).toMatch(/^qu-0003-/)
    expect(await next('spike')).toMatch(/^sp-0003-/)
    expect(await next('task')).toMatch(/^ts-0003-/)
    expect(await next('proposal')).toMatch(/^pr-0003-/)
    expect(await next('epic')).toMatch(/^ep-004-/)
  })
})

describe('setStatus on a question', () => {
  const QUESTION = 'qu-0001-aa'
  const FILE = `${QUESTIONS}/${QUESTION}-why.md`
  const repoWith = (status = 'open', extra = ''): string =>
    repoOf({
      ...EPIC_FILES,
      [FILE]: question(QUESTION, status, extra),
      [`${EPICS}/${FOLDER}/tasks/ts-0001-aa-work.md`]: task('ts-0001-aa'),
      [`${EPICS}/${FOLDER}/proposals/pr-0001-aa-plan.md`]: proposal('pr-0001-aa'),
    })
  const set = (root: string, id: string, file: string, args: Record<string, unknown>) =>
    call(root, 'apply', { op: 'setStatus', id, version: fileVersion(join(root, file)), args })

  test('moves an open question to dropped and back to open, changing the status line and nothing else', async () => {
    const root = repoWith()
    const dropped = await set(root, QUESTION, FILE, { status: 'dropped' })
    expect(dropped.result).toMatchObject({
      outcome: 'ok',
      files: [FILE],
      task: { state: { status: 'dropped', category: 'dropped' } },
    })
    expect(read(root, FILE)).toBe(question(QUESTION, 'dropped'))
    const open = await set(root, QUESTION, FILE, { status: 'open' })
    expect(open.result).toMatchObject({ outcome: 'ok', task: { state: { status: 'open', category: 'ready' } } })
    expect(read(root, FILE)).toBe(question(QUESTION, 'open'))
  })

  test('to the status it has changes nothing', async () => {
    const root = repoWith()
    const { result } = await set(root, QUESTION, FILE, { status: 'open' })
    expect(result).toMatchObject({ outcome: 'ok', files: [], items: [] })
    expect(git(root, 'status', '--porcelain')).toBe('')
  })

  test('to resolved is refused, and the message says where a question is resolved', async () => {
    const root = repoWith()
    const { result } = await set(root, QUESTION, FILE, { status: 'resolved' })
    expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('resolve op') })
    expect(git(root, 'status', '--porcelain')).toBe('')
  })

  test.each(['open', 'dropped'])('of a resolved question to %s is refused', async (status) => {
    const root = repoWith('resolved', 'resolved_by: human:2026-11-18\n')
    const { result } = await set(root, QUESTION, FILE, { status })
    expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('resolved_by') })
    expect(git(root, 'status', '--porcelain')).toBe('')
  })

  test('with a category is refused, as for a proposal and a spike', async () => {
    const root = repoWith()
    const { result } = await set(root, QUESTION, FILE, { category: 'dropped' })
    expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('whose statuses are its own') })
  })

  test('open is no status of a task or a proposal', async () => {
    const root = repoWith()
    const onTask = await set(root, 'ts-0001-aa', `${EPICS}/${FOLDER}/tasks/ts-0001-aa-work.md`, { status: 'open' })
    expect(onTask.result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('not open') })
    const proposalFile = `${EPICS}/${FOLDER}/proposals/pr-0001-aa-plan.md`
    const onProposal = await set(root, 'pr-0001-aa', proposalFile, { status: 'open' })
    expect(onProposal.result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('not open') })
    expect(git(root, 'status', '--porcelain')).toBe('')
  })

  test('a status no kind has lists the words of each kind, the question\'s too', async () => {
    const root = repoWith()
    const { result } = await set(root, QUESTION, FILE, { status: 'nonsense' })
    expect(result).toMatchObject({
      outcome: 'refused',
      message: expect.stringContaining('open, resolved, dropped for a question'),
    })
  })
})

describe('link includes and supersedes', () => {
  const FILES = {
    proposal: `${EPICS}/${FOLDER}/proposals/pr-0001-aa-plan.md`,
    other: `${EPICS}/${FOLDER}/proposals/pr-0002-bb-old.md`,
    task: `${EPICS}/${FOLDER}/tasks/ts-0001-aa-work.md`,
    question: `${QUESTIONS}/qu-0001-aa-why.md`,
    spike: `${ROOT}/spikes/sp-0001-aa-demo/record.md`,
    older: `${ROOT}/spikes/sp-0002-bb-old/record.md`,
    epic: `${EPICS}/${FOLDER}/epic.md`,
  }
  const BY_ID: Record<string, string> = {
    'pr-0001-aa': FILES.proposal,
    'pr-0002-bb': FILES.other,
    'ts-0001-aa': FILES.task,
    'qu-0001-aa': FILES.question,
    'sp-0001-aa': FILES.spike,
    'sp-0002-bb': FILES.older,
    'ep-001-aa': FILES.epic,
  }
  const world = (): string =>
    repoOf({
      [FILES.epic]: EPIC_FILE,
      [FILES.proposal]: proposal('pr-0001-aa'),
      [FILES.other]: proposal('pr-0002-bb'),
      [FILES.task]: task('ts-0001-aa'),
      [FILES.question]: question('qu-0001-aa'),
      [FILES.spike]: spike('sp-0001-aa'),
      [FILES.older]: spike('sp-0002-bb'),
    })
  const link = (root: string, id: string, args: Record<string, unknown>) =>
    call(root, 'apply', { op: 'link', id, version: fileVersion(join(root, BY_ID[id] as string)), args })
  const linksOf = (result: Row | undefined) => (result?.task.links as Row[]).filter((entry) => entry.type !== 'part-of')

  test.each(['qu-0001-aa', 'ts-0001-aa', 'sp-0001-aa'])('includes from a proposal to %s is stored', async (to) => {
    const root = world()
    const { result } = await link(root, 'pr-0001-aa', { type: 'includes', to })
    expect(result).toMatchObject({ outcome: 'ok', files: [FILES.proposal] })
    expect(linksOf(result)).toEqual([{ type: 'includes', to }])
    expect(read(root, FILES.proposal)).toBe(proposal('pr-0001-aa', `links:\n  - { type: includes, to: ${to} }\n`))
    const after = await listed(root)
    expect(after.invalid).toEqual([])
    expect(taskOf(after.tasks, 'pr-0001-aa').links).toContainEqual({ type: 'includes', to })
  })

  test('includes of the same target twice changes nothing the second time', async () => {
    const root = world()
    await link(root, 'pr-0001-aa', { type: 'includes', to: 'qu-0001-aa' })
    const again = await link(root, 'pr-0001-aa', { type: 'includes', to: 'qu-0001-aa' })
    expect(again.result).toMatchObject({ outcome: 'ok', files: [], items: [] })
  })

  test.each([
    ['ts-0001-aa', 'a task'],
    ['qu-0001-aa', 'a question'],
    ['sp-0001-aa', 'a spike'],
    ['ep-001-aa', 'an epic'],
  ])('includes from %s is refused with the rule', async (id, what) => {
    const root = world()
    const { result } = await link(root, id, { type: 'includes', to: 'ts-0001-aa' })
    expect(result).toMatchObject({
      outcome: 'refused',
      message: expect.stringContaining(`${id} is ${what}; includes links from a proposal`),
    })
    expect(git(root, 'status', '--porcelain')).toBe('')
  })

  test.each([
    [{ type: 'includes', to: 'ts-0099-zz' }, 'there is no item ts-0099-zz to include'],
    [{ type: 'includes', to: 'ep-001-aa' }, 'ep-001-aa is an epic; includes holds'],
    [{ type: 'includes', to: 'pr-0002-bb' }, 'pr-0002-bb is a proposal; includes holds'],
    [{ type: 'includes', to: 'qu-0001-aa#F1' }, 'includes names an item, not a feature'],
    [{ type: 'supersedes', to: 'pr-0099-zz' }, 'there is no item pr-0099-zz to supersede'],
    [{ type: 'supersedes', to: 'pr-0001-aa' }, 'pr-0001-aa cannot supersede itself'],
    [{ type: 'supersedes', to: 'sp-0001-aa' }, 'supersedes links a proposal to a proposal'],
    [{ type: 'supersedes', to: 'pr-0002-bb#F1' }, 'supersedes names an item, not a feature'],
  ])('from a proposal, %j is refused', async (args, message) => {
    const root = world()
    const { result } = await link(root, 'pr-0001-aa', args)
    expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining(message) })
    expect(git(root, 'status', '--porcelain')).toBe('')
  })

  test('supersedes from a proposal to a proposal, and from a spike to a spike, are stored', async () => {
    const root = world()
    const fromProposal = await link(root, 'pr-0001-aa', { type: 'supersedes', to: 'pr-0002-bb' })
    expect(linksOf(fromProposal.result)).toEqual([{ type: 'supersedes', to: 'pr-0002-bb' }])
    const fromSpike = await link(root, 'sp-0001-aa', { type: 'supersedes', to: 'sp-0002-bb' })
    expect(linksOf(fromSpike.result)).toEqual([{ type: 'supersedes', to: 'sp-0002-bb' }])
    expect((await listed(root)).invalid).toEqual([])
  })

  test.each([
    ['ts-0001-aa', 'pr-0002-bb', 'a task'],
    ['qu-0001-aa', 'qu-0001-aa', 'a question'],
    ['ep-001-aa', 'ep-001-aa', 'an epic'],
    ['sp-0001-aa', 'pr-0002-bb', 'a spike'],
  ])('supersedes from %s to %s is refused with the rule', async (id, to, what) => {
    const root = world()
    const { result } = await link(root, id, { type: 'supersedes', to })
    expect(result).toMatchObject({
      outcome: 'refused',
      message: expect.stringContaining(`supersedes links a proposal to a proposal, or a spike to a spike`),
    })
    expect(result?.message).toContain(what)
    expect(git(root, 'status', '--porcelain')).toBe('')
  })

  test('a stored link of either type to an id no file has still lists, and is counted as a broken link', async () => {
    const root = repoOf({
      [FILES.epic]: EPIC_FILE,
      [FILES.proposal]: proposal('pr-0001-aa', links(['includes', 'ts-0099-zz'], ['supersedes', 'pr-0098-zz'])),
    })
    const { tasks, invalid, logs } = await listed(root)
    expect(invalid).toEqual([])
    expect(taskOf(tasks, 'pr-0001-aa').links).toEqual([
      { type: 'part-of', to: EPIC },
      { type: 'includes', to: 'ts-0099-zz' },
      { type: 'supersedes', to: 'pr-0098-zz' },
    ])
    expect(logs.join('\n')).toContain('1 with a broken link')
  })

  test('informs still starts from a question', async () => {
    const root = world()
    const { result } = await link(root, 'qu-0001-aa', { type: 'informs', to: 'sp-0001-aa' })
    expect(linksOf(result)).toEqual([{ type: 'informs', to: 'sp-0001-aa' }])
  })
})

describe('reid over every link type', () => {
  const PROPOSAL_FILE = `${EPICS}/${FOLDER}/proposals/pr-0003-km-plan.md`
  const FILES: Record<string, string> = {
    [`${EPICS}/${FOLDER}/epic.md`]: EPIC_FILE,
    [PROPOSAL_FILE]: proposal(
      'pr-0003-km',
      links(
        ['includes', 'qu-0001-aa'],
        ['includes', 'sp-0001-aa'],
        ['includes', 'sp-0002-bb'],
        ['includes', 'ts-0001-aa'],
      ),
    ),
    [`${EPICS}/${FOLDER}/proposals/pr-0004-cc-next.md`]: proposal('pr-0004-cc', links(['supersedes', 'pr-0003-km'])),
    [`${QUESTIONS}/qu-0001-aa-why.md`]: question('qu-0001-aa'),
    [`${QUESTIONS}/qu-0002-bb-how.md`]: question('qu-0002-bb', 'open', '', links(['informs', 'sp-0001-aa'])),
    [`${EPICS}/${FOLDER}/tasks/ts-0001-aa-work.md`]: task(
      'ts-0001-aa',
      links(['informs', 'qu-0001-aa'], ['implements', 'pr-0003-km#F2']),
    ),
    [`${ROOT}/spikes/sp-0001-aa-demo/record.md`]: spike(
      'sp-0001-aa',
      links(['informs', 'qu-0001-aa'], ['supersedes', 'sp-0002-bb']),
    ),
    [`${ROOT}/spikes/sp-0002-bb-old/record.md`]: spike('sp-0002-bb'),
  }

  const filesOf = (root: string): Record<string, string> => {
    const found: Record<string, string> = {}
    for (const name of readdirSync(join(root, ROOT), { recursive: true, encoding: 'utf8' })) {
      if (name.endsWith('.md') && name !== 'README.md') found[`${ROOT}/${name}`] = read(root, `${ROOT}/${name}`)
    }
    return found
  }
  const commits = (root: string): number => Number(git(root, 'rev-list', '--count', '--all').trim())

  test.each([
    ['qu-0001-aa', ['pr-0003-km:includes:', 'sp-0001-aa:informs:', 'ts-0001-aa:informs:']],
    ['qu-0002-bb', []],
    ['sp-0001-aa', ['pr-0003-km:includes:', 'qu-0002-bb:informs:']],
    ['sp-0002-bb', ['pr-0003-km:includes:', 'sp-0001-aa:supersedes:']],
    ['pr-0003-km', ['pr-0004-cc:supersedes:', 'ts-0001-aa:implements:#F2']],
  ])('of %s rewrites each link that names it, keeps a feature fragment, and commits nothing', async (id, names) => {
    const root = repoOf(FILES)
    const before = filesOf(root)
    const commitsBefore = commits(root)
    const { result } = await call(root, 'apply', { op: 'reid', id, args: { seed: 1 } })
    expect(result?.outcome, JSON.stringify(result)).toBe('ok')
    const newId = result?.task.id as string
    expect(newId).not.toBe(id)
    expect(newId.slice(0, 7)).toBe(id.slice(0, 7))

    // Every file is as it was with the old id swapped for the new one, in its text and in its path.
    const expected = Object.fromEntries(
      Object.entries(before).map(([path, text]) => [path.replaceAll(id, newId), text.replaceAll(id, newId)]),
    )
    expect(filesOf(root)).toEqual(expected)

    const after = await listed(root)
    expect(after.invalid).toEqual([])
    expect(after.logs).toEqual([])
    const naming = after.tasks.flatMap((candidate) =>
      (candidate.links as Row[])
        .filter((entry) => entry.to.startsWith(newId))
        .map((entry) => `${candidate.id}:${entry.type}:${entry.to.slice(newId.length)}`),
    )
    expect(naming.sort()).toEqual([...names].sort())
    expect(commits(root)).toBe(commitsBefore)
  })
})

describe('reid of an epic renames its private folder too', () => {
  const SHARED_TASK = `${EPICS}/${FOLDER}/tasks/ts-0001-aa-shared.md`
  const world = () => {
    const repo = repoOf({ ...EPIC_FILES, [SHARED_TASK]: task('ts-0001-aa', links(['blocked-by', 'qu-0001-aa'])) })
    const data = tempDir()
    const planning = join(data, 'planning')
    const privateFiles = {
      [`epics/${EPIC}/tasks/ts-0002-bb-mine.md`]: task('ts-0002-bb'),
      [`epics/${EPIC}/questions/qu-0001-aa-mine.md`]: question('qu-0001-aa'),
    }
    for (const [path, text] of Object.entries(privateFiles)) {
      mkdirSync(dirname(join(planning, path)), { recursive: true })
      writeFileSync(join(planning, path), text)
    }
    return { repo, data, planning }
  }
  const direct = (w: ReturnType<typeof world>, params: Record<string, unknown>, timeoutMs = 10_000) =>
    applyOp(w.repo, params as never, { timeoutMs }, {}, DEFAULT_LAYOUT, w.planning)

  test('a shared epic: the private folder takes the new id, so the private items keep their epic', async () => {
    const w = world()
    const reply = await direct(w, { op: 'reid', id: EPIC, args: { seed: 1 } })
    expect(reply).toMatchObject({ outcome: 'ok', task: { id: 'ep-001-ra', path: `${EPICS}/ep-001-ra-alpha/epic.md` } })
    expect(existsSync(join(w.planning, 'epics', EPIC))).toBe(false)
    expect(existsSync(join(w.planning, 'epics', 'ep-001-ra', 'tasks', 'ts-0002-bb-mine.md'))).toBe(true)
    expect(existsSync(join(w.planning, 'epics', 'ep-001-ra', 'questions', 'qu-0001-aa-mine.md'))).toBe(true)
    const client = connect({ packData: w.data })
    try {
      await client.request('initialize', { config: {}, root: w.repo })
      const { tasks, invalid } = (await client.request('list')).result as { tasks: Row[]; invalid: Row[] }
      expect(invalid).toEqual([])
      for (const id of ['ts-0002-bb', 'qu-0001-aa']) {
        expect(taskOf(tasks, id).links).toContainEqual({ type: 'part-of', to: 'ep-001-ra' })
      }
    } finally {
      await client.close()
    }
    // Nothing of the private root is in the reply's files, and only the shared folder is a change of the repository.
    expect((reply as Row).files.filter((file: string) => file.startsWith('private:'))).toEqual([])
    expect(git(w.repo, 'status', '--porcelain', '--', ROOT).split('\n').filter(Boolean).sort()).toEqual([
      ` D ${EPICS}/${FOLDER}/epic.md`,
      ` D ${SHARED_TASK}`,
      `?? ${EPICS}/ep-001-ra-alpha/`,
    ])
  })

  test('a private epic: its folder is renamed whole, with the items in it', async () => {
    const w = world()
    const args = { kind: 'epic', title: 'Mine', seed: 1, root: 'private' }
    const made = (await direct(w, { op: 'create', args })) as Row
    const epic = made.task.id as string
    await direct(w, { op: 'create', args: { kind: 'question', epic, title: 'Inside', seed: 1, root: 'private' } })
    const reply = await direct(w, { op: 'reid', id: epic, args: { seed: 2 } })
    const renamed = (reply as Row).task.id as string
    expect(renamed).not.toBe(epic)
    expect(existsSync(join(w.planning, 'epics', epic))).toBe(false)
    expect(readdirSync(join(w.planning, 'epics', renamed, 'questions'))).toHaveLength(1)
  })

  test('is held by the private root\'s lock, as the shared root\'s', async () => {
    const w = world()
    writeFileSync(join(w.planning, '.write-lock'), `${process.pid}\n`)
    await expect(direct(w, { op: 'reid', id: EPIC, args: { seed: 1 } }, 300)).rejects.toThrow(
      `waiting for the lock file ${join(w.planning, '.write-lock')}`,
    )
    expect(existsSync(join(w.planning, 'epics', EPIC))).toBe(true)
  })
})
