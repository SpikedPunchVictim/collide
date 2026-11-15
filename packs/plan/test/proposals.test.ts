import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, test } from 'vitest'
import { call, commitAll, copyFixture, EPIC_FILE, fileVersion, fixture, git, makeRepo } from './helpers.ts'

const EPICS = '.collide/planning/epics'
const FOLDER = 'ep-001-aa-alpha'
const PROPOSAL_PATH = `${FOLDER}/proposals/pr-0003-km-features.md`

const proposal = (status: string, body: string, id = 'pr-0003-km'): string =>
  `---\nid: ${id}\nkind: proposal\nstatus: ${status}\ntitle: "Made-up proposal"\nowners: []\nlinks: []\n---\n\n${body}`

const GOOD_PART = '## Features\n\n### Packs\n- **F1 Trust prompt**: a repo pack asks first.\n- **F2 Pack lock**: pins each pack.\n'

/** Two problems, on the 7th and 10th lines of the body, and a body that starts on line 9 of the file (see `proposal`). */
const BAD_PART = '## Context\n\nText.\n\n## Features\n\nstray text\n### Packs\n- **F1 A**: b\n- **F1 B**: c\n'

const repoWith = (text: string): string => makeRepo({ [`${FOLDER}/epic.md`]: EPIC_FILE, [PROPOSAL_PATH]: text })

const pathOf = (root: string): string => join(root, EPICS, PROPOSAL_PATH)
const taskOf = (result: Record<string, any> | undefined, id: string): Record<string, any> => (result?.tasks as Array<Record<string, any>>).find((task) => task.id === id)!

const setStatus = (root: string, status: string, args: Record<string, unknown> = { status }) =>
  call(root, 'apply', { op: 'setStatus', id: 'pr-0003-km', args, version: fileVersion(pathOf(root)) })

describe("a proposal in proposals/ over 0.10's example", () => {
  test('lists as kind proposal with its status and category, part of the epic that holds the folder', async () => {
    const { result, invalid } = await call(fixture('basic'), 'list')
    expect(invalid).toEqual([])
    expect(taskOf(result, 'pr-0001-aa')).toMatchObject({
      key: 'plan:pr-0001-aa',
      kind: 'proposal',
      state: { status: 'draft', category: 'active' },
      path: `${EPICS}/ep-001-aa-alpha/proposals/pr-0001-aa-idea.md`,
      links: [{ type: 'part-of', to: 'ep-001-aa' }],
    })
  })

  test('lists F1 and F2 in section Packs and F3 with section "", in list and in get, and no problems', async () => {
    const expected = [
      { id: 'F1', title: 'Trust prompt for repo packs', section: 'Packs', summary: 'a repo pack asks before its first run. A continuation line is indented by two spaces.' },
      { id: 'F2', title: 'Pack lock', section: 'Packs', summary: '`packs.lock` pins each pack to a tree.' },
      { id: 'F3', title: 'Menu bar badge', section: '', summary: 'the Queue count in the menu bar.' },
    ]
    const listed = taskOf((await call(fixture('basic'), 'list')).result, 'pr-0001-aa')
    expect(listed.features).toEqual(expected)
    expect(listed).not.toHaveProperty('featureProblems')
    expect(listed).not.toHaveProperty('body')
    const got = (await call(fixture('basic'), 'get', { id: 'pr-0001-aa' })).result?.task
    expect(got.features).toEqual(expected)
    expect(got.body).toContain('## Features')
  })

  test('an item that is not a proposal carries neither features nor problems, even with a Features part in its body', async () => {
    const root = makeRepo({ [`${FOLDER}/epic.md`]: EPIC_FILE, [`${FOLDER}/tasks/ts-0001-aa-x.md`]: proposal('ready', 'stray\n\n## Features\n\nnot read\n', 'ts-0001-aa').replace('kind: proposal', 'kind: task') })
    const task = taskOf((await call(root, 'list')).result, 'ts-0001-aa')
    expect(task).not.toHaveProperty('features')
    expect(task).not.toHaveProperty('featureProblems')
  })
})

