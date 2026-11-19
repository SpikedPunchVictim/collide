import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { describe, expect, test } from 'vitest'
import { BODY_CAP } from '../src/read-file.ts'
import { call, EPIC_FILE, fileVersion, tempDir } from './helpers.ts'

const ROOT = '.collide/planning'
const FOLDER = `${ROOT}/epics/ep-001-aa-alpha`
const PROPOSAL = 'pr-0003-km'
const PROPOSAL_PATH = `${FOLDER}/proposals/${PROPOSAL}-quality.md`

type Row = Record<string, any>

const doc = (kind: string, id: string, status: string, body: string): string =>
  `---\nid: ${id}\nkind: ${kind}\nstatus: ${status}\ntitle: "Made-up ${kind}"\nowners: []\nlinks: []\n---\n\n${body}`

const FEATURES = [
  '## Features',
  '',
  '### Packs',
  '- **F1 Trust prompt**: a repo pack asks first.',
  '- **F2 Pack lock**: pins each pack.',
  '',
].join('\n')

const DECISIONS = [
  '## Decision',
  '',
  '- **D1 Keep the files**: Plain files.',
  '  Evidence: ts-9999-zz',
  '- **D2 Use a lock**: A lock file.',
  '  Passed on: A daemon, because it is one more process.',
  '',
].join('\n')

/**
 * A1 has no `State:`. A2 is verified by a concluded spike and has a planned one to settle it. A3 names a task as its
 * spike. A4 cites an item that is not there.
 */
const ASSUMPTIONS = [
  '## Assumptions',
  '',
  '- **A1 Files are enough**: The plan leans on plain files.',
  '  Evidence: ts-0001-aa',
  '- **A2 A lock is safe**: Two writers cannot race.',
  '  State: verified',
  '  Evidence: sp-0001-aa',
  '  Spike: sp-0002-bb',
  '- **A3 Disks are fast**: Scans stay short.',
  '  State: accepted risk',
  '  Spike: ts-0001-aa',
  '- **A4 Nobody edits by hand**: People use the CLI.',
  '  State: contradicted',
  '  Evidence: ts-9999-zz',
  '',
].join('\n')

/**
 * R1 is an open high finding of D2. R2 targets a decision that is not there and is accepted. R3 is dismissed and
 * targets A1. R4 is open, of a feature.
 */
const FINDINGS = [
  '## Findings',
  '',
  '### Red-team',
  '- **R1 high Two writers**: A lock is not enough.',
  '  Target: D2',
  '- **R2 medium Stale lock**: The lock may outlive its owner.',
  '  Target: D5',
  '  Accepted: Added a lock timeout to the plan.',
  '',
  '### Pre-mortem',
  '- **R3 low Slow disk**: A scan is slow.',
  '  Target: A1',
  '  Dismissed: The scan reads a few files.',
  '- **R4 high No prompt**: Nobody reads the prompt.',
  '  Target: F1',
  '',
].join('\n')

const MIXED = [FEATURES, DECISIONS, ASSUMPTIONS, FINDINGS].join('\n')

const files = (body: string, status = 'draft'): Record<string, string> => ({
  [`${FOLDER}/epic.md`]: EPIC_FILE,
  [PROPOSAL_PATH]: doc('proposal', PROPOSAL, status, body),
  [`${FOLDER}/tasks/ts-0001-aa-work.md`]: doc('task', 'ts-0001-aa', 'ready', '## Goal\n'),
  [`${ROOT}/spikes/sp-0001-aa-demo/record.md`]: doc('spike', 'sp-0001-aa', 'concluded', '# Made-up spike\n'),
  [`${ROOT}/spikes/sp-0002-bb-next/record.md`]: doc('spike', 'sp-0002-bb', 'planned', '# Made-up spike\n'),
})

/** A fresh temp repository with `files`; it is not a git repository, which a list and a status write do not need. */
function repoOf(tree: Record<string, string>): string {
  const dir = tempDir()
  for (const [path, text] of Object.entries(tree)) {
    mkdirSync(dirname(join(dir, path)), { recursive: true })
    writeFileSync(join(dir, path), text)
  }
  return dir
}

