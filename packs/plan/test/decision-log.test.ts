import { cpSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { describe, expect, test } from 'vitest'
import { BODY_CAP } from '../src/read-file.ts'
import { call, EPIC_FILE, fileVersion, tempDir } from './helpers.ts'

const ROOT = '.collide/planning'
const FOLDER = `${ROOT}/epics/ep-001-aa-alpha`
const PROPOSAL = 'pr-0003-km'
const PROPOSAL_PATH = `${FOLDER}/proposals/${PROPOSAL}-log.md`

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

/**
 * A proposal whose Decision log has two entries. D1 has no alternative and cites an item that is not there. D2 cites a
 * concluded spike, a feature of another proposal, a task and a path. The body starts on line 9 of the file, with the
 * blank line after the frontmatter, so `## Decision` is line 10 and D1 is line 12.
 */
const LOG = [
  '## Decision',
  '',
  '- **D1 Keep the files**: Plain files.',
  '  Evidence: ts-9999-zz',
  '- **D2 Use a lock**: A lock file.',
  '  Passed on: A daemon, because it is one more process.',
  '  Evidence: sp-0001-aa, pr-0004-nn#F1, ts-0001-aa, core/src/item.rs',
  '  Confidence: high',
  '',
].join('\n')

const files = (body: string, status = 'draft'): Record<string, string> => ({
  [`${FOLDER}/epic.md`]: EPIC_FILE,
  [PROPOSAL_PATH]: doc('proposal', PROPOSAL, status, body),
  [`${FOLDER}/proposals/pr-0004-nn-other.md`]: doc('proposal', 'pr-0004-nn', 'accepted', FEATURES),
  [`${FOLDER}/tasks/ts-0001-aa-work.md`]: doc('task', 'ts-0001-aa', 'ready', '## Goal\n'),
  [`${ROOT}/spikes/sp-0001-aa-demo/record.md`]: doc('spike', 'sp-0001-aa', 'concluded', '# Made-up spike\n'),
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
const setStatus = (root: string, status: string) =>
  call(root, 'apply', {
    op: 'setStatus',
    id: PROPOSAL,
    args: { status },
    version: fileVersion(join(root, PROPOSAL_PATH)),
  })

describe('a proposal with a Decision log', () => {
  test('lists both decisions, with each bad entry in partProblems by its line in the file', async () => {
    const root = repoOf(files(LOG))
    const task = taskOf((await call(root, 'list')).result, PROPOSAL)
    expect(task.decisions.map((decision: Row) => decision.id)).toEqual(['D1', 'D2'])
    expect(task.partProblems).toEqual([
      { part: 'Decision', line: 12, reason: expect.stringMatching(/^D1 .*Passed on:/) },
      { part: 'Decision', line: 13, reason: expect.stringMatching(/^D1 .*ts-9999-zz.*no item/) },
    ])
  })

  test('a cited spike carries its status, a cited feature its proposal, and a path is as written', async () => {
    const root = repoOf(files(LOG))
    const task = taskOf((await call(root, 'list')).result, PROPOSAL)
    expect(task.decisions[1]).toMatchObject({
      id: 'D2',
      title: 'Use a lock',
      text: 'A lock file.',
      passedOn: ['A daemon, because it is one more process.'],
      confidence: 'high',
      evidence: [
        { target: 'sp-0001-aa', kind: 'item', status: 'concluded' },
        { target: 'pr-0004-nn#F1', kind: 'feature', proposal: 'pr-0004-nn' },
        { target: 'ts-0001-aa', kind: 'item', status: 'ready' },
        { target: 'core/src/item.rs', kind: 'path' },
      ],
    })
    expect(task.decisions[0].evidence).toEqual([{ target: 'ts-9999-zz', kind: 'item' }])
  })

  test('a feature that the proposal has not, and one that is a proposal that is not there, are problems', async () => {
    const body = '## Decision\n- **D1 A**: x\n  Passed on: y\n  Evidence: pr-0004-nn#F7, pr-0099-zz#F1\n'
    const root = repoOf(files(body))
    const { partProblems } = taskOf((await call(root, 'list')).result, PROPOSAL)
    expect(partProblems).toEqual([
      { part: 'Decision', line: 13, reason: expect.stringContaining('pr-0004-nn has no feature F7') },
      { part: 'Decision', line: 13, reason: expect.stringContaining('pr-0099-zz') },
    ])
  })

  test('list and get say the same, and a get carries the body as written', async () => {
    const root = repoOf(files(LOG))
    const listed = taskOf((await call(root, 'list')).result, PROPOSAL)
    const got = (await call(root, 'get', { id: PROPOSAL })).result?.task
    expect(got.decisions).toEqual(listed.decisions)
    expect(got.partProblems).toEqual(listed.partProblems)
    expect(got.body).toBe(`\n${LOG}`)
    expect(listed).not.toHaveProperty('body')
  })

  test('a file with CRLF line endings gives the same decisions, with the same lines', async () => {
    const crlf = repoOf(
      Object.fromEntries(Object.entries(files(LOG)).map(([path, text]) => [path, text.replaceAll('\n', '\r\n')])),
    )
    const lf = repoOf(files(LOG))
    const one = taskOf((await call(crlf, 'list')).result, PROPOSAL)
    const two = taskOf((await call(lf, 'list')).result, PROPOSAL)
    expect(one.decisions).toEqual(two.decisions)
    expect(one.partProblems).toEqual(two.partProblems)
  })

  test('the problems do not refuse a move to proposed, and the file changes by the status line only', async () => {
    const root = repoOf(files(`${FEATURES}\n${LOG}`))
    const before = readFileSync(join(root, PROPOSAL_PATH), 'utf8')
    const { result } = await setStatus(root, 'proposed')
    expect(result).toMatchObject({ outcome: 'ok', task: { state: { status: 'proposed', category: 'review' } } })
    expect(readFileSync(join(root, PROPOSAL_PATH), 'utf8')).toBe(before.replace('status: draft', 'status: proposed'))
  })

  test('the task an apply answers with carries the decisions, with the evidence resolved', async () => {
    const root = repoOf(files(`${FEATURES}\n${LOG}`))
    const { result } = await setStatus(root, 'proposed')
    expect(result?.task.decisions[1].evidence[0]).toEqual({ target: 'sp-0001-aa', kind: 'item', status: 'concluded' })
    expect(result?.task.partProblems).toHaveLength(2)
  })

  test('a Features problem still refuses the move, whatever the log says', async () => {
    const root = repoOf(files(`## Features\n\nstray text\n\n${LOG}`))
    const { result } = await setStatus(root, 'proposed')
    expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('Features') })
  })
})

describe('what reads as no Decision log', () => {
  test('a Decision section of plain text has no decisions and no partProblems; its body is as written', async () => {
    const plain = ['## Decision', '', 'We chose files, because they diff.', '', '- **Chosen**: files']
    const body = `${FEATURES}\n${plain.join('\n')}\n  Evidence: ts-9999-zz\n`
    const root = repoOf(files(body))
    const got = (await call(root, 'get', { id: PROPOSAL })).result?.task
    expect(got).not.toHaveProperty('decisions')
    expect(got).not.toHaveProperty('partProblems')
    expect(got.body).toBe(`\n${body}`)
    const listed = taskOf((await call(root, 'list')).result, PROPOSAL)
    expect(listed).not.toHaveProperty('decisions')
    expect(listed).not.toHaveProperty('partProblems')
  })

  test('a proposal with no Decision section, a task and an epic carry none of the quality-part keys', async () => {
    const root = repoOf(files('## Context\n\nNothing yet.\n'))
    const { result } = await call(root, 'list')
    for (const task of result?.tasks as Row[]) {
      for (const key of ['decisions', 'assumptions', 'findings', 'partProblems']) expect(task).not.toHaveProperty(key)
    }
  })

  test('a question or a task with entries in its Decision section has no decisions: only a proposal does', async () => {
    const body = '## Decision\n- **D1 A**: x\n  Evidence: ts-9999-zz\n'
    const tree = {
      ...files('## Context\n'),
      [`${FOLDER}/questions/qs-0001-aa-q.md`]: doc('question', 'qs-0001-aa', 'open', body),
      [`${FOLDER}/tasks/ts-0002-bb-t.md`]: doc('task', 'ts-0002-bb', 'ready', body),
    }
    const { result } = await call(repoOf(tree), 'list')
    for (const id of ['qs-0001-aa', 'ts-0002-bb']) {
      expect(taskOf(result, id)).not.toHaveProperty('decisions')
      expect(taskOf(result, id)).not.toHaveProperty('partProblems')
    }
  })

  test('a proposal file over the body cap is invalid and is not listed, so nothing of its log is read', async () => {
    const big = `${LOG}\n${'x'.repeat(BODY_CAP)}`
    const root = repoOf(files(big))
    const reply = await call(root, 'list')
    expect((reply.result?.tasks as Row[]).map((task) => task.id)).not.toContain(PROPOSAL)
    expect(reply.invalid).toHaveLength(1)
  })

  test('a proposal file with a duplicate key in its frontmatter is invalid and is not listed', async () => {
    const text = doc('proposal', PROPOSAL, 'draft', LOG).replace('owners: []\n', 'owners: []\nowners: []\n')
    const root = repoOf({ ...files(''), [PROPOSAL_PATH]: text })
    const reply = await call(root, 'list')
    expect((reply.result?.tasks as Row[]).map((task) => task.id)).not.toContain(PROPOSAL)
    expect(reply.invalid).toHaveLength(1)
  })
})

describe("this repository's own planning tree", () => {
  /** A copy of the epics and each spike's record.md: a spike folder holds more, and none of it is read. */
  function copyOfTree(): string {
    const source = join(import.meta.dirname, '..', '..', '..', ROOT)
    const dir = tempDir()
    const spikes = `${source}/spikes/`
    cpSync(source, join(dir, ROOT), {
      recursive: true,
      filter: (path) =>
        !path.startsWith(spikes) || path.slice(spikes.length).split('/').length < 2 || path.endsWith('/record.md'),
    })
    return dir
  }

  test('reads with no decisions, no assumptions, no findings and no partProblems on any item', async () => {
    const { result, invalid } = await call(copyOfTree(), 'list')
    expect(invalid).toEqual([])
    expect((result?.tasks as Row[]).length).toBeGreaterThan(50)
    for (const task of result?.tasks as Row[]) {
      for (const key of ['decisions', 'assumptions', 'findings', 'partProblems']) {
        expect(task, `${task.id} has ${key}`).not.toHaveProperty(key)
      }
    }
  })

  test('a spike record with a Decision section is still a spike with no decisions', async () => {
    const { result } = await call(copyOfTree(), 'get', { id: 'sp-0003-ye' })
    expect(result?.task.kind).toBe('spike')
    expect(result?.task.body).toContain('## Decision')
    expect(result?.task).not.toHaveProperty('decisions')
  })
})
