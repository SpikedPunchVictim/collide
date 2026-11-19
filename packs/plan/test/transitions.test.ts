import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, test } from 'vitest'
import { PROPOSAL_MOVES } from '../src/transitions.ts'
import { call, commitAll, EPIC_FILE, fileVersion, git, makeRepo } from './helpers.ts'

const EPICS = '.collide/planning/epics'
const FOLDER = 'ep-001-aa-alpha'
const PROPOSAL_FILE = `${FOLDER}/proposals/pr-0001-aa-plan.md`
const QUESTION_ONE = `${FOLDER}/questions/qu-0001-aa-why.md`
const QUESTION_TWO = `${FOLDER}/questions/qu-0002-bb-how.md`
const TASK_FILE = `${FOLDER}/tasks/ts-0001-aa-work.md`

type Row = Record<string, any>

const STATUSES = ['draft', 'proposed', 'accepted', 'deferred', 'rejected', 'superseded']
const TODAY = '2026-06-15'
const person = { kind: 'person', at: TODAY }
const agent = { kind: 'agent', at: TODAY, run: 'R-7' }

const GOOD_FEATURES = '## Features\n\n### Packs\n- **F1 Trust prompt**: a repo pack asks first.\n'

const doc = (kind: string, id: string, status: string, extra = '', links = 'links: []\n', body = ''): string =>
  `---\nid: ${id}\nkind: ${kind}\nstatus: ${status}\ntitle: "Made-up ${kind}"\nowners: []\n` +
  `${links}${extra}---\n\n${body}`
const links = (...pairs: Array<[string, string]>): string =>
  `links:\n${pairs.map(([type, to]) => `  - { type: ${type}, to: ${to} }\n`).join('')}`
const proposal = (status: string, extra = '', linkLines?: string): string =>
  doc('proposal', 'pr-0001-aa', status, extra, linkLines, `${GOOD_FEATURES}\n## Decision\n`)
const question = (status = 'open', extra = '', body = '## Question\n\n## Decision\n'): string =>
  doc('question', 'qu-0001-aa', status, extra, undefined, body)
/** The question as `resolve` leaves it: `resolved_by` goes right after the status line. */
const resolvedQuestion = (by: string, body: string): string =>
  question('resolved', '', body).replace('status: resolved\n', `status: resolved\nresolved_by: ${by}\n`)

const repoOf = (files: Record<string, string>): string => makeRepo({ [`${FOLDER}/epic.md`]: EPIC_FILE, ...files })
/** As `repoOf`, and a git repository with everything committed, which `reid` needs to read the refs for taken ids. */
const gitRepoOf = (files: Record<string, string>): string => {
  const root = repoOf(files)
  git(root, 'init', '--quiet')
  commitAll(root, 'fixture')
  return root
}
const read = (root: string, file: string): string => readFileSync(join(root, EPICS, file), 'utf8')

/** One `apply` of `op` on `id`, whose file is `file`, as `actor`. */
const send = (root: string, file: string, id: string, op: string, args: Row = {}, actor: Row | null = person) =>
  call(root, 'apply', {
    op,
    id,
    args,
    version: fileVersion(join(root, EPICS, file)),
    ...(actor === null ? {} : { actor }),
  })
const onProposal = (root: string, op: string, args: Row = {}, actor: Row | null = person) =>
  send(root, PROPOSAL_FILE, 'pr-0001-aa', op, args, actor)
const onQuestion = (root: string, op: string, args: Row = {}, actor: Row | null = person) =>
  send(root, QUESTION_ONE, 'qu-0001-aa', op, args, actor)

const declared = async (): Promise<Row> => (await call(repoOf({}), 'initialize')).result as Row
const kindOf = async (name: string): Promise<Row> => (await declared()).kinds.find((kind: Row) => kind.name === name)