describe('the six statuses and their categories', () => {
  test.each([
    ['draft', 'active'],
    ['proposed', 'review'],
    ['accepted', 'done'],
    ['deferred', 'backlog'],
    ['rejected', 'dropped'],
    ['superseded', 'dropped'],
  ])('%s is category %s', async (status, category) => {
    const { result, invalid } = await call(repoWith(proposal(status, GOOD_PART)), 'list')
    expect(invalid).toEqual([])
    expect(taskOf(result, 'pr-0003-km').state).toEqual({ status, category })
  })

  test.each(['ready', 'active', 'review', 'done', 'dropped', 'open', ''])('a proposal with status "%s" is a task.invalid naming the six', async (status) => {
    const { result, invalid } = await call(repoWith(proposal(status === '' ? '""' : status, GOOD_PART)), 'list')
    expect(result?.tasks.map((task: { id: string }) => task.id)).toEqual(['ep-001-aa'])
    expect(invalid).toEqual([{ path: `${EPICS}/${PROPOSAL_PATH}`, checkout: null, reason: expect.stringContaining('draft, proposed, accepted, deferred, rejected, superseded') }])
  })

  test('a task with a proposal status is still a task.invalid', async () => {
    const root = makeRepo({ [`${FOLDER}/epic.md`]: EPIC_FILE, [`${FOLDER}/tasks/ts-0001-aa-x.md`]: proposal('draft', '', 'ts-0001-aa').replace('kind: proposal', 'kind: task') })
    expect((await call(root, 'list')).invalid).toEqual([expect.objectContaining({ reason: expect.stringContaining('status must be one of ready, active, review, done, dropped') })])
  })
})

describe('feature problems in a draft', () => {
  test('are reported on the item, with the lines of the file, and the proposal is not a task.invalid', async () => {
    const root = repoWith(proposal('draft', BAD_PART))
    const { result, invalid } = await call(root, 'list')
    expect(invalid).toEqual([])
    const item = taskOf(result, 'pr-0003-km')
    // The frontmatter block is lines 1 to 8, so the body starts on line 9, a blank one: `stray text` is line 16.
    expect(item.featureProblems).toEqual([
      { line: 16, reason: expect.stringContaining('neither an entry nor a continuation') },
      { line: 19, reason: expect.stringContaining('F1 is already used on line 18') },
    ])
    expect(item.features.map((feature: { id: string }) => feature.id)).toEqual(['F1'])
    expect(item.state).toEqual({ status: 'draft', category: 'active' })
  })

  test('are reported for a proposal in any status, not only a draft', async () => {
    const root = repoWith(proposal('accepted', BAD_PART))
    const { result, invalid } = await call(root, 'list')
    expect(invalid).toEqual([])
    expect(taskOf(result, 'pr-0003-km').featureProblems).toHaveLength(2)
  })

  test('count the lines of a file with CRLF endings the same way', async () => {
    const root = repoWith(proposal('draft', BAD_PART).replaceAll('\n', '\r\n'))
    expect(taskOf((await call(root, 'list')).result, 'pr-0003-km').featureProblems.map((problem: { line: number }) => problem.line)).toEqual([16, 19])
  })

  test('a proposal with no Features part has no features and no problems', async () => {
    const item = taskOf((await call(repoWith(proposal('draft', '## Context\n\nText.\n')), 'list')).result, 'pr-0003-km')
    expect(item).not.toHaveProperty('features')
    expect(item).not.toHaveProperty('featureProblems')
  })
})