const taskOf = (result: Row | undefined, id: string): Row => (result?.tasks as Row[]).find((task) => task.id === id)!
const proposalOf = async (root: string): Promise<Row> => taskOf((await call(root, 'list')).result, PROPOSAL)
const setStatus = (root: string, status: string) =>
  call(root, 'apply', {
    op: 'setStatus',
    id: PROPOSAL,
    args: { status },
    version: fileVersion(join(root, PROPOSAL_PATH)),
  })

/** The line of the proposal's file that starts with `start`, counting from 1, the first one at or after `anchor`. */
const lineOf = (body: string, start: string, anchor = start): number => {
  const lines = doc('proposal', PROPOSAL, 'draft', body).split('\n')
  const from = lines.findIndex((line) => line.startsWith(anchor))
  return from + lines.slice(from).findIndex((line) => line.startsWith(start)) + 1
}

describe('a proposal with all three quality parts', () => {
  test('lists the decisions, assumptions and findings, each bad entry in partProblems with its line', async () => {
    const task = await proposalOf(repoOf(files(MIXED)))
    expect(task.decisions.map((one: Row) => one.id)).toEqual(['D1', 'D2'])
    expect(task.assumptions.map((one: Row) => one.id)).toEqual(['A1', 'A2', 'A3', 'A4'])
    expect(task.findings.map((one: Row) => one.id)).toEqual(['R1', 'R2', 'R3', 'R4'])
    const evidence = lineOf(MIXED, '  Evidence: ts-9999-zz')
    const later = lineOf(MIXED, '  Evidence: ts-9999-zz', '- **A4')
    expect(task.partProblems).toEqual([
      { part: 'Decision', line: lineOf(MIXED, '- **D1'), reason: expect.stringMatching(/^D1 .*Passed on:/) },
      { part: 'Decision', line: evidence, reason: expect.stringMatching(/^D1 .*ts-9999-zz.*no item/) },
      { part: 'Assumptions', line: lineOf(MIXED, '  Spike: ts-0001-aa'), reason: expect.stringMatching(/^A3 .*not a/) },
      { part: 'Assumptions', line: later, reason: expect.stringMatching(/^A4 .*ts-9999-zz.*no item/) },
      { part: 'Findings', line: lineOf(MIXED, '  Target: D5'), reason: expect.stringMatching(/^R2 .*D5/) },
    ])
  })

  test('an assumption with no State is unverified and is no problem, and a spike carries its status', async () => {
    const { assumptions, partProblems } = await proposalOf(repoOf(files(MIXED)))
    expect(assumptions[0]).toMatchObject({ id: 'A1', state: 'unverified' })
    expect(partProblems.filter((problem: Row) => /^A1 /.test(problem.reason))).toEqual([])
    expect(assumptions[1]).toMatchObject({
      id: 'A2',
      state: 'verified',
      spike: 'sp-0002-bb',
      spikeStatus: 'planned',
      evidence: [{ target: 'sp-0001-aa', kind: 'item', status: 'concluded' }],
    })
    expect(assumptions[2]).toMatchObject({ state: 'accepted risk', spike: 'ts-0001-aa' })
    expect(assumptions[2]).not.toHaveProperty('spikeStatus')
    expect(assumptions[3]).toMatchObject({ state: 'contradicted', evidence: [{ target: 'ts-9999-zz', kind: 'item' }] })
  })

  test('an open high finding is listed as open and is no problem; a closed one is not open', async () => {
    const { findings, partProblems } = await proposalOf(repoOf(files(MIXED)))
    expect(findings.map((one: Row) => [one.id, one.severity, one.section, one.open])).toEqual([
      ['R1', 'high', 'Red-team', true],
      ['R2', 'medium', 'Red-team', false],
      ['R3', 'low', 'Pre-mortem', false],
      ['R4', 'high', 'Pre-mortem', true],
    ])
    expect(findings[1]).toMatchObject({ accepted: 'Added a lock timeout to the plan.' })
    expect(findings[2]).toMatchObject({ dismissed: 'The scan reads a few files.' })
    expect(partProblems.filter((problem: Row) => /^R[14] /.test(problem.reason))).toEqual([])
  })

  test('a Target reads against the proposal: a decision, an assumption, a feature, or none', async () => {
    const { findings, partProblems } = await proposalOf(repoOf(files(MIXED)))
    expect(findings.map((one: Row) => one.target)).toEqual(['D2', 'D5', 'A1', 'F1'])
    expect(partProblems.filter((problem: Row) => problem.part === 'Findings')).toHaveLength(1)
  })

  test('a Target does not read against another proposal', async () => {
    const other = '## Features\n\n### Packs\n- **F9 Elsewhere**: x\n'
    const tree = {
      ...files(MIXED),
      [`${FOLDER}/proposals/pr-0004-nn-other.md`]: doc('proposal', 'pr-0004-nn', 'draft', other),
    }
    const body = `${FEATURES}\n## Findings\n### Red-team\n- **R1 low T**: x\n  Target: F9\n`
    const root = repoOf({ ...tree, [PROPOSAL_PATH]: doc('proposal', PROPOSAL, 'draft', body) })
    const { partProblems } = await proposalOf(root)
    const line = lineOf(body, '  Target: F9')
    expect(partProblems).toEqual([{ part: 'Findings', line, reason: expect.stringContaining('F9') }])
  })

  test('partProblems are in line order, whichever order the parts are in', async () => {
    const task = await proposalOf(repoOf(files([FINDINGS, ASSUMPTIONS, FEATURES, DECISIONS].join('\n'))))
    const lines = task.partProblems.map((problem: Row) => problem.line)
    expect(lines).toHaveLength(5)
    expect(lines).toEqual([...lines].sort((a, b) => a - b))
    expect(task.partProblems.map((problem: Row) => problem.part)).toEqual([
      'Findings',
      'Assumptions',
      'Assumptions',
      'Decision',
      'Decision',
    ])
  })

  test('list and get say the same, and a get carries the body as written', async () => {
    const root = repoOf(files(MIXED))
    const listed = await proposalOf(root)
    const got = (await call(root, 'get', { id: PROPOSAL })).result?.task
    for (const key of ['decisions', 'assumptions', 'findings', 'partProblems']) expect(got[key]).toEqual(listed[key])
    expect(got.body).toBe(`\n${MIXED}`)
    expect(listed).not.toHaveProperty('body')
  })

  test('a file with CRLF line endings gives the same entries, with the same lines', async () => {
    const crlf = repoOf(
      Object.fromEntries(Object.entries(files(MIXED)).map(([path, text]) => [path, text.replaceAll('\n', '\r\n')])),
    )
    const one = await proposalOf(crlf)
    const two = await proposalOf(repoOf(files(MIXED)))
    for (const key of ['decisions', 'assumptions', 'findings', 'partProblems']) expect(one[key]).toEqual(two[key])
  })

  test('the reply is the same after a JSON round trip, so no key holds undefined', async () => {
    const task = await proposalOf(repoOf(files(MIXED)))
    expect(JSON.parse(JSON.stringify(task))).toEqual(task)
    expect(JSON.stringify(task)).not.toContain('null')
  })

  test('the problems do not refuse a move to proposed, and the file changes by the status line only', async () => {
    const root = repoOf(files(MIXED))
    const before = readFileSync(join(root, PROPOSAL_PATH), 'utf8')
    const { result } = await setStatus(root, 'proposed')
    expect(result).toMatchObject({ outcome: 'ok', task: { state: { status: 'proposed', category: 'review' } } })
    expect(readFileSync(join(root, PROPOSAL_PATH), 'utf8')).toBe(before.replace('status: draft', 'status: proposed'))
  })

  test('the task an apply answers with carries the three parts and their problems', async () => {
    const { result } = await setStatus(repoOf(files(MIXED)), 'proposed')
    expect(result?.task.assumptions[1]).toMatchObject({ id: 'A2', spike: 'sp-0002-bb', spikeStatus: 'planned' })
    expect(result?.task.findings[0]).toMatchObject({ id: 'R1', open: true })
    expect(result?.task.partProblems).toHaveLength(5)
  })

  test('a Features problem still refuses the move, whatever the other parts say', async () => {
    const body = `## Features\n\nstray text\n\n${ASSUMPTIONS}\n${FINDINGS}`
    const { result } = await setStatus(repoOf(files(body)), 'proposed')
    expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('Features') })
  })
})