describe('initialize declares accept, defer, reject and resolve beside setStatus', () => {
  test('a proposal has setStatus, accept, defer and reject, and no resolve', async () => {
    const found = await kindOf('proposal')
    expect(found.ops.map((op: Row) => op.name)).toEqual(['setStatus', 'accept', 'defer', 'reject'])
  })

  test('a question has setStatus and resolve, and a task or an epic has neither', async () => {
    expect((await kindOf('question')).ops.map((op: Row) => op.name)).toEqual(['setStatus', 'resolve'])
    expect((await kindOf('task')).ops).toBeUndefined()
    expect((await kindOf('epic')).ops).toBeUndefined()
  })

  test('accept takes nothing, defer needs a trigger, reject may give a reason, resolve needs text', async () => {
    const proposalOps = (await kindOf('proposal')).ops as Row[]
    const args = (name: string): Row => proposalOps.find((op) => op.name === name)?.args
    expect(args('accept')).toEqual({ type: 'object', properties: {}, additionalProperties: false })
    expect(args('defer')).toMatchObject({
      properties: { trigger: { type: 'string', minLength: 1 } },
      required: ['trigger'],
      additionalProperties: false,
    })
    expect(args('reject')).toMatchObject({ properties: { reason: { type: 'string', minLength: 1 } } })
    expect(args('reject').required).toBeUndefined()
    const resolve = ((await kindOf('question')).ops as Row[]).find((op) => op.name === 'resolve')?.args
    expect(resolve).toMatchObject({
      properties: { by: { type: 'string', pattern: '^[a-z]{2}-[0-9]{3,}-[a-z]{2}$' }, text: { type: 'string' } },
      required: ['text'],
      additionalProperties: false,
    })
  })
})

describe('the transition table of a proposal', () => {
  const EDGES: Record<string, string[]> = {
    draft: ['proposed', 'rejected'],
    proposed: ['accepted', 'deferred', 'rejected', 'draft'],
    deferred: ['proposed', 'rejected'],
    accepted: ['superseded'],
    rejected: ['draft'],
    superseded: [],
  }

  test('is the one the item gives, in one table', () => {
    expect(Object.fromEntries(Object.entries(PROPOSAL_MOVES).map(([from, to]) => [from, [...to].sort()]))).toEqual(
      Object.fromEntries(Object.entries(EDGES).map(([from, to]) => [from, [...to].sort()])),
    )
  })

  /** The op that makes the move to `to`: the words a person-only status has their own op, for a trigger or a reason. */
  const moveTo = (root: string, to: string) => {
    if (to === 'accepted') return onProposal(root, 'accept')
    if (to === 'deferred') return onProposal(root, 'defer', { trigger: 'when the pack lock lands' })
    if (to === 'rejected') return onProposal(root, 'reject')
    return onProposal(root, 'setStatus', { status: to })
  }

  const PAIRS = STATUSES.flatMap((from) => STATUSES.filter((to) => to !== from).map((to) => [from, to] as const))
  test.each(PAIRS)('from %s to %s', async (from, to) => {
    const root = repoOf({ [PROPOSAL_FILE]: proposal(from) })
    const before = read(root, PROPOSAL_FILE)
    const { result } = await moveTo(root, to)
    if ((EDGES[from] as string[]).includes(to)) {
      expect(result, JSON.stringify(result)).toMatchObject({ outcome: 'ok', task: { state: { status: to } } })
      return
    }
    const moves = (EDGES[from] as string[]).join(', ')
    const named = moves === '' ? 'is final' : `can move to ${moves}`
    expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining(`pr-0001-aa is ${from}`) })
    expect(result?.message).toContain(`cannot move to ${to}`)
    expect(result?.message).toContain(named)
    expect(read(root, PROPOSAL_FILE)).toBe(before)
  })

  test('to the status it has changes nothing, whatever the table says', async () => {
    const root = repoOf({ [PROPOSAL_FILE]: proposal('superseded') })
    const { result } = await onProposal(root, 'setStatus', { status: 'superseded' })
    expect(result).toMatchObject({ outcome: 'ok', files: [], items: [] })
  })

  test('a move to proposed still waits for the Features part to be right', async () => {
    const bad = proposal('draft').replace('- **F1 Trust prompt**', 'stray\n- **F1 Trust prompt**')
    const root = repoOf({ [PROPOSAL_FILE]: bad })
    const { result } = await onProposal(root, 'setStatus', { status: 'proposed' })
    expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('cannot move to proposed') })
  })
})

