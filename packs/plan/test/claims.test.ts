import { mkdirSync, readdirSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { describe, expect, test } from 'vitest'
import { CLAIM_CAP, readClaims } from '../src/claims.ts'
import { call, copyFixture, EPIC_FILE, tempDir } from './helpers.ts'

type Row = Record<string, any>

const lines = (...rows: string[]): string => rows.join('\n')

/** A claim block, fenced with three backticks. */
const block = (...rows: string[]): string => lines('```claim', ...rows, '```')

const OK = ['id: Q1.c1', 'label: measured', 'says: A fresh session is cheaper.']

const read = (body: string, firstLine = 1) => readClaims(body, firstLine)

describe('a claim block is read as YAML, with every key', () => {
  test('a block with every key and value is a claim with the four fields the wire carries', () => {
    const { claims, problems } = read(
      block(
        'id: Q6.c2',
        'label: measured                 # measured | inferred | unmeasured',
        'says: A fresh session from a handoff note cost less than a cold resume.',
        'value: 0.004',
        'unit: usd',
        'tolerance: 25%',
        'repro: .collide/planning/spikes/sp-0007-zf-tokens-context/06b-idle-resume.sh',
        'output: out/06b-idle-resume.txt',
        'cost_usd: 0.02',
        'verifiable: yes',
        'load_bearing: true',
      ),
    )
    expect(problems).toEqual([])
    expect(claims).toEqual([{ id: 'Q6.c2', label: 'measured', loadBearing: true, verifiable: true }])
  })

  test('a block with the three required keys has load_bearing false and no verifiable key', () => {
    const { claims } = read(block(...OK))
    expect(claims).toEqual([{ id: 'Q1.c1', label: 'measured', loadBearing: false }])
    expect(claims[0]).not.toHaveProperty('verifiable')
  })

  test('each of the three labels reads', () => {
    for (const label of ['measured', 'inferred', 'unmeasured']) {
      expect(read(block('id: Q1.c1', `label: ${label}`, 'says: x')).claims).toEqual([
        { id: 'Q1.c1', label, loadBearing: false },
      ])
    }
  })

  test('claims come in file order, and prose, headings and other fenced blocks between them are ignored', () => {
    const body = lines(
      '# Spike',
      '',
      'Some prose with id: Q9.c9 in it.',
      block('id: Q2.c1', 'label: inferred', 'says: b'),
      '```yaml',
      'id: Q3.c1',
      '```',
      block('id: Q1.c1', 'label: unmeasured', 'says: a'),
    )
    const { claims, problems } = read(body)
    expect(problems).toEqual([])
    expect(claims.map((claim) => claim.id)).toEqual(['Q2.c1', 'Q1.c1'])
  })

  test('only the info string claim opens one: yaml, claims and Claim do not; a space before it does', () => {
    for (const info of ['yaml', 'claims', 'Claim', 'claim extra']) {
      const { claims, problems } = read(lines(`\`\`\`${info}`, 'nothing: here', '```'))
      expect([claims, problems]).toEqual([[], []])
    }
    expect(read(lines('``` claim', ...OK, '```')).claims).toHaveLength(1)
  })

  test('a tilde fence works the same', () => {
    expect(read(lines('~~~claim', ...OK, '~~~')).claims).toHaveLength(1)
  })
})

describe('each problem is one entry under the part claims, with the line of the block', () => {
  const problemsOf = (...rows: string[]) => read(lines('# Spike', '', block(...rows))).problems

  test('a missing id', () => {
    expect(problemsOf('label: measured', 'says: x')).toEqual([
      { part: 'claims', line: 3, reason: expect.stringContaining('id') },
    ])
  })

  test('a missing says, and a says that is only blanks', () => {
    expect(problemsOf('id: Q1.c1', 'label: measured')).toEqual([
      { part: 'claims', line: 3, reason: expect.stringContaining('says') },
    ])
    expect(problemsOf('id: Q1.c1', 'label: measured', 'says: "  "')).toHaveLength(1)
  })

  test('a missing label', () => {
    expect(problemsOf('id: Q1.c1', 'says: x')).toEqual([
      { part: 'claims', line: 3, reason: expect.stringContaining('label') },
    ])
  })

  test('a bad id: not Q<n>.c<n>, a zero, or the wrong case', () => {
    for (const id of ['c1', 'Q1', 'Q0.c1', 'Q1.c0', 'q1.c1', 'Q1.c1.c2', 'Q01.c1', '1e10']) {
      const found = problemsOf(`id: ${id}`, 'label: measured', 'says: x')
      expect(found, id).toEqual([{ part: 'claims', line: 3, reason: expect.stringContaining('Q<n>.c<n>') }])
    }
  })

  test('a bad label is named, with the three that are allowed', () => {
    expect(problemsOf('id: Q1.c1', 'label: guessed', 'says: x')).toEqual([
      { part: 'claims', line: 3, reason: expect.stringMatching(/"guessed".*measured.*inferred.*unmeasured/) },
    ])
  })

  test('an unknown key is named; two unknown keys are two problems', () => {
    expect(problemsOf(...OK, 'colour: red')).toEqual([
      { part: 'claims', line: 3, reason: expect.stringContaining('"colour"') },
    ])
    expect(problemsOf(...OK, 'colour: red', 'shape: round')).toHaveLength(2)
  })

  test('one block with an unknown key and a bad label has two problems, both on the block', () => {
    const found = problemsOf('id: Q1.c1', 'label: guessed', 'says: x', 'colour: red')
    expect(found.map((problem) => problem.line)).toEqual([3, 3])
  })

  test('a repeated id is a problem on the second block, which is not listed; the first is kept', () => {
    const body = lines(block(...OK), '', block('id: Q1.c1', 'label: inferred', 'says: again'))
    const { claims, problems } = read(body)
    expect(claims).toEqual([{ id: 'Q1.c1', label: 'measured', loadBearing: false }])
    expect(problems).toEqual([{ part: 'claims', line: 7, reason: expect.stringMatching(/Q1\.c1.*line 1/) }])
  })

  test('a YAML error, a block that is not a mapping, and an empty block', () => {
    expect(problemsOf('id: [Q1.c1')).toEqual([{ part: 'claims', line: 3, reason: expect.stringContaining('YAML') }])
    expect(problemsOf('- Q1.c1', '- Q1.c2')).toEqual([
      { part: 'claims', line: 3, reason: expect.stringContaining('mapping') },
    ])
    expect(problemsOf()).toEqual([{ part: 'claims', line: 3, reason: expect.stringContaining('mapping') }])
  })

  test('a wrong type is a problem: says as a number, a flag or yes/no as another word, cost_usd as text', () => {
    for (const [row, key] of [
      ['says: 1e10', 'says'],
      ['load_bearing: maybe', 'load_bearing'],
      ['verifiable: true', 'verifiable'],
      ['verifiable: perhaps', 'verifiable'],
      ['cost_usd: cheap', 'cost_usd'],
      ['cost_usd: -1', 'cost_usd'],
      ['unit: 5', 'unit'],
    ] as const) {
      const rows = row.startsWith('says') ? OK.filter((one) => !one.startsWith('says')) : OK
      const found = problemsOf(...rows, row)
      expect(found, row).toHaveLength(1)
      expect(found[0]?.reason, row).toContain(key)
    }
  })

  test('a block with a problem is not a claim, and every other block still reads', () => {
    const body = lines(
      block('id: Q1.c1', 'label: guessed', 'says: x'),
      block('id: Q1.c2', 'label: inferred', 'says: y'),
    )
    const { claims, problems } = read(body)
    expect(claims.map((claim) => claim.id)).toEqual(['Q1.c2'])
    expect(problems).toHaveLength(1)
  })

  test('two bad blocks are two entries, each with its own line', () => {
    const first = block('id: Q1.c1', 'label: measured')
    const body = lines('intro', first, 'between', block('id: Q1.c2', 'label: x', 'says: y'))
    expect(read(body).problems.map((problem) => problem.line)).toEqual([2, 7])
  })

  test('the first line of the body is a line of the file, so a problem names that line', () => {
    const { problems } = read(lines('intro', block('id: Q1.c1', 'label: measured')), 10)
    expect(problems.map((problem) => problem.line)).toEqual([11])
  })
})

describe('fences are matched by length, and a block is only read where a block is', () => {
  test('a claim block inside a four-backtick fence is text, as the plan shows one', () => {
    const body = lines('````markdown', block('id: Q1', 'says: bad on purpose'), '````')
    expect(read(body)).toEqual({ claims: [], problems: [] })
  })

  test('a five-backtick fence holds a four-backtick one holding a claim block', () => {
    const body = lines('`````', '````markdown', block(...OK), '````', '`````')
    expect(read(body)).toEqual({ claims: [], problems: [] })
  })

  test('a tilde fence holds a claim block of backticks', () => {
    expect(read(lines('~~~markdown', block(...OK), '~~~'))).toEqual({ claims: [], problems: [] })
  })

  test('after the longer fence closes, the next block is read', () => {
    const body = lines('````', block('id: Q1'), '````', block(...OK))
    expect(read(body).claims).toHaveLength(1)
    expect(read(body).problems).toEqual([])
  })

  test('a shorter fence does not close a longer one, so what follows it is still inside', () => {
    const body = lines('````', '```', block('id: Q1'), '````', block(...OK))
    expect(read(body)).toEqual({ claims: [{ id: 'Q1.c1', label: 'measured', loadBearing: false }], problems: [] })
  })

  test('an indented fence is read, and the indent is taken off its lines', () => {
    const body = lines('- an item', '', '  ```claim', '  id: Q1.c1', '  label: inferred', '  says: x', '  ```')
    const { claims, problems } = read(body)
    expect(problems).toEqual([])
    expect(claims).toEqual([{ id: 'Q1.c1', label: 'inferred', loadBearing: false }])
  })

  test('a block with no closing fence is one problem on its opening line and reads nothing', () => {
    const body = lines('intro', '```claim', ...OK, '', 'prose that follows')
    expect(read(body)).toEqual({
      claims: [],
      problems: [{ part: 'claims', line: 2, reason: expect.stringContaining('closing') }],
    })
  })

  test('a closing fence of more characters than the opening one closes it', () => {
    expect(read(lines('```claim', ...OK, '`````')).claims).toHaveLength(1)
  })
})

describe('line ends and tabs', () => {
  test('CRLF gives the same claims and the same lines as LF', () => {
    const body = lines('# Spike', '', block(...OK), '', block('id: Q1.c2', 'label: measured'))
    expect(read(body.replaceAll('\n', '\r\n'))).toEqual(read(body))
    expect(read(body).problems).toHaveLength(1)
  })

  test('a tab where YAML wants an indent is a YAML problem; a tab inside a value is fine', () => {
    expect(read(block(...OK, 'value:\t\t3')).problems).toEqual([])
    const bad = read(block('id: Q1.c1', 'label: measured', 'says:', '\t- tabbed'))
    expect(bad.claims).toEqual([])
    expect(bad.problems).toEqual([{ part: 'claims', line: 1, reason: expect.stringContaining('YAML') }])
  })
})

describe('the hazards of spike S5', () => {
  test('an unquoted no is the text "no", so verifiable: no is false and load_bearing: no is refused', () => {
    expect(read(block(...OK, 'verifiable: no')).claims[0]).toEqual({
      id: 'Q1.c1',
      label: 'measured',
      loadBearing: false,
      verifiable: false,
    })
    const refused = read(block(...OK, 'load_bearing: no'))
    expect(refused.claims).toEqual([])
    expect(refused.problems[0]?.reason).toContain('load_bearing')
  })

  test('yes is the text "yes", so load_bearing: yes is refused and verifiable: yes is true', () => {
    expect(read(block(...OK, 'load_bearing: yes')).problems).toHaveLength(1)
    expect(read(block(...OK, 'verifiable: yes')).claims[0]?.verifiable).toBe(true)
  })

  test('a hash like 1e10 is a number, and where text belongs it is a problem; quoted it is text', () => {
    const bare = read(block('id: Q1.c1', 'label: measured', 'says: 1e10'))
    expect(bare.problems[0]?.reason).toMatch(/says.*text.*quote/)
    expect(read(block('id: Q1.c1', 'label: measured', 'says: "1e10"')).problems).toEqual([])
    expect(read(block(...OK, 'value: 1e10')).problems).toEqual([])
  })

  test('a duplicate key is a problem and the block is not a claim', () => {
    const found = read(block(...OK, 'says: twice'))
    expect(found.claims).toEqual([])
    expect(found.problems).toEqual([{ part: 'claims', line: 1, reason: expect.stringContaining('YAML') }])
  })

  test('an alias bomb is a problem, and the block is small enough to be read', () => {
    const aliases = Array.from({ length: 200 }, () => '*a').join(', ')
    const body = block(...OK, 'seed: &a [x, y, z]', `bomb: [${aliases}]`)
    expect(Buffer.byteLength(body)).toBeLessThan(CLAIM_CAP)
    const found = read(body)
    expect(found.claims).toEqual([])
    expect(found.problems).toHaveLength(1)
    expect(found.problems[0]?.reason).toMatch(/YAML|alias/i)
  })

  test('a merge key and a tag are not honoured: the block is refused or a key is unknown', () => {
    expect(read(block(...OK, 'base: &b { a: 1 }', '<<: *b')).claims).toEqual([])
    expect(read(block(...OK, 'value: !!str 5')).claims).toHaveLength(1)
  })

  test('a block over the cap is one problem and is not parsed', () => {
    const says = 'x'.repeat(CLAIM_CAP)
    const found = read(block('id: Q1.c1', 'label: measured', `says: ${says}`))
    expect(found.claims).toEqual([])
    expect(found.problems).toEqual([
      { part: 'claims', line: 1, reason: expect.stringMatching(new RegExp(`over the ${CLAIM_CAP}-byte cap`)) },
    ])
  })

  test('a block just under the cap reads', () => {
    const head = 'id: Q1.c1\nlabel: measured\nsays: '
    const says = 'x'.repeat(CLAIM_CAP - Buffer.byteLength(head))
    expect(read(block(...`${head}${says}`.split('\n'))).problems).toEqual([])
  })
})

// ---- through the provider -------------------------------------------------------------------------------------------

const ROOT = '.collide/planning'

const spike = (id: string, status: string, body: string, extra = ''): string =>
  `---\nid: ${id}\nkind: spike\nstatus: ${status}\ntitle: "Made-up"\nowners: []\nlinks: []\n${extra}---\n\n${body}`

function repoOf(files: Record<string, string>): string {
  const dir = tempDir()
  for (const [path, text] of Object.entries(files)) {
    mkdirSync(dirname(join(dir, path)), { recursive: true })
    writeFileSync(join(dir, path), text)
  }
  return dir
}

const spikeAt = (id: string, text: string): Record<string, string> => ({
  [`${ROOT}/spikes/${id}-demo/record.md`]: text,
})

const tasksOf = async (root: string): Promise<Row[]> => ((await call(root, 'list')).result?.tasks ?? []) as Row[]

describe('a spike record over the provider', () => {
  const bad = lines(
    '# Spike',
    '',
    block('label: measured', 'says: no id'),
    '',
    block('id: Q1.c2', 'label: guessed', 'says: a bad label'),
    '',
    block(...OK),
    '',
    block('id: Q1.c1', 'label: inferred', 'says: the id repeats'),
  )

  test('a block with no id, label: guessed and a repeated id each have a problem and a line', async () => {
    const root = repoOf(spikeAt('sp-0001-aa', spike('sp-0001-aa', 'results', bad)))
    const [task] = await tasksOf(root)
    expect(task).toMatchObject({ id: 'sp-0001-aa', kind: 'spike', state: { status: 'results', category: 'review' } })
    expect(task?.partProblems).toEqual([
      { part: 'claims', line: 12, reason: expect.stringContaining('id') },
      { part: 'claims', line: 17, reason: expect.stringContaining('guessed') },
      { part: 'claims', line: 29, reason: expect.stringContaining('Q1.c1') },
    ])
    expect(task?.claims).toEqual([{ id: 'Q1.c1', label: 'measured', loadBearing: false }])
  })

  test('the lines are lines of the file: the frontmatter counts', async () => {
    const root = repoOf(spikeAt('sp-0001-aa', spike('sp-0001-aa', 'planned', block('id: Q1.c1'))))
    const [task] = await tasksOf(root)
    expect(task?.partProblems[0].line).toBe(10)
  })

  test('get returns the same claims and problems as list, and a record with good claims has no problems', async () => {
    const text = spike('sp-0001-aa', 'concluded', lines('# S', block(...OK, 'load_bearing: true')))
    const root = repoOf(spikeAt('sp-0001-aa', text))
    const [listed] = await tasksOf(root)
    const got = (await call(root, 'get', { id: 'sp-0001-aa' })).result?.task
    expect(listed?.claims).toEqual([{ id: 'Q1.c1', label: 'measured', loadBearing: true }])
    expect(got?.claims).toEqual(listed?.claims)
    expect(listed).not.toHaveProperty('partProblems')
    expect(got).not.toHaveProperty('partProblems')
  })

  test('a record with no claim block has no claims key and no problem', async () => {
    const root = repoOf(spikeAt('sp-0001-aa', spike('sp-0001-aa', 'concluded', '# S\n\nJust prose.\n')))
    const [task] = await tasksOf(root)
    for (const key of ['claims', 'partProblems', 'preregistration']) expect(task).not.toHaveProperty(key)
  })

  test('only a spike is searched: a proposal, a question and a task that show a claim block read none', async () => {
    const body = lines('## Goal', '', block('id: Q1'))
    const doc = (kind: string, id: string, status: string): string =>
      `---\nid: ${id}\nkind: ${kind}\nstatus: ${status}\ntitle: "Made-up"\nowners: []\nlinks: []\n---\n\n${body}\n`
    const folder = `${ROOT}/epics/ep-001-aa-alpha`
    const root = repoOf({
      [`${folder}/epic.md`]: EPIC_FILE,
      [`${folder}/proposals/pr-0001-aa-p.md`]: doc('proposal', 'pr-0001-aa', 'draft'),
      [`${folder}/questions/qs-0001-aa-q.md`]: doc('question', 'qs-0001-aa', 'open'),
      [`${folder}/tasks/ts-0001-aa-t.md`]: doc('task', 'ts-0001-aa', 'ready'),
    })
    const tasks = await tasksOf(root)
    expect(tasks).toHaveLength(4)
    for (const task of tasks) {
      for (const key of ['claims', 'partProblems']) expect(task, task.id).not.toHaveProperty(key)
    }
  })

  test("Collide's ten records, copied, read with no problem and no claims", async () => {
    const root = copyFixture('collide-spikes')
    const spikes = (await tasksOf(root)).filter((task) => task.kind === 'spike')
    expect(spikes.length).toBeGreaterThanOrEqual(10)
    for (const task of spikes) {
      for (const key of ['claims', 'partProblems', 'preregistration']) expect(task, task.id).not.toHaveProperty(key)
    }
  })

  test("every record in this repository's spikes folder reads with no problem and no claims", async () => {
    const root = join(import.meta.dirname, '..', '..', '..')
    const entries = readdirSync(join(root, ROOT, 'spikes'), { withFileTypes: true })
    const folders = entries.filter((entry) => entry.isDirectory())
    const spikes = (await tasksOf(root)).filter((task) => task.kind === 'spike')
    expect(spikes).toHaveLength(folders.length)
    expect(spikes.length).toBeGreaterThanOrEqual(10)
    for (const task of spikes) {
      for (const key of ['claims', 'partProblems']) expect(task, task.id).not.toHaveProperty(key)
    }
  })
})
