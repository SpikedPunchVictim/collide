import { createHash } from 'node:crypto'
import { existsSync, mkdirSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { Ajv2020 } from 'ajv/dist/2020.js'
import { describe, expect, test } from 'vitest'
import { REPO, connect, copyTree, keysOf, listAll, start, taskOf, tree } from './helpers.ts'
import { makeRepo, versionOf } from './repo.ts'

// The three kinds that the Orchard pack's plan provider reads besides tasks: ADRs, spikes and questions. The tree is
// `fixtures/orchard-arch/kinds`: 6 ADRs, 4 spikes, 5 questions and one task, all made up.

type Message = Record<string, any>

const ROOT = tree('kinds')
const sha256 = (path: string) => createHash('sha256').update(readFileSync(path)).digest('hex')

const ADR_002 = 'arch/002-2026-01-19-berth-prices.md'
const SPIKE_2B = 'arch/spikes/s2b-2026-02-12-lamp-wiring/PLAN.md'
const QUESTION_Q02 = 'arch/proposals/03-harbor-index/questions/Q-03-02-how-deep-is-the-quay.md'

const KEYS = [
  'plan:ADR-001', 'plan:ADR-002', 'plan:ADR-003', 'plan:ADR-004', 'plan:ADR-005', 'plan:ADR-006',
  'plan:Q-03-01', 'plan:Q-03-02', 'plan:Q-03-03', 'plan:Q-03-04', 'plan:Q-05-01',
  'plan:S1', 'plan:S2a', 'plan:S2b', 'plan:S4d-dock',
  'plan:T-03-001',
]

/** Writes a file under a copied tree, making its folders, and returns the path it was given. */
function put(root: string, path: string, text: string): string {
  mkdirSync(join(root, path, '..'), { recursive: true })
  writeFileSync(join(root, path), text)
  return path
}

const edit = (root: string, path: string, change: (text: string) => string) =>
  writeFileSync(join(root, path), change(readFileSync(join(root, path), 'utf8')))

const initialize = async (root = ROOT): Promise<Message> => {
  const client = connect()
  try {
    return (await client.request('initialize', { config: {}, root })).result as Message
  } finally {
    await client.close()
  }
}

describe('initialize declares the kinds', () => {
  test('adr, spike and question, each with its own statuses and their categories', async () => {
    const { kinds } = await initialize()
    expect(kinds).toEqual([
      {
        name: 'adr',
        transitions: [],
        ops: [],
        roles: [],
        statuses: [
          { name: 'proposed', category: 'review' },
          { name: 'accepted', category: 'done' },
          { name: 'rejected', category: 'dropped' },
          { name: 'deprecated', category: 'dropped' },
          { name: 'superseded', category: 'dropped' },
        ],
      },
      {
        name: 'spike',
        transitions: [],
        ops: [],
        roles: [],
        statuses: [
          { name: 'open', category: 'ready' },
          { name: 'running', category: 'active' },
          { name: 'concluded', category: 'done' },
          { name: 'abandoned', category: 'dropped' },
          { name: 'superseded', category: 'dropped' },
        ],
      },
      {
        name: 'question',
        transitions: [],
        ops: [],
        roles: [],
        statuses: [
          { name: 'open', category: 'ready' },
          { name: 'resolved', category: 'done' },
          { name: 'dropped', category: 'dropped' },
        ],
      },
    ])
  })

  test('declares the links with what the other end calls each', async () => {
    const { links } = await initialize()
    expect(links).toEqual([
      { name: 'blocked-by', dir: 'out', blocking: true },
      { name: 'evidence', dir: 'out', blocking: false, inverse: 'evidence for' },
      { name: 'supersedes', dir: 'out', blocking: false, inverse: 'superseded by' },
      { name: 'blocks', dir: 'out', blocking: false, inverse: 'blocked by' },
    ])
  })

  test('the id patterns take the id and the short id of every kind, and not a made-up one', async () => {
    const { idPattern, shortIdPattern } = await initialize()
    const long = new RegExp(idPattern)
    const short = new RegExp(shortIdPattern)
    const longIds = ['T-03-001-survey-the-quay', 'ADR-004-sunken-quays', 'S2a-berth-pricing', 'S4d-dock-crane-limits']
    for (const id of [...longIds, 'S1-tide-timing', 'Q-03-01-which-berth-first']) expect(long.test(id), id).toBe(true)
    for (const id of ['T-03-001', 'ADR-004', 'ADR-1234', 'S2a', 'S4d-dock', 'S1', 'Q-03-01']) {
      expect(short.test(id), id).toBe(true)
    }
    for (const id of ['ADR-4', 'X-03-001', 'Q-3-1', 's2a', 'ADR-004-']) {
      expect(long.test(id) || short.test(id), id).toBe(false)
    }
  })

  test('is still valid against the generated schema', async () => {
    const schema = JSON.parse(readFileSync(join(REPO, 'schemas', 'InitializeResult.output.json'), 'utf8'))
    const validate = new Ajv2020({ strict: false, allErrors: true }).compile(schema)
    expect(validate(await initialize()), JSON.stringify(validate.errors)).toBe(true)
  })

  test('the ops stay the task ones: a write on another kind is refused by the provider, not by a schema', async () => {
    const { ops } = await initialize()
    expect(ops.map((op: Message) => op.name)).toEqual(['setStatus', 'create'])
    expect(ops.every((op: Message) => op.args?.properties?.status?.enum?.includes('accepted') !== true)).toBe(true)
  })
})

describe('list over the kinds tree', () => {
  test('lists each ADR, spike, question and task once, in short id order, with no invalid file', async () => {
    const result = await listAll(ROOT)
    expect(keysOf(result)).toEqual(KEYS)
    expect(result.invalid).toEqual([])
  })

  test('an item that is not a task names its kind, and a task names none', async () => {
    const result = await listAll(ROOT)
    const kindOf = (id: string) => taskOf(result, id).kind
    expect(kindOf('ADR-001')).toBe('adr')
    expect(kindOf('S2a')).toBe('spike')
    expect(kindOf('S4d-dock')).toBe('spike')
    expect(kindOf('Q-03-01')).toBe('question')
    expect(kindOf('T-03-001')).toBeUndefined()
  })

  test('an ADR has its key, long id, title, path and checkout', async () => {
    expect(taskOf(await listAll(ROOT), 'ADR-004')).toMatchObject({
      key: 'plan:ADR-004',
      id: 'ADR-004-sunken-quays',
      title: 'Sink the quays a little',
      path: 'arch/004-2026-02-16-sunken-quays.md',
      checkout: ROOT,
      state: { status: 'accepted', category: 'done' },
    })
  })

  test('a spike is its PLAN.md: the path and the version are the file, and the suffix may have a hyphen', async () => {
    const result = await listAll(ROOT)
    expect(taskOf(result, 'S2a')).toMatchObject({
      key: 'plan:S2a',
      id: 'S2a-berth-pricing',
      title: 'Try three berth prices',
      path: 'arch/spikes/s2a-2026-02-10-berth-pricing/PLAN.md',
      version: sha256(join(ROOT, 'arch/spikes/s2a-2026-02-10-berth-pricing/PLAN.md')),
    })
    expect(taskOf(result, 'S4d-dock')).toMatchObject({
      id: 'S4d-dock-crane-limits',
      path: 'arch/spikes/s4d-dock-2026-03-01-crane-limits/PLAN.md',
    })
    expect(taskOf(result, 'S1')).toMatchObject({ id: 'S1-tide-timing' })
  })

  test('a question carries the proposal of its folder, and its path and id', async () => {
    expect(taskOf(await listAll(ROOT), 'Q-05-01')).toMatchObject({
      key: 'plan:Q-05-01',
      id: 'Q-05-01-which-lamp-first',
      path: 'arch/proposals/05-lamp-works/questions/Q-05-01-which-lamp-first.md',
      fields: { proposal: '05' },
    })
  })

  test('the kind says what the status word means', async () => {
    const result = await listAll(ROOT)
    const state = (id: string) => taskOf(result, id).state
    expect(state('ADR-001')).toEqual({ status: 'superseded', category: 'dropped' })
    expect(state('ADR-002')).toEqual({ status: 'proposed', category: 'review' })
    expect(state('S2b')).toEqual({ status: 'running', category: 'active' })
    expect(state('S4d-dock')).toEqual({ status: 'open', category: 'ready' })
    expect(state('Q-03-04')).toEqual({ status: 'resolved', category: 'done' })
    expect(state('T-03-001')).toEqual({ status: 'ready', category: 'ready' })
  })

  test('get answers any kind by its long id, short id or key, with the body', async () => {
    const client = await start(ROOT)
    try {
      for (const id of ['ADR-002-berth-prices', 'ADR-002', 'plan:ADR-002']) {
        const { result } = await client.request('get', { id })
        expect(result?.task).toMatchObject({ key: 'plan:ADR-002', kind: 'adr' })
        expect(result?.task.body).toMatch(/Made-up body for ADR-002/)
      }
      expect((await client.request('get', { id: 'S4d-dock-crane-limits' })).result?.task.key).toBe('plan:S4d-dock')
      expect((await client.request('get', { id: 'Q-03-03' })).result?.task.key).toBe('plan:Q-03-03')
    } finally {
      await client.close()
    }
  })
})

describe('each status of each kind', () => {
  const TABLE: Array<[string, string, string, string, string]> = [
    ['adr', ADR_002, 'proposed', 'review', 'ADR-002'],
    ['adr', ADR_002, 'accepted', 'done', 'ADR-002'],
    ['adr', ADR_002, 'rejected', 'dropped', 'ADR-002'],
    ['adr', ADR_002, 'deprecated', 'dropped', 'ADR-002'],
    ['adr', ADR_002, 'superseded', 'dropped', 'ADR-002'],
    ['spike', SPIKE_2B, 'open', 'ready', 'S2b'],
    ['spike', SPIKE_2B, 'running', 'active', 'S2b'],
    ['spike', SPIKE_2B, 'concluded', 'done', 'S2b'],
    ['spike', SPIKE_2B, 'abandoned', 'dropped', 'S2b'],
    ['spike', SPIKE_2B, 'superseded', 'dropped', 'S2b'],
    ['question', QUESTION_Q02, 'open', 'ready', 'Q-03-02'],
    ['question', QUESTION_Q02, 'resolved', 'done', 'Q-03-02'],
    ['question', QUESTION_Q02, 'dropped', 'dropped', 'Q-03-02'],
  ]
  const withStatus = (root: string, path: string, status: string) =>
    edit(root, path, (text) => text.replace(/^status: .*$/m, `status: ${status}`))

  test.each(TABLE)('a %s file (%s) with the status %s is in category %s', async (_k, path, status, category, id) => {
    const root = copyTree('kinds')
    withStatus(root, path, status)
    const result = await listAll(root)
    expect(result.invalid).toEqual([])
    expect(taskOf(result, id).state).toEqual({ status, category })
  })

  const WRONG: Array<[string, string, string]> = [
    ['an ADR', ADR_002, 'running'],
    ['an ADR', ADR_002, 'open'],
    ['a spike', SPIKE_2B, 'accepted'],
    ['a spike', SPIKE_2B, 'planned'],
    ['a question', QUESTION_Q02, 'running'],
    ['a question', QUESTION_Q02, 'done'],
  ]
  test.each(WRONG)('%s with the status %s is invalid, and the reason lists them', async (_name, path, status) => {
    const root = copyTree('kinds')
    withStatus(root, path, status)
    const result = await listAll(root)
    const reason = new RegExp(`status "${status}" is not declared.*it must be one of`)
    expect(result.invalid).toEqual([{ path, checkout: root, reason: expect.stringMatching(reason) }])
    expect(result.tasks).toHaveLength(KEYS.length - 1)
  })
})

describe('links', () => {
  const linksOf = (result: Message, id: string) => taskOf(result, id).links

  test('an ADR\'s evidence is a link to each spike, from a flow list or a block list', async () => {
    const result = await listAll(ROOT)
    expect(linksOf(result, 'ADR-001')).toEqual([{ type: 'evidence', to: 'S1' }])
    expect(linksOf(result, 'ADR-002')).toEqual([
      { type: 'evidence', to: 'S2a' },
      { type: 'evidence', to: 'S2b' },
    ])
  })

  test('an ADR that supersedes another has a supersedes link, after its evidence', async () => {
    expect(linksOf(await listAll(ROOT), 'ADR-004')).toEqual([
      { type: 'evidence', to: 'S1' },
      { type: 'evidence', to: 'S2a' },
      { type: 'supersedes', to: 'ADR-001' },
    ])
  })

  test('a spike that supersedes another has a supersedes link to it', async () => {
    expect(linksOf(await listAll(ROOT), 'S2a')).toEqual([{ type: 'supersedes', to: 'S1' }])
  })

  test('a question\'s blocks are links to the obligations it blocks, by short id', async () => {
    const result = await listAll(ROOT)
    expect(linksOf(result, 'Q-03-01')).toEqual([{ type: 'blocks', to: 'O-03-01' }])
    expect(linksOf(result, 'Q-03-03')).toEqual([
      { type: 'blocks', to: 'O-03-02' },
      { type: 'blocks', to: 'O-03-03' },
    ])
    expect(linksOf(result, 'Q-03-02')).toEqual([])
  })

  test('evidence that names a spike the tree lacks is a link to nothing, and the file is not invalid', async () => {
    const result = await listAll(ROOT)
    expect(linksOf(result, 'ADR-006')).toEqual([{ type: 'evidence', to: 'S9' }])
    expect(keysOf(result)).not.toContain('plan:S9')
    expect(result.invalid).toEqual([])
  })

  test('a task has the links it had, and the new kinds add none to it', async () => {
    expect(linksOf(await listAll(ROOT), 'T-03-001')).toEqual([])
  })

  test.each([
    ['the long id', 'S4d-dock-crane-limits', 'S4d-dock'],
    ['the short id', 'S4d-dock', 'S4d-dock'],
    ['a slug that has a hyphen of its own', 'S2a-berth-pricing', 'S2a'],
    ['a suffix cut short', 'S4d', 'S4d'],
    ['a spike that is not there', 'S7-never-written', 'S7'],
  ])('evidence by %s (%s) is the link to %s', async (_name, text, shortId) => {
    const root = copyTree('kinds')
    edit(root, ADR_002, (body) => body.replace(/evidence:\n {2}- .*\n {2}- .*\n/, `evidence: [${text}]\n`))
    expect(linksOf(await listAll(root), 'ADR-002')).toEqual([{ type: 'evidence', to: shortId }])
  })

  const afterStatus = (line: string) => (text: string) => text.replace(/^(status: .*)$/m, `$1\n${line}`)
  const BAD: Array<[string, string, (text: string) => string, RegExp]> = [
    [
      'evidence that is not a spike id',
      ADR_002,
      (text) => text.replace(/evidence:\n {2}- .*\n {2}- .*\n/, 'evidence: [ADR-001]\n'),
      /evidence entry "ADR-001" is not a spike id/,
    ],
    [
      'evidence that is not a list',
      ADR_002,
      (text) => text.replace(/evidence:\n {2}- .*\n {2}- .*\n/, 'evidence: S1-tide-timing\n'),
      /evidence must be a list/,
    ],
    [
      'evidence with a number in it',
      ADR_002,
      (text) => text.replace(/evidence:\n {2}- .*\n {2}- .*\n/, 'evidence: [1e10]\n'),
      /evidence entry 10000000000 is not a string/,
    ],
    [
      'supersedes that is not an ADR id',
      ADR_002,
      afterStatus('supersedes: [S1-tide-timing]'),
      /"S1-tide-timing" is not an ADR id/,
    ],
    [
      'blocks that is not an obligation id',
      QUESTION_Q02,
      afterStatus('blocks: [T-03-001]'),
      /"T-03-001" is not an obligation id/,
    ],
  ]
  test.each(BAD)('%s makes the file invalid, with the path and the reason', async (_name, path, change, reason) => {
    const root = copyTree('kinds')
    edit(root, path, change)
    const result = await listAll(root)
    expect(result.invalid).toEqual([{ path, checkout: root, reason: expect.stringMatching(reason) }])
    expect(result.tasks).toHaveLength(KEYS.length - 1)
  })
})

describe('invalid files and files that are not items', () => {
  test('a file with no title, a bad status and no frontmatter each go to invalid, and the rest list', async () => {
    const root = copyTree('kinds')
    const adr = put(root, 'arch/007-2026-04-01-no-title.md', '---\nstatus: proposed\n---\n')
    const spike = put(root, 'arch/spikes/s7-2026-04-01-bad/PLAN.md', '---\ntitle: "Bad"\nstatus: planned\n---\n')
    const question = put(root, 'arch/proposals/03-harbor-index/questions/Q-03-07-bare.md', 'no frontmatter\n')
    const result = await listAll(root)
    expect(result.invalid).toEqual([
      { path: adr, checkout: root, reason: expect.stringMatching(/title must be a non-empty string/) },
      { path: question, checkout: root, reason: expect.stringMatching(/no frontmatter/) },
      { path: spike, checkout: root, reason: expect.stringMatching(/status "planned" is not declared/) },
    ])
    expect(keysOf(result)).toEqual(KEYS)
  })

  test('two ADRs with one number are both invalid, and neither lists', async () => {
    const root = copyTree('kinds')
    const twin = put(root, 'arch/002-2026-05-05-twin.md', '---\ntitle: "Twin"\nstatus: proposed\n---\n')
    const result = await listAll(root)
    const reasons = Object.fromEntries(result.invalid.map((entry: Message) => [entry.path, entry.reason]))
    expect(Object.keys(reasons).sort()).toEqual([ADR_002, twin])
    expect(reasons[twin]).toMatch(/ADR-002 is also the id of another file/)
    expect(keysOf(result)).not.toContain('plan:ADR-002')
  })

  test('a spike folder with no PLAN.md, and files named like no kind, are neither listed nor invalid', async () => {
    const root = copyTree('kinds')
    put(root, 'arch/spikes/s8-2026-04-02-empty/RESULTS.md', '---\nconcluded: 2026-04-03\n---\n')
    put(root, 'arch/spikes/README.md', 'notes\n')
    put(root, 'arch/spikes/notes/PLAN.md', '---\ntitle: "x"\nstatus: open\n---\n')
    put(root, 'arch/README.md', 'notes\n')
    put(root, 'arch/7-2026-04-01-short-number.md', '---\ntitle: "x"\nstatus: proposed\n---\n')
    put(root, 'arch/proposals/03-harbor-index/questions/notes.md', 'notes\n')
    put(root, 'arch/proposals/03-harbor-index/questions/Q-03-01-which-berth-first.md.bak', 'old\n')
    const result = await listAll(root)
    expect(keysOf(result)).toEqual(KEYS)
    expect(result.invalid).toEqual([])
  })

  test('an ADR that is a link is not read', async () => {
    const root = copyTree('kinds')
    rmSync(join(root, ADR_002))
    symlinkSync(join(root, 'arch/001-2026-01-05-flat-quays.md'), join(root, ADR_002))
    const result = await listAll(root)
    expect(keysOf(result)).not.toContain('plan:ADR-002')
    expect(result.invalid).toEqual([])
  })

  test('the tasks of the basic tree list as they did, with no kind', async () => {
    const result = await listAll(tree('basic'))
    expect(keysOf(result)).toHaveLength(8)
    expect(result.tasks.every((task: Message) => task.kind === undefined)).toBe(true)
    expect(result.invalid).toEqual([])
  })
})

describe('the hazards of the file format, for each kind', () => {
  const FILES: Array<[string, string, string, (root: string, text: string) => string]> = [
    ['an ADR', 'ADR-007', 'proposed', (root, text) => put(root, 'arch/007-2026-04-01-hazard.md', text)],
    ['a spike', 'S7', 'open', (root, text) => put(root, 'arch/spikes/s7-2026-04-01-hazard/PLAN.md', text)],
    [
      'a question',
      'Q-03-07',
      'open',
      (root, text) => put(root, 'arch/proposals/03-harbor-index/questions/Q-03-07-hazard.md', text),
    ],
  ]
  const invalidAt = async (root: string, path: string) =>
    (await listAll(root)).invalid.find((entry: Message) => entry.path === path)

  describe.each(FILES)('%s', (_name, id, status, write) => {
    test('an unquoted no is the text "no" in YAML 1.2, so it is a title and not false', async () => {
      const root = copyTree('kinds')
      write(root, `---\ntitle: no\nstatus: ${status}\n---\n`)
      expect(taskOf(await listAll(root), id).title).toBe('no')
    })

    test('an unquoted no as the status is not a status', async () => {
      const root = copyTree('kinds')
      const path = write(root, '---\ntitle: "x"\nstatus: no\n---\n')
      expect((await invalidAt(root, path))?.reason).toMatch(/status "no" is not declared/)
    })

    test('a title of 1e10 is a number, so the file is invalid and the reason says title', async () => {
      const root = copyTree('kinds')
      const path = write(root, `---\ntitle: 1e10\nstatus: ${status}\n---\n`)
      expect((await invalidAt(root, path))?.reason).toMatch(/title must be a non-empty string/)
    })

    test('a duplicate key is invalid', async () => {
      const root = copyTree('kinds')
      const path = write(root, `---\ntitle: "x"\nstatus: ${status}\ntitle: "y"\n---\n`)
      expect((await invalidAt(root, path))?.reason).toMatch(/unique|duplicate/i)
    })

    test('an alias bomb is refused at its anchor, quickly, and the other items still list', async () => {
      const root = copyTree('kinds')
      const rows = Array.from({ length: 9 }, (_, at) => {
        const inner = at === 0 ? '"x"' : `*l${at}, *l${at}`
        return `l${at + 1}: &l${at + 1} [${inner}]`
      })
      const path = write(root, `---\ntitle: "bomb"\nstatus: ${status}\n${rows.join('\n')}\n---\n`)
      const started = Date.now()
      const result = await listAll(root)
      expect(Date.now() - started).toBeLessThan(10_000)
      expect(result.invalid.find((entry: Message) => entry.path === path)?.reason).toMatch(/anchor|alias/)
      expect(keysOf(result)).toEqual(KEYS)
    })

    test('a merge key is invalid', async () => {
      const root = copyTree('kinds')
      const path = write(root, `---\ntitle: "x"\nstatus: ${status}\nnotes: { <<: { a: b } }\n---\n`)
      expect((await invalidAt(root, path))?.reason).toMatch(/merge|anchor|alias/)
    })

    test('a frontmatter block over 64 KiB is invalid and says the cap', async () => {
      const root = copyTree('kinds')
      const path = write(root, `---\ntitle: "x"\nstatus: ${status}\nnotes: "${'x'.repeat(70 * 1024)}"\n---\n`)
      expect((await invalidAt(root, path))?.reason).toMatch(/over the 65536-byte cap/)
    })

    test('a body over 1 MiB is invalid and says the cap', async () => {
      const root = copyTree('kinds')
      const path = write(root, `---\ntitle: "x"\nstatus: ${status}\n---\n\n${'x'.repeat(1024 * 1024 + 10)}\n`)
      expect((await invalidAt(root, path))?.reason).toMatch(/over the 1048576-byte cap/)
    })

    test('a conflict marker is invalid', async () => {
      const root = copyTree('kinds')
      const path = write(root, `---\ntitle: "x"\nstatus: ${status}\n---\n\n<<<<<<< ours\nx\n>>>>>>> theirs\n`)
      expect((await invalidAt(root, path))?.reason).toMatch(/conflict marker/)
    })
  })
})

describe('a folder with 2,000 files', () => {
  /** Every page of a list, in order: the keys, and how many tasks each page held. */
  const walk = async (root: string): Promise<{ keys: string[]; sizes: number[] }> => {
    const client = await start(root)
    try {
      const keys: string[] = []
      const sizes: number[] = []
      let cursor: string | undefined
      for (let page = 0; page < 20; page++) {
        const { result } = await client.request('list', cursor === undefined ? {} : { cursor })
        keys.push(...keysOf(result as Message))
        sizes.push(result?.tasks.length)
        cursor = result?.nextCursor
        if (cursor === undefined) break
      }
      return { keys, sizes }
    } finally {
      await client.close()
    }
  }
  const BULK = (title: string, status: string) => `---\ntitle: "${title}"\nstatus: ${status}\n---\n`

  test('2,000 ADR files in arch/ are listed in pages of 500, each once', async () => {
    const root = copyTree('kinds')
    for (let at = 0; at < 2000; at++) {
      const num = String(1000 + at)
      writeFileSync(join(root, `arch/${num}-2026-06-01-bulk.md`), BULK(`Bulk ${num}`, 'accepted'))
    }
    const { keys, sizes } = await walk(root)
    expect(keys).toHaveLength(2000 + KEYS.length)
    expect(new Set(keys).size).toBe(keys.length)
    expect(sizes).toEqual([500, 500, 500, 500, KEYS.length])
  }, 60_000)

  test('2,000 question files in one folder are listed in pages of 500, each once', async () => {
    const root = copyTree('kinds')
    const dir = join(root, 'arch/proposals/06-bulk/questions')
    mkdirSync(dir, { recursive: true })
    for (let at = 0; at < 2000; at++) {
      const name = `Q-${String((at % 90) + 10)}-${String(Math.floor(at / 90) + 10)}-bulk.md`
      writeFileSync(join(dir, name), BULK('Bulk', 'dropped'))
    }
    const { keys, sizes } = await walk(root)
    expect(keys).toHaveLength(2000 + KEYS.length)
    expect(new Set(keys).size).toBe(keys.length)
    expect(sizes).toEqual([500, 500, 500, 500, KEYS.length])
  }, 60_000)

  test('the first page carries the invalid files, and no other page does', async () => {
    const root = copyTree('kinds')
    writeFileSync(join(root, 'arch/009-2026-06-02-bad.md'), 'no frontmatter\n')
    const client = await start(root, { pageSize: 5 })
    try {
      const first = await client.request('list')
      const second = await client.request('list', { cursor: first.result?.nextCursor })
      expect(first.result?.invalid).toHaveLength(1)
      expect(second.result?.invalid).toEqual([])
    } finally {
      await client.close()
    }
  })
})

describe('a write on an ADR, a spike or a question is refused', () => {
  // A plan CLI that records that it was called, so that a test can say it was not.
  const NEVER_CALLED = '#!/bin/sh\necho "$*" >> "$HOME/plan-calls.log"\nexit 3\n'
  const CASES: Array<[string, string, string]> = [
    ['an ADR', 'ADR-002', ADR_002],
    ['a spike', 'S2b', SPIKE_2B],
    ['a question', 'Q-03-02', QUESTION_Q02],
  ]

  describe.each(CASES)('%s', (_name, id, path) => {
    test.each([
      ['apply', 'status', { status: 'done' }],
      ['apply', 'category', { category: 'done' }],
      ['preview', 'status', { status: 'dropped' }],
      ['preview', 'category', { category: 'active' }],
    ])('%s by %s: refused with the reason, no plan CLI call, no file changed', async (method, _by, args) => {
      const repo = makeRepo('kinds', NEVER_CALLED)
      const before = repo.snapshot()
      const client = await repo.provider()
      try {
        const reply = await client.request(method, { op: 'setStatus', id, args, version: versionOf(repo, path) })
        expect(reply.error).toBeUndefined()
        const refusal = expect.stringMatching(/the plan CLI changes these/)
        expect(reply.result).toEqual({ outcome: 'refused', message: refusal })
        expect((reply.result as Message).message).toContain(id)
      } finally {
        await client.close()
      }
      expect(existsSync(join(repo.env.HOME as string, 'plan-calls.log'))).toBe(false)
      expect(repo.snapshot()).toEqual(before)
      expect(repo.git('status', '--porcelain')).toBe('')
    })

    test('a stale version is refused too, and is not a conflict', async () => {
      const repo = makeRepo('kinds', NEVER_CALLED)
      const client = await repo.provider()
      try {
        const reply = await client.request('apply', { op: 'setStatus', id, args: { status: 'done' }, version: 'stale' })
        expect(reply.result).toMatchObject({ outcome: 'refused', message: expect.stringMatching(/plan CLI changes/) })
      } finally {
        await client.close()
      }
    })
  })
})