describe('accept', () => {
  test('writes status accepted and by: human:<date of actor.at>, and no other line changes', async () => {
    const root = repoOf({ [PROPOSAL_FILE]: proposal('proposed') })
    const before = read(root, PROPOSAL_FILE)
    const { result } = await onProposal(root, 'accept', {}, { kind: 'person', at: '2031-02-03' })
    expect(result).toMatchObject({
      outcome: 'ok',
      files: [`${EPICS}/${PROPOSAL_FILE}`],
      task: { state: { status: 'accepted', category: 'done' }, fields: { by: 'human:2031-02-03' } },
      items: [{ from: { status: 'proposed' }, to: { status: 'accepted' } }],
    })
    const accepted = 'status: accepted\nby: human:2031-02-03\n'
    expect(read(root, PROPOSAL_FILE)).toBe(before.replace('status: proposed\n', accepted))
  })

  test('setStatus accepted gives the same file as accept', async () => {
    const viaAccept = repoOf({ [PROPOSAL_FILE]: proposal('proposed') })
    const viaStatus = repoOf({ [PROPOSAL_FILE]: proposal('proposed') })
    await onProposal(viaAccept, 'accept')
    const { result } = await onProposal(viaStatus, 'setStatus', { status: 'accepted' })
    expect(result?.outcome).toBe('ok')
    expect(read(viaStatus, PROPOSAL_FILE)).toBe(read(viaAccept, PROPOSAL_FILE))
    expect(read(viaStatus, PROPOSAL_FILE)).toContain(`by: human:${TODAY}\n`)
  })

  test('takes the date from the actor and has none to take without one', async () => {
    const root = repoOf({ [PROPOSAL_FILE]: proposal('proposed') })
    const before = read(root, PROPOSAL_FILE)
    for (const actor of [null, { kind: 'person' }, { kind: 'person', at: 'today' }]) {
      const { result } = await onProposal(root, 'accept', {}, actor)
      expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('actor.at') })
    }
    expect(read(root, PROPOSAL_FILE)).toBe(before)
  })

  test('takes no argument', async () => {
    const root = repoOf({ [PROPOSAL_FILE]: proposal('proposed') })
    const { result } = await onProposal(root, 'accept', { by: 'someone' })
    expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('unknown argument "by"') })
  })

  test('is for a proposal: a task and a question are refused in words that say so', async () => {
    const root = repoOf({ [TASK_FILE]: doc('task', 'ts-0001-aa', 'ready'), [QUESTION_ONE]: question() })
    const onTask = await send(root, TASK_FILE, 'ts-0001-aa', 'accept')
    expect(onTask.result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('is a task') })
    expect((await onQuestion(root, 'accept')).result).toMatchObject({ outcome: 'refused' })
  })

  describe('while a question the proposal includes is open', () => {
    const files = (one = 'open', two = 'open') => ({
      [PROPOSAL_FILE]: proposal('proposed', '', links(['includes', 'qu-0001-aa'], ['includes', 'qu-0002-bb'])),
      [QUESTION_ONE]: question(one, one === 'resolved' ? 'resolved_by: human:2026-06-01\n' : ''),
      [QUESTION_TWO]: doc('question', 'qu-0002-bb', two, two === 'resolved' ? 'resolved_by: ts-0001-aa\n' : ''),
    })

    test('is refused naming each open question, and the file is untouched', async () => {
      const root = repoOf(files())
      const before = read(root, PROPOSAL_FILE)
      for (const op of ['accept', 'setStatus']) {
        const { result } = await onProposal(root, op, op === 'accept' ? {} : { status: 'accepted' })
        expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('qu-0001-aa') })
        expect(result?.message).toContain('qu-0002-bb')
        expect(result?.message).toContain('open')
      }
      expect(read(root, PROPOSAL_FILE)).toBe(before)
    })

    test('names only the one that is still open', async () => {
      const root = repoOf(files('dropped', 'open'))
      const { result } = await onProposal(root, 'accept')
      expect(result?.message).toContain('qu-0002-bb')
      expect(result?.message).not.toContain('qu-0001-aa')
    })

    test('works once each is resolved or dropped', async () => {
      const root = repoOf(files('dropped', 'resolved'))
      const { result } = await onProposal(root, 'accept')
      expect(result).toMatchObject({ outcome: 'ok', task: { state: { status: 'accepted' } } })
    })

    test('does not look at a question the proposal does not include', async () => {
      const root = repoOf({ [PROPOSAL_FILE]: proposal('proposed'), [QUESTION_ONE]: question() })
      expect((await onProposal(root, 'accept')).result?.outcome).toBe('ok')
    })
  })

  test('a proposal hand-edited to accepted with no by: lists, and nothing is invalid', async () => {
    const root = repoOf({ [PROPOSAL_FILE]: proposal('accepted') })
    const listed = await call(root, 'list')
    expect(listed.invalid).toEqual([])
    const found = listed.result?.tasks.find((candidate: Row) => candidate.id === 'pr-0001-aa')
    expect(found).toMatchObject({ state: { status: 'accepted', category: 'done' } })
    expect(found.fields).toBeUndefined()
  })

  test('an accepted proposal with no by: moves to superseded and gains no by:', async () => {
    const root = repoOf({ [PROPOSAL_FILE]: proposal('accepted') })
    const before = read(root, PROPOSAL_FILE)
    const { result } = await onProposal(root, 'setStatus', { status: 'superseded' })
    expect(result?.outcome).toBe('ok')
    expect(read(root, PROPOSAL_FILE)).toBe(before.replace('status: accepted', 'status: superseded'))
  })

  test('replaces a by: that is already there, and keeps it when the proposal is superseded', async () => {
    const root = repoOf({ [PROPOSAL_FILE]: proposal('proposed', 'by: human:2020-01-01\n') })
    const before = read(root, PROPOSAL_FILE)
    await onProposal(root, 'accept')
    const accepted = read(root, PROPOSAL_FILE)
    expect(accepted).toBe(before.replace('status: proposed', 'status: accepted').replace('2020-01-01', TODAY))
    await onProposal(root, 'setStatus', { status: 'superseded' })
    expect(read(root, PROPOSAL_FILE)).toBe(accepted.replace('status: accepted', 'status: superseded'))
  })
})