describe('setStatus', () => {
  test('to proposed is refused while the part has a problem: one message with every problem and its line, and the file is unchanged', async () => {
    const root = repoWith(proposal('draft', BAD_PART))
    const before = readFileSync(pathOf(root), 'utf8')
    const { result } = await setStatus(root, 'proposed')
    expect(result?.outcome).toBe('refused')
    expect(result?.message).toContain('pr-0003-km cannot move to proposed')
    expect(result?.message).toContain('2 problems')
    expect(result?.message).toMatch(/line 16: .*neither an entry nor a continuation/)
    expect(result?.message).toMatch(/line 19: .*F1 is already used on line 18/)
    expect(readFileSync(pathOf(root), 'utf8')).toBe(before)
  })

  test('to proposed is refused from deferred too, and moving to any other status is allowed with the same problems', async () => {
    const root = repoWith(proposal('deferred', BAD_PART))
    expect((await setStatus(root, 'proposed')).result?.outcome).toBe('refused')
    expect((await setStatus(root, 'rejected')).result).toMatchObject({ outcome: 'ok', task: { state: { status: 'rejected', category: 'dropped' } } })
  })

  test('to proposed succeeds once the part is fixed, and changes only the status line, as git diff shows', async () => {
    const root = repoWith(proposal('draft', BAD_PART))
    git(root, 'init', '--quiet')
    commitAll(root, 'fixture')
    expect((await setStatus(root, 'proposed')).result?.outcome).toBe('refused')

    const fixed = readFileSync(pathOf(root), 'utf8').replace('stray text\n', '').replace('- **F1 B**: c', '- **F2 B**: c')
    writeFileSync(pathOf(root), fixed)
    commitAll(root, 'fixed the part')

    const { result } = await setStatus(root, 'proposed')
    expect(result).toMatchObject({ outcome: 'ok', task: { state: { status: 'proposed', category: 'review' } }, items: [{ from: { status: 'draft', category: 'active' }, to: { status: 'proposed', category: 'review' } }] })
    expect(result?.task.features.map((feature: { id: string }) => feature.id)).toEqual(['F1', 'F2'])
    const file = `${EPICS}/${PROPOSAL_PATH}`
    expect(git(root, 'diff', '--numstat')).toBe(`1\t1\t${file}\n`)
    expect(git(root, 'diff', '-U0').split('\n').filter((line) => /^[-+]/.test(line) && !/^(---|\+\+\+)/.test(line))).toEqual(['-status: draft', '+status: proposed'])
  })

  test('to proposed succeeds for a proposal with no Features part', async () => {
    const root = repoWith(proposal('draft', '## Decision\n'))
    expect((await setStatus(root, 'proposed')).result).toMatchObject({ outcome: 'ok', task: { state: { status: 'proposed', category: 'review' } } })
  })

  test('to proposed when it is proposed already writes nothing, whatever the part says', async () => {
    const root = repoWith(proposal('proposed', BAD_PART))
    expect((await setStatus(root, 'proposed')).result).toMatchObject({ outcome: 'ok', files: [], items: [] })
  })

  test.each(['draft', 'proposed', 'accepted', 'deferred', 'rejected', 'superseded'])('moves among the six: to %s', async (to) => {
    const from = to === 'draft' ? 'proposed' : 'draft'
    const root = repoWith(proposal(from, GOOD_PART))
    const before = readFileSync(pathOf(root), 'utf8')
    const { result } = await setStatus(root, to)
    expect(result).toMatchObject({ outcome: 'ok', task: { state: { status: to } } })
    expect(readFileSync(pathOf(root), 'utf8')).toBe(before.replace(`status: ${from}`, `status: ${to}`))
  })

  test('refuses a status that is not one of the six, and a category, and writes nothing', async () => {
    const root = repoWith(proposal('draft', GOOD_PART))
    const before = readFileSync(pathOf(root), 'utf8')
    expect((await setStatus(root, 'active')).result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('draft, proposed, accepted, deferred, rejected, superseded') })
    expect((await setStatus(root, 'doing')).result?.outcome).toBe('refused')
    expect((await setStatus(root, '', { category: 'review' })).result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('give status') })
    expect(readFileSync(pathOf(root), 'utf8')).toBe(before)
  })

  test('refuses a proposal status on a task', async () => {
    const root = makeRepo({ [`${FOLDER}/epic.md`]: EPIC_FILE, [`${FOLDER}/tasks/ts-0001-aa-x.md`]: proposal('ready', '', 'ts-0001-aa').replace('kind: proposal', 'kind: task') })
    const path = join(root, EPICS, FOLDER, 'tasks/ts-0001-aa-x.md')
    const { result } = await call(root, 'apply', { op: 'setStatus', id: 'ts-0001-aa', args: { status: 'proposed' }, version: fileVersion(path) })
    expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('ready, active, review, done, dropped') })
  })
})