describe('what reads as no ledger and no findings', () => {
  test('a proposal with neither part has neither key, and nor has a task, an epic, a spike or a question', async () => {
    const tree = {
      ...files('## Context\n\nNothing yet.\n'),
      [`${FOLDER}/questions/qs-0001-aa-q.md`]: doc('question', 'qs-0001-aa', 'open', ASSUMPTIONS + FINDINGS),
      [`${FOLDER}/tasks/ts-0002-bb-t.md`]: doc('task', 'ts-0002-bb', 'ready', ASSUMPTIONS + FINDINGS),
    }
    const { result } = await call(repoOf(tree), 'list')
    for (const task of result?.tasks as Row[]) {
      for (const key of ['assumptions', 'findings', 'partProblems']) expect(task).not.toHaveProperty(key)
    }
  })

  test('a spike record with an Assumptions section is still a spike with none', async () => {
    const tree = {
      ...files(''),
      [`${ROOT}/spikes/sp-0001-aa-demo/record.md`]: doc('spike', 'sp-0001-aa', 'concluded', ASSUMPTIONS),
    }
    const got = (await call(repoOf(tree), 'get', { id: 'sp-0001-aa' })).result?.task
    expect(got.body).toContain('## Assumptions')
    expect(got).not.toHaveProperty('assumptions')
  })

  test('parts of plain prose have no entries and no problems; the body is as written', async () => {
    const prose = '## Assumptions\n\nWe assume files.\n\n## Findings\n\n### Red-team\n\nNothing found.\n'
    const root = repoOf(files(prose))
    const task = (await call(root, 'get', { id: PROPOSAL })).result?.task
    for (const key of ['assumptions', 'findings', 'partProblems']) expect(task).not.toHaveProperty(key)
    expect(task.body).toBe(`\n${prose}`)
  })

  test('a Findings part with only a heading of its own is a problem and no finding', async () => {
    const task = await proposalOf(repoOf(files('## Findings\n\n### Notes\n\nSome text.\n')))
    expect(task).not.toHaveProperty('findings')
    expect(task.partProblems).toEqual([{ part: 'Findings', line: 12, reason: expect.stringContaining('"Notes"') }])
  })

  test('an entry outside the two headings is a problem, is listed, and is kept as written', async () => {
    const body = '## Findings\n\n- **R1 high Loose**: x\n\n### Red-team\n- **R2 low Placed**: y\n'
    const task = await proposalOf(repoOf(files(body)))
    expect(task.findings.map((one: Row) => one.id)).toEqual(['R1', 'R2'])
    expect(task.findings[0]).not.toHaveProperty('section')
    expect(task.partProblems).toEqual([{ part: 'Findings', line: 12, reason: expect.stringMatching(/^R1 .*outside/) }])
  })

  test('a proposal file over the body cap is invalid and is not listed, so nothing of its parts is read', async () => {
    const reply = await call(repoOf(files(`${MIXED}\n${'x'.repeat(BODY_CAP)}`)), 'list')
    expect((reply.result?.tasks as Row[]).map((task) => task.id)).not.toContain(PROPOSAL)
    expect(reply.invalid).toHaveLength(1)
  })

  test('a proposal file with a duplicate key in its frontmatter is invalid and is not listed', async () => {
    const text = doc('proposal', PROPOSAL, 'draft', MIXED).replace('owners: []\n', 'owners: []\nowners: []\n')
    const reply = await call(repoOf({ ...files(''), [PROPOSAL_PATH]: text }), 'list')
    expect((reply.result?.tasks as Row[]).map((task) => task.id)).not.toContain(PROPOSAL)
    expect(reply.invalid).toHaveLength(1)
  })

})