describe('defer', () => {
  test('writes the trigger as a trigger: line and moves a proposed proposal to deferred', async () => {
    const root = repoOf({ [PROPOSAL_FILE]: proposal('proposed') })
    const before = read(root, PROPOSAL_FILE)
    const trigger = 'when the "pack lock" lands: ask again'
    const { result } = await onProposal(root, 'defer', { trigger })
    expect(result).toMatchObject({
      outcome: 'ok',
      task: { state: { status: 'deferred', category: 'backlog' }, fields: { trigger } },
    })
    expect(read(root, PROPOSAL_FILE)).toBe(
      before.replace('status: proposed\n', `status: deferred\ntrigger: ${JSON.stringify(trigger)}\n`),
    )
  })

  test.each([
    ['none', {}],
    ['an empty one', { trigger: '' }],
    ['one of spaces', { trigger: '   ' }],
    ['one that is not text', { trigger: 5 }],
    ['one with a line break', { trigger: 'a\nb' }],
    ['one with a space at an end', { trigger: ' later' }],
  ])('with %s is refused naming the trigger, and writes nothing', async (_name, args) => {
    const root = repoOf({ [PROPOSAL_FILE]: proposal('proposed') })
    const before = read(root, PROPOSAL_FILE)
    const { result } = await onProposal(root, 'defer', args)
    expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('trigger') })
    expect(read(root, PROPOSAL_FILE)).toBe(before)
  })

  test('a deferred proposal goes back to proposed by setStatus, and keeps its trigger line', async () => {
    const root = repoOf({ [PROPOSAL_FILE]: proposal('deferred', 'trigger: "later"\n') })
    const before = read(root, PROPOSAL_FILE)
    const { result } = await onProposal(root, 'setStatus', { status: 'proposed' })
    expect(result).toMatchObject({
      outcome: 'ok',
      task: { state: { status: 'proposed' }, fields: { trigger: 'later' } },
    })
    expect(read(root, PROPOSAL_FILE)).toBe(before.replace('status: deferred', 'status: proposed'))
  })

  test('setStatus deferred is refused, and the message points to defer, which takes the trigger', async () => {
    const root = repoOf({ [PROPOSAL_FILE]: proposal('proposed') })
    const before = read(root, PROPOSAL_FILE)
    const { result } = await onProposal(root, 'setStatus', { status: 'deferred' })
    expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('use defer') })
    expect(read(root, PROPOSAL_FILE)).toBe(before)
  })
})

