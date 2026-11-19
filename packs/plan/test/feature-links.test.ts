import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, test } from 'vitest'
import { addLinkLine, replaceLinkTarget } from '../src/frontmatter-edit.ts'
import { call, commitAll, EPIC_FILE, fileVersion, git, makeRepo } from './helpers.ts'

const EPICS = '.collide/planning/epics'
const FOLDER = 'ep-001-aa-alpha'
const PROPOSAL = 'pr-0003-km'
const PROPOSAL_FILE = `${FOLDER}/proposals/${PROPOSAL}-features.md`
const TASK_FILE = (id: string): string => `${FOLDER}/tasks/${id}-work.md`

const FEATURES = '## Features\n\n### Packs\n- **F1 Trust prompt**: a repo pack asks first.\n- **F2 Pack lock**: pins each pack.\n'

const proposal = (id: string, status: string, body: string): string => `---\nid: ${id}\nkind: proposal\nstatus: ${status}\ntitle: "Made-up proposal"\nowners: []\nlinks: []\n---\n\n${body}`
const task = (id: string, links = 'links: []\n', status = 'ready'): string => `---\nid: ${id}\nkind: task\nstatus: ${status}\ntitle: "Item ${id}"\nowners: []\n${links}---\n\n## Goal\n\nMade-up.\n`
const link = (type: string, to: string): string => `  - { type: ${type}, to: ${to} }\n`

/** A repository with a proposal that has F1 and F2, a proposal with no features, and the tasks `ts-0001-aa` and `ts-0002-bb`. */
const repo = (extra: Record<string, string> = {}): string =>
  makeRepo({
    [`${FOLDER}/epic.md`]: EPIC_FILE,
    [PROPOSAL_FILE]: proposal(PROPOSAL, 'accepted', FEATURES),
    [`${FOLDER}/proposals/pr-0004-nn-empty.md`]: proposal('pr-0004-nn', 'draft', '## Context\n\nNo features yet.\n'),
    [TASK_FILE('ts-0001-aa')]: task('ts-0001-aa'),
    [TASK_FILE('ts-0002-bb')]: task('ts-0002-bb'),
    ...extra,
  })

const pathOf = (root: string, file: string): string => join(root, EPICS, file)
const read = (root: string, file: string): string => readFileSync(pathOf(root, file), 'utf8')

/** Sends `link` for `ts-0001-aa` at its current version. */
const linkOp = (root: string, args: Record<string, unknown>, id = 'ts-0001-aa') =>
  call(root, 'apply', { op: 'link', id, args, version: fileVersion(pathOf(root, TASK_FILE(id))) })

const storedLinks = (result: Record<string, any> | undefined) => (result?.task.links as Array<{ type: string; to: string }>).filter((candidate) => candidate.type !== 'part-of')

describe('the declaration', () => {
  test('implements is declared outward, not blocking, with a feature target; the other link types declare none', async () => {
    const { result } = await call(repo(), 'initialize', { config: {} })
    expect(result?.links).toEqual([
      { name: 'blocked-by', dir: 'out', blocking: true },
      { name: 'part-of', dir: 'out', blocking: false },
      { name: 'implements', dir: 'out', blocking: false, target: 'feature' },
      { name: 'informs', dir: 'out', blocking: false },
      { name: 'includes', dir: 'out', blocking: false },
      { name: 'supersedes', dir: 'out', blocking: false },
    ])
  })

  test('link takes the five stored link types, and a target with or without a feature', async () => {
    const { result } = await call(repo(), 'initialize', { config: {} })
    const op = (result?.ops as Array<Record<string, any>>).find((candidate) => candidate.name === 'link')
    expect(op?.args.properties.type).toEqual({
      enum: ['blocked-by', 'implements', 'informs', 'includes', 'supersedes'],
    })
    const pattern = new RegExp(op?.args.properties.to.pattern)
    expect(['ts-0002-bb', 'pr-0003-km#F2', 'pr-0003-km#F10'].every((to) => pattern.test(to))).toBe(true)
    expect(['pr-0003-km#F0', 'pr-0003-km#f2', 'pr-0003-km#F2x', 'pr-0003-km#F2#F3', 'two'].some((to) => pattern.test(to))).toBe(false)
  })
})