describe('create of a proposal', () => {
  const EPIC = 'ep-001-aa'
  const created = async (root: string, args: Record<string, unknown>) => (await call(root, 'apply', { op: 'create', args: { kind: 'proposal', epic: EPIC, seed: 1, ...args } })).result

  function repoWithGit(files: Record<string, string> = {}): string {
    const root = makeRepo({ [`${FOLDER}/epic.md`]: EPIC_FILE, ...files })
    git(root, 'init', '--quiet')
    commitAll(root, 'fixture')
    return root
  }

  test('writes pr-NNNN-xx-<slug>.md into the epic\'s proposals/ folder as a draft, and it reads back', async () => {
    const root = repoWithGit()
    const result = await created(root, { title: 'Features in proposals' })
    const path = `${EPICS}/${FOLDER}/proposals/pr-0001-ra-features-in-proposals.md`
    expect(result).toMatchObject({
      outcome: 'ok',
      files: [path],
      task: { id: 'pr-0001-ra', kind: 'proposal', state: { status: 'draft', category: 'active' }, path, links: [{ type: 'part-of', to: EPIC }], owners: [] },
      items: [{ key: 'plan:pr-0001-ra', from: null, to: { status: 'draft', category: 'active' } }],
    })
    expect(result?.task).not.toHaveProperty('featureProblems')
    expect(git(root, 'status', '--porcelain', '--untracked-files=all')).toBe(`?? ${path}\n`)
    const listed = await call(root, 'list')
    expect(listed.invalid).toEqual([])
    expect(taskOf(listed.result, 'pr-0001-ra')).toMatchObject({ kind: 'proposal' })
  })

  test('numbers across proposals, in the working tree, and apart from tasks', async () => {
    const root = repoWithGit({
      [`${FOLDER}/proposals/pr-0003-km-features.md`]: proposal('draft', GOOD_PART),
      [`${FOLDER}/tasks/ts-0009-aa-task.md`]: proposal('ready', '', 'ts-0009-aa').replace('kind: proposal', 'kind: task'),
    })
    expect((await created(root, { title: 'Next' }))?.task.id).toMatch(/^pr-0004-[a-z]{2}$/)
    expect((await created(root, { kind: 'task', title: 'A task' }))?.task.id).toMatch(/^ts-0010-[a-z]{2}$/)
  })

  test('counts a proposal that exists only on another branch', async () => {
    const root = repoWithGit()
    git(root, 'checkout', '--quiet', '-b', 'other')
    mkdirSync(join(root, EPICS, FOLDER, 'proposals'), { recursive: true })
    writeFileSync(join(root, EPICS, FOLDER, 'proposals/pr-0007-zz-elsewhere.md'), proposal('draft', GOOD_PART, 'pr-0007-zz'))
    commitAll(root, 'other')
    git(root, 'checkout', '--quiet', '-')
    expect((await created(root, { title: 'After the other branch' }))?.task.id).toMatch(/^pr-0008-[a-z]{2}$/)
  })

  test('a new proposal moves to proposed, because its empty Features part has no problem', async () => {
    const root = repoWithGit()
    const result = await created(root, { title: 'Ready to propose' })
    const path = join(root, result?.task.path as string)
    const moved = await call(root, 'apply', { op: 'setStatus', id: result?.task.id, args: { status: 'proposed' }, version: fileVersion(path) })
    expect(moved.result).toMatchObject({ outcome: 'ok', task: { state: { status: 'proposed' } } })
  })

  test('needs an epic that exists, like a task does', async () => {
    const root = repoWithGit()
    expect(await created(root, { epic: 'ep-009-zz', title: 'Nowhere' })).toMatchObject({ outcome: 'refused', message: expect.stringContaining('no epic ep-009-zz') })
    expect(await created(root, { epic: undefined, title: 'No epic' })).toMatchObject({ outcome: 'refused', message: expect.stringContaining('a proposal needs epic') })
  })
})

describe('reid of a proposal', () => {
  test('gives it a new suffix, keeps its state, and keeps its file in proposals/', async () => {
    const root = copyFixture('basic')
    git(root, 'init', '--quiet')
    commitAll(root, 'fixture')
    const { result } = await call(root, 'apply', { op: 'reid', id: 'pr-0001-aa', args: { seed: 3 } })
    expect(result?.outcome).toBe('ok')
    expect(result?.task.id).toMatch(/^pr-0001-[a-z]{2}$/)
    expect(result?.task.id).not.toBe('pr-0001-aa')
    expect(result?.task.path).toMatch(/\/proposals\/pr-0001-[a-z]{2}-idea\.md$/)
    expect(result?.task.features).toHaveLength(3)
    expect(result?.items[0]).toMatchObject({ from: { status: 'draft', category: 'active' }, to: { status: 'draft', category: 'active' } })
  })
})