describe('reject', () => {
  test('with a reason writes a reason: line', async () => {
    const root = repoOf({ [PROPOSAL_FILE]: proposal('proposed') })
    const before = read(root, PROPOSAL_FILE)
    const { result } = await onProposal(root, 'reject', { reason: 'Out of scope: for later' })
    expect(result).toMatchObject({
      outcome: 'ok',
      task: { state: { status: 'rejected', category: 'dropped' }, fields: { reason: 'Out of scope: for later' } },
    })
    expect(read(root, PROPOSAL_FILE)).toBe(
      before.replace('status: proposed\n', 'status: rejected\nreason: "Out of scope: for later"\n'),
    )
  })

  test('without one changes the status line and nothing else, as setStatus rejected does', async () => {
    const viaOp = repoOf({ [PROPOSAL_FILE]: proposal('draft') })
    const viaStatus = repoOf({ [PROPOSAL_FILE]: proposal('draft') })
    const before = read(viaOp, PROPOSAL_FILE)
    await onProposal(viaOp, 'reject')
    await onProposal(viaStatus, 'setStatus', { status: 'rejected' })
    expect(read(viaOp, PROPOSAL_FILE)).toBe(before.replace('status: draft', 'status: rejected'))
    expect(read(viaStatus, PROPOSAL_FILE)).toBe(read(viaOp, PROPOSAL_FILE))
  })

  test.each([{ reason: '' }, { reason: 'a\nb' }, { reason: 4 }])('with the reason %j is refused', async (args) => {
    const root = repoOf({ [PROPOSAL_FILE]: proposal('proposed') })
    const before = read(root, PROPOSAL_FILE)
    const { result } = await onProposal(root, 'reject', args)
    expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('reason') })
    expect(read(root, PROPOSAL_FILE)).toBe(before)
  })

  test('a rejected proposal returns to draft, and a later reason replaces the old one', async () => {
    const root = repoOf({ [PROPOSAL_FILE]: proposal('proposed') })
    await onProposal(root, 'reject', { reason: 'first' })
    await onProposal(root, 'setStatus', { status: 'draft' })
    await onProposal(root, 'setStatus', { status: 'proposed' })
    await onProposal(root, 'reject', { reason: 'second' })
    const text = read(root, PROPOSAL_FILE)
    expect(text).toContain('status: rejected\nreason: "second"\n')
    expect(text.match(/^reason:/gm)).toHaveLength(1)
  })
})