describe('link implements', () => {
  test('to a feature the proposal has writes the link, changing only the links lines', async () => {
    const root = repo()
    const before = read(root, TASK_FILE('ts-0001-aa'))
    const { result } = await linkOp(root, { to: `${PROPOSAL}#F2`, type: 'implements' })
    expect(result).toMatchObject({ outcome: 'ok', files: [`.collide/planning/epics/${TASK_FILE('ts-0001-aa')}`] })
    expect(storedLinks(result)).toEqual([{ type: 'implements', to: `${PROPOSAL}#F2` }])
    expect(read(root, TASK_FILE('ts-0001-aa'))).toBe(before.replace('links: []\n', `links:\n${link('implements', `${PROPOSAL}#F2`)}`))
  })

  test('is added after a blocked-by entry that is already there, and the item reads back', async () => {
    const text = task('ts-0001-aa', `links:\n${link('blocked-by', 'ts-0002-bb')}`)
    const root = repo({ [TASK_FILE('ts-0001-aa')]: text })
    const { result } = await linkOp(root, { to: `${PROPOSAL}#F1`, type: 'implements' })
    expect(storedLinks(result)).toEqual([
      { type: 'blocked-by', to: 'ts-0002-bb' },
      { type: 'implements', to: `${PROPOSAL}#F1` },
    ])
    expect((await call(root, 'list')).invalid).toEqual([])
  })

  test('a second feature of the same proposal is its own link', async () => {
    const root = repo()
    await linkOp(root, { to: `${PROPOSAL}#F1`, type: 'implements' })
    const { result } = await linkOp(root, { to: `${PROPOSAL}#F2`, type: 'implements' })
    expect(storedLinks(result).map((candidate) => candidate.to)).toEqual([`${PROPOSAL}#F1`, `${PROPOSAL}#F2`])
  })

  test('a link that is already there is ok and writes nothing', async () => {
    const text = task('ts-0001-aa', `links:\n${link('implements', `${PROPOSAL}#F2`)}`)
    const root = repo({ [TASK_FILE('ts-0001-aa')]: text })
    const { result } = await linkOp(root, { to: `${PROPOSAL}#F2`, type: 'implements' })
    expect(result).toMatchObject({ outcome: 'ok', files: [], items: [] })
    expect(read(root, TASK_FILE('ts-0001-aa'))).toBe(text)
  })

  test('to a feature the proposal lacks is refused, naming the features it has, and writes nothing', async () => {
    const root = repo()
    const before = read(root, TASK_FILE('ts-0001-aa'))
    const { result } = await linkOp(root, { to: `${PROPOSAL}#F9`, type: 'implements' })
    expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining(`${PROPOSAL} has no feature F9`) })
    expect(result?.message).toContain('F1 (Trust prompt)')
    expect(result?.message).toContain('F2 (Pack lock)')
    expect(read(root, TASK_FILE('ts-0001-aa'))).toBe(before)
  })

  test('to a proposal that has no features at all is refused, and says it has none', async () => {
    const { result } = await linkOp(repo(), { to: 'pr-0004-nn#F1', type: 'implements' })
    expect(result).toMatchObject({ outcome: 'refused', message: expect.stringMatching(/pr-0004-nn has no feature F1.*has no features/) })
  })

  test('with no feature is refused, naming the features the proposal has', async () => {
    const { result } = await linkOp(repo(), { to: PROPOSAL, type: 'implements' })
    expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('implements links to a feature') })
    expect(result?.message).toContain(`${PROPOSAL}#F<n>`)
    expect(result?.message).toContain('F1 (Trust prompt)')
    expect(result?.message).toContain('F2 (Pack lock)')
  })

  test('to an item that is not a proposal is refused, saying what the item is and what implements takes', async () => {
    const root = repo()
    for (const to of ['ts-0002-bb#F1', 'ts-0002-bb']) {
      const { result } = await linkOp(root, { to, type: 'implements' })
      expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('ts-0002-bb is a task, not a proposal') })
      expect(result?.message).toContain('a feature of a proposal')
    }
  })

  test('to an id no item has is refused', async () => {
    const { result } = await linkOp(repo(), { to: 'pr-0099-zz#F1', type: 'implements' })
    expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('no item pr-0099-zz') })
  })

  test.each(['F0', 'f2', 'F2x', 'F02', 'F'])('with the fragment #%s is refused, and the message says what a fragment is', async (fragment) => {
    const { result } = await linkOp(repo(), { to: `${PROPOSAL}#${fragment}`, type: 'implements' })
    expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining(`#${fragment}`) })
    expect(result?.message).toContain('F and a number from 1')
  })

  test('with two fragments is refused', async () => {
    expect((await linkOp(repo(), { to: `${PROPOSAL}#F1#F2`, type: 'implements' })).result?.outcome).toBe('refused')
  })

  test('from a proposal to its own feature is refused', async () => {
    const root = repo()
    const { result } = await call(root, 'apply', { op: 'link', id: PROPOSAL, args: { to: `${PROPOSAL}#F1`, type: 'implements' }, version: fileVersion(pathOf(root, PROPOSAL_FILE)) })
    expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('cannot implement its own feature') })
  })
})