describe('resolve', () => {
  const resolve = (root: string, args: Row, actor: Row | null = person) => onQuestion(root, 'resolve', args, actor)

  test('with no by writes resolved_by: human:<date> and the text under ## Decision', async () => {
    const root = repoOf({ [QUESTION_ONE]: question() })
    const { result } = await resolve(root, { text: 'Use the file store.' })
    expect(result).toMatchObject({
      outcome: 'ok',
      files: [`${EPICS}/${QUESTION_ONE}`],
      task: { state: { status: 'resolved', category: 'done' }, fields: { resolved_by: `human:${TODAY}` } },
      items: [{ from: { status: 'open' }, to: { status: 'resolved' } }],
    })
    expect(read(root, QUESTION_ONE)).toBe(
      resolvedQuestion(`human:${TODAY}`, '## Question\n\n## Decision\n\nUse the file store.\n'),
    )
  })

  test('with by names the item that settled it, which must exist', async () => {
    const root = repoOf({ [QUESTION_ONE]: question(), [TASK_FILE]: doc('task', 'ts-0001-aa', 'ready') })
    const { result } = await resolve(root, { by: 'ts-0001-aa', text: 'Built in ts-0001-aa.' })
    expect(result).toMatchObject({ outcome: 'ok', task: { fields: { resolved_by: 'ts-0001-aa' } } })
    expect(read(root, QUESTION_ONE)).toContain('status: resolved\nresolved_by: ts-0001-aa\n')
    expect((await call(root, 'list')).invalid).toEqual([])
  })

  test('with a by that names no item is refused, and so is one that names the question itself', async () => {
    const root = repoOf({ [QUESTION_ONE]: question() })
    const before = read(root, QUESTION_ONE)
    const missing = await resolve(root, { by: 'ts-0099-zz', text: 'x' })
    expect(missing.result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('no item ts-0099-zz') })
    const itself = await resolve(root, { by: 'qu-0001-aa', text: 'x' })
    expect(itself.result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('itself') })
    const shape = await resolve(root, { by: 'someone', text: 'x' })
    expect(shape.result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('by must') })
    expect(read(root, QUESTION_ONE)).toBe(before)
  })

  test('the question reads back with no task.invalid', async () => {
    const root = repoOf({ [QUESTION_ONE]: question() })
    await resolve(root, { text: 'Done.' })
    const listed = await call(root, 'list')
    expect(listed.invalid).toEqual([])
    expect(listed.result?.tasks.map((task: Row) => task.id)).toContain('qu-0001-aa')
  })

  test('makes the ## Decision section at the end of the body when the body has none', async () => {
    const root = repoOf({ [QUESTION_ONE]: question('open', '', '## Question\n\nWhy?\n') })
    await resolve(root, { text: 'Because.' })
    expect(read(root, QUESTION_ONE)).toBe(
      resolvedQuestion(`human:${TODAY}`, '## Question\n\nWhy?\n\n## Decision\n\nBecause.\n'),
    )
  })

  test('makes it in an empty body, too', async () => {
    const root = repoOf({ [QUESTION_ONE]: question('open', '', '') })
    await resolve(root, { text: 'Because.' })
    expect(read(root, QUESTION_ONE)).toBe(
      resolvedQuestion(`human:${TODAY}`, '## Decision\n\nBecause.\n'),
    )
  })

  test('adds to a ## Decision section that has text, and keeps the section after it', async () => {
    const body = '## Question\n\n## Decision\n\nDraft note.\n\n## Notes\n\nKept.\n'
    const root = repoOf({ [QUESTION_ONE]: question('open', '', body) })
    await resolve(root, { text: 'Final: yes.\nWith a second line.' })
    expect(read(root, QUESTION_ONE)).toBe(
      resolvedQuestion(
        `human:${TODAY}`,
        '## Question\n\n## Decision\n\nDraft note.\n\nFinal: yes.\nWith a second line.\n\n## Notes\n\nKept.\n',
      ),
    )
  })

  test('keeps the line endings of a file that has CRLF', async () => {
    const text = question().replaceAll('\n', '\r\n')
    const root = repoOf({ [QUESTION_ONE]: text })
    const { result } = await resolve(root, { text: 'One.\nTwo.' })
    expect(result?.outcome, JSON.stringify(result)).toBe('ok')
    const after = read(root, QUESTION_ONE)
    expect(after).toBe(
      resolvedQuestion(`human:${TODAY}`, '## Question\n\n## Decision\n\nOne.\nTwo.\n').replaceAll(
        '\n',
        '\r\n',
      ),
    )
  })

  test.each([
    ['none', {}],
    ['an empty one', { text: '' }],
    ['one of spaces', { text: ' \n ' }],
    ['one that is not text', { text: 7 }],
    ['one with a control character', { text: 'a\u0007b' }],
  ])('with %s for the text is refused, and writes nothing', async (_name, args) => {
    const root = repoOf({ [QUESTION_ONE]: question() })
    const before = read(root, QUESTION_ONE)
    const { result } = await resolve(root, args)
    expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('text') })
    expect(read(root, QUESTION_ONE)).toBe(before)
  })

  test.each([
    ['resolved', 'resolved_by: human:2026-01-01\n', 'already resolved'],
    ['dropped', '', 'dropped'],
  ])('a question that is %s is refused', async (status, extra, said) => {
    const root = repoOf({ [QUESTION_ONE]: question(status, extra) })
    const before = read(root, QUESTION_ONE)
    const { result } = await resolve(root, { text: 'x' })
    expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining(said) })
    expect(read(root, QUESTION_ONE)).toBe(before)
  })

  test('is for a question: a proposal is refused in words that say so', async () => {
    const root = repoOf({ [PROPOSAL_FILE]: proposal('proposed') })
    const { result } = await onProposal(root, 'resolve', { text: 'x' })
    expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('is a proposal') })
  })
})