describe('link blocked-by with a feature', () => {
  test.each([{ args: { to: `${PROPOSAL}#F1` } }, { args: { to: `${PROPOSAL}#F1`, type: 'blocked-by' } }])('%j is refused: a blocking link names an item', async ({ args }) => {
    const root = repo()
    const before = read(root, TASK_FILE('ts-0001-aa'))
    const { result } = await linkOp(root, args)
    expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('blocked-by names an item, not a feature') })
    expect(read(root, TASK_FILE('ts-0001-aa'))).toBe(before)
  })

  test('to an item still writes the link, as before', async () => {
    const { result } = await linkOp(repo(), { to: 'ts-0002-bb' })
    expect(storedLinks(result)).toEqual([{ type: 'blocked-by', to: 'ts-0002-bb' }])
  })
})

describe('a hand-written implements link', () => {
  const logsFor = async (links: string) => {
    const root = repo({ [TASK_FILE('ts-0001-aa')]: task('ts-0001-aa', `links:\n${links}`) })
    const reply = await call(root, 'list')
    return { ...reply, task: (reply.result?.tasks as Array<Record<string, any>>).find((candidate) => candidate.id === 'ts-0001-aa') }
  }

  test('to a feature the proposal has is not a broken link', async () => {
    const { logs, task: listed, invalid } = await logsFor(link('implements', `${PROPOSAL}#F2`))
    expect(logs).toEqual([])
    expect(invalid).toEqual([])
    expect(listed?.links).toContainEqual({ type: 'implements', to: `${PROPOSAL}#F2` })
  })

  test.each([
    ['a feature the proposal lacks', `${PROPOSAL}#F9`],
    ['no feature', PROPOSAL],
    ['a malformed feature', `${PROPOSAL}#f2`],
    ['a proposal with no features', 'pr-0004-nn#F1'],
    ['an item that is not a proposal', 'ts-0002-bb#F1'],
    ['an id no item has', 'pr-0099-zz#F1'],
  ])('to %s lists the task and counts one broken link', async (_name, to) => {
    const { logs, task: listed, invalid } = await logsFor(link('implements', to))
    expect(invalid).toEqual([])
    expect(listed).toBeDefined()
    expect(logs).toEqual(['plan: 5 items, 0 invalid, 0 with an unknown kind, 1 with a broken link'])
  })

  test('two broken links on one task count the task once, and a blocked-by with a feature is broken too', async () => {
    const { logs } = await logsFor(link('implements', `${PROPOSAL}#F9`) + link('blocked-by', `${PROPOSAL}#F1`))
    expect(logs).toEqual(['plan: 5 items, 0 invalid, 0 with an unknown kind, 1 with a broken link'])
  })
})