describe('reid rewrites the item id that a question was resolved by', () => {
  test('and changes only that line of the question, and the question still reads', async () => {
    const root = gitRepoOf({
      [QUESTION_ONE]: question('resolved', 'resolved_by: ts-0001-aa\n'),
      [TASK_FILE]: doc('task', 'ts-0001-aa', 'done'),
    })
    const before = read(root, QUESTION_ONE)
    const { result } = await call(root, 'apply', { op: 'reid', id: 'ts-0001-aa', args: { seed: 1 } })
    expect(result?.outcome, JSON.stringify(result)).toBe('ok')
    const newId = result?.task.id as string
    expect(newId).not.toBe('ts-0001-aa')
    expect(read(root, QUESTION_ONE)).toBe(before.replace('resolved_by: ts-0001-aa', `resolved_by: ${newId}`))
    const listed = await call(root, 'list')
    expect(listed.invalid).toEqual([])
    expect(listed.result?.tasks.find((task: Row) => task.id === 'qu-0001-aa').fields.resolved_by).toBe(newId)
    expect(result?.items.map((item: Row) => item.key)).toContain('plan:qu-0001-aa')
  })

  test('a question resolved by a person is not touched', async () => {
    const root = gitRepoOf({
      [QUESTION_ONE]: question('resolved', 'resolved_by: human:2026-06-01\n'),
      [TASK_FILE]: doc('task', 'ts-0001-aa', 'done'),
    })
    const before = read(root, QUESTION_ONE)
    await call(root, 'apply', { op: 'reid', id: 'ts-0001-aa', args: { seed: 1 } })
    expect(read(root, QUESTION_ONE)).toBe(before)
  })

  test('a quoted resolved_by is rewritten in its quotes', async () => {
    const root = gitRepoOf({
      [QUESTION_ONE]: question('resolved', 'resolved_by: "ts-0001-aa"  # why\n'),
      [TASK_FILE]: doc('task', 'ts-0001-aa', 'done'),
    })
    const { result } = await call(root, 'apply', { op: 'reid', id: 'ts-0001-aa', args: { seed: 1 } })
    expect(read(root, QUESTION_ONE)).toContain(`resolved_by: "${result?.task.id}"  # why\n`)
  })
})

/**
 * The provider's wall must agree with the daemon's `person_only_request` (host/src/auth.rs) on which writes are
 * person-only. Each row is a write and whether the daemon reads it as person-only.
 */
describe('an agent actor, on every write the daemon reads as person-only and on those it does not', () => {
  const PROPOSAL_ROWS: Array<[string, string, Row, boolean]> = [
    ['accept', 'proposed', {}, true],
    ['defer', 'proposed', { trigger: 'later' }, true],
    ['reject', 'proposed', {}, true],
    ['setStatus', 'proposed', { status: 'accepted' }, true],
    ['setStatus', 'proposed', { status: 'deferred' }, true],
    ['setStatus', 'draft', { status: 'rejected' }, true],
    ['setStatus', 'deferred', { status: 'proposed' }, true],
    ['setStatus', 'draft', { status: 'proposed' }, false],
    ['setStatus', 'proposed', { status: 'draft' }, false],
    ['setStatus', 'accepted', { status: 'superseded' }, false],
    ['setStatus', 'rejected', { status: 'draft' }, false],
    ['claim', 'proposed', { claimant: 'me' }, false],
  ]

  test.each(PROPOSAL_ROWS)('%s on a %s proposal with %j: person-only is %s', async (op, status, args, only) => {
    const root = repoOf({ [PROPOSAL_FILE]: proposal(status) })
    const before = read(root, PROPOSAL_FILE)
    const { result } = await onProposal(root, op, args, agent)
    if (only) {
      expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('only a person') })
      expect(result?.message).toContain('R-7')
      expect(read(root, PROPOSAL_FILE)).toBe(before)
    } else {
      expect(result?.outcome, JSON.stringify(result)).toBe('ok')
    }
  })

  test('a person-only write to the status it has is refused too, and nothing is written', async () => {
    const root = repoOf({ [PROPOSAL_FILE]: proposal('accepted') })
    const { result } = await onProposal(root, 'setStatus', { status: 'accepted' }, agent)
    expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('only a person') })
  })

  test('resolve is refused to an agent, and a question moves to open or dropped for one', async () => {
    const root = repoOf({ [QUESTION_ONE]: question() })
    const before = read(root, QUESTION_ONE)
    const resolved = await onQuestion(root, 'resolve', { text: 'x' }, agent)
    expect(resolved.result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('only a person') })
    expect(read(root, QUESTION_ONE)).toBe(before)
    const dropped = await onQuestion(root, 'setStatus', { status: 'dropped' }, agent)
    expect(dropped.result?.outcome).toBe('ok')
  })

  test('accept, defer and reject on a question: an agent meets the wall, a person the kind', async () => {
    const root = repoOf({ [QUESTION_ONE]: question() })
    for (const [op, args] of [['accept', {}], ['defer', { trigger: 'x' }], ['reject', {}]] as Array<[string, Row]>) {
      const byAgent = await onQuestion(root, op, args, agent)
      expect(byAgent.result?.message, op).toContain('only a person')
      const byPerson = await onQuestion(root, op, args, person)
      expect(byPerson.result?.message, op).toContain('is a question')
    }
  })

  test('an agent with no run is named as an agent', async () => {
    const root = repoOf({ [PROPOSAL_FILE]: proposal('proposed') })
    const { result } = await onProposal(root, 'accept', {}, { kind: 'agent', at: TODAY })
    expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('agent') })
  })

  test('the refusal comes before a conflict, so a stale version does not hide it', async () => {
    const root = repoOf({ [PROPOSAL_FILE]: proposal('proposed') })
    const { result } = await call(root, 'apply', {
      op: 'accept',
      id: 'pr-0001-aa',
      args: {},
      version: 'stale',
      actor: agent,
    })
    expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('only a person') })
  })
})

describe('a person with no actor', () => {
  test('is read as a person: a write that needs no date is made', async () => {
    const root = repoOf({ [PROPOSAL_FILE]: proposal('proposed') })
    const { result } = await onProposal(root, 'reject', {}, null)
    expect(result).toMatchObject({ outcome: 'ok', task: { state: { status: 'rejected' } } })
  })
})