describe('reid', () => {
  const linking = (id: string, ...links: string[]): string => task(id, `links:\n${links.join('')}`)

  test('of a proposal rewrites pr-<old>#F2 to pr-<new>#F2 in every item that links to it, in every form', async () => {
    const root = repo({
      [TASK_FILE('ts-0001-aa')]: linking('ts-0001-aa', link('implements', `${PROPOSAL}#F2`), link('blocked-by', 'ts-0002-bb')),
      [TASK_FILE('ts-0002-bb')]: task('ts-0002-bb', `links:\n  - { type: implements, to: "${PROPOSAL}#F1" }  # why\n  - type: implements\n    to: ${PROPOSAL}#F2\n`),
    })
    git(root, 'init', '--quiet')
    commitAll(root, 'fixture')
    const { result } = await call(root, 'apply', { op: 'reid', id: PROPOSAL, args: { seed: 1 } })
    const renamed = result?.task.id as string
    expect(renamed).toMatch(/^pr-0003-(?!km)[a-z]{2}$/)
    expect(read(root, TASK_FILE('ts-0001-aa'))).toBe(linking('ts-0001-aa', link('implements', `${renamed}#F2`), link('blocked-by', 'ts-0002-bb')))
    expect(read(root, TASK_FILE('ts-0002-bb'))).toBe(task('ts-0002-bb', `links:\n  - { type: implements, to: "${renamed}#F1" }  # why\n  - type: implements\n    to: ${renamed}#F2\n`))
    expect(read(root, `${FOLDER}/proposals/${renamed}-features.md`)).toContain(`id: ${renamed}`)
    const listed = await call(root, 'list')
    expect(listed.invalid).toEqual([])
    expect(listed.logs).toEqual([])
  })

  test('of a proposal leaves a link to another proposal and its feature alone', async () => {
    const other = proposal('pr-0004-nn', 'draft', FEATURES)
    const root = repo({
      [`${FOLDER}/proposals/pr-0004-nn-empty.md`]: other,
      [TASK_FILE('ts-0001-aa')]: linking('ts-0001-aa', link('implements', `${PROPOSAL}#F1`), link('implements', 'pr-0004-nn#F2')),
    })
    git(root, 'init', '--quiet')
    commitAll(root, 'fixture')
    const { result } = await call(root, 'apply', { op: 'reid', id: PROPOSAL, args: { seed: 1 } })
    const renamed = result?.task.id as string
    expect(read(root, TASK_FILE('ts-0001-aa'))).toBe(linking('ts-0001-aa', link('implements', `${renamed}#F1`), link('implements', 'pr-0004-nn#F2')))
  })

  test('of a task rewrites the links to it and leaves the fragments on other targets alone', async () => {
    const root = repo({
      [TASK_FILE('ts-0001-aa')]: linking('ts-0001-aa', link('implements', `${PROPOSAL}#F2`)),
      [TASK_FILE('ts-0002-bb')]: linking('ts-0002-bb', link('blocked-by', 'ts-0001-aa'), link('implements', `${PROPOSAL}#F2`)),
    })
    git(root, 'init', '--quiet')
    commitAll(root, 'fixture')
    const { result } = await call(root, 'apply', { op: 'reid', id: 'ts-0001-aa', args: { seed: 1 } })
    const renamed = result?.task.id as string
    expect(renamed).toMatch(/^ts-0001-(?!aa)[a-z]{2}$/)
    expect(read(root, TASK_FILE('ts-0002-bb'))).toBe(linking('ts-0002-bb', link('blocked-by', renamed), link('implements', `${PROPOSAL}#F2`)))
    expect(read(root, `${FOLDER}/tasks/${renamed}-work.md`)).toBe(linking('ts-0001-aa', link('implements', `${PROPOSAL}#F2`)).replace('id: ts-0001-aa', `id: ${renamed}`))
    expect(git(root, 'status', '--porcelain', '--', PROPOSAL_FILE)).toBe('')
  })

  test('reports every file it rewrote, and the proposal file is among them', async () => {
    const root = repo({ [TASK_FILE('ts-0001-aa')]: linking('ts-0001-aa', link('implements', `${PROPOSAL}#F1`)) })
    git(root, 'init', '--quiet')
    commitAll(root, 'fixture')
    const { result } = await call(root, 'apply', { op: 'reid', id: PROPOSAL, args: { seed: 1 } })
    const renamed = result?.task.id as string
    expect(result?.files).toEqual(expect.arrayContaining([`${EPICS}/${TASK_FILE('ts-0001-aa')}`, `${EPICS}/${FOLDER}/proposals/${renamed}-features.md`]))
  })
})

describe('the line edits', () => {
  test('addLinkLine adds an implements entry like it adds a blocked-by one', () => {
    expect(addLinkLine('links: []\n', 'implements', 'pr-0003-km#F2')).toEqual({ ok: true, text: 'links:\n  - { type: implements, to: pr-0003-km#F2 }\n' })
    expect(addLinkLine('links:\n    - { type: blocked-by, to: ts-0002-bb }\n', 'implements', 'pr-0003-km#F2')).toEqual({
      ok: true,
      text: 'links:\n    - { type: blocked-by, to: ts-0002-bb }\n    - { type: implements, to: pr-0003-km#F2 }\n',
    })
  })

  test('replaceLinkTarget keeps the fragment of a target it renames, quoted or not, and leaves other ids alone', () => {
    const text = 'links:\n  - { type: implements, to: pr-0003-km#F2 }\n  - { type: implements, to: "pr-0003-km#F10" } # c\n  - type: implements\n    to: pr-0003-km#F1\n  - { type: implements, to: pr-0003-kmx#F2 }\n  - { type: implements, to: pr-0004-nn#F2 }\n'
    expect(replaceLinkTarget(text, 'pr-0003-km', 'pr-0003-cd')).toEqual({
      ok: true,
      text: 'links:\n  - { type: implements, to: pr-0003-cd#F2 }\n  - { type: implements, to: "pr-0003-cd#F10" } # c\n  - type: implements\n    to: pr-0003-cd#F1\n  - { type: implements, to: pr-0003-kmx#F2 }\n  - { type: implements, to: pr-0004-nn#F2 }\n',
    })
  })
})
