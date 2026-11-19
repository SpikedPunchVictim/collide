import { mkdirSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { describe, expect, test } from 'vitest'
import { call, EPIC_FILE, tempDir } from './helpers.ts'

const ROOT = '.collide/planning'
const FOLDER = `${ROOT}/epics/ep-001-aa-alpha`
const PROPOSAL = 'pr-0003-km'

type Row = Record<string, any>

const doc = (kind: string, id: string, status: string, links: string[], body = '## Goal\n'): string => {
  const list = links.length === 0 ? 'links: []\n' : `links:\n${links.map((entry) => `  - ${entry}`).join('\n')}\n`
  const head = `id: ${id}\nkind: ${kind}\nstatus: ${status}\ntitle: "Made-up ${kind}"\nowners: []\n${list}`
  return `---\n${head}---\n\n${body}`
}

const implementsOf = (feature: string, proposal = PROPOSAL): string =>
  `{ type: implements, to: ${proposal}#${feature} }`
const includes = (...ids: string[]): string[] => ids.map((id) => `{ type: includes, to: ${id} }`)

const THREE_FEATURES = [
  '## Features',
  '',
  '### Packs',
  '- **F1 Trust prompt**: a repo pack asks first.',
  '- **F2 Pack lock**: pins each pack.',
  '- **F3 Badge**: the Queue count.',
  '',
].join('\n')

/** A temp repository: the epic, the proposal with `body` and `links`, and the other items. */
function repoOf(
  body: string,
  proposalLinks: string[],
  others: Record<string, string> = {},
  status = 'accepted',
): string {
  const dir = tempDir()
  const tree: Record<string, string> = {
    [`${FOLDER}/epic.md`]: EPIC_FILE,
    [`${FOLDER}/proposals/${PROPOSAL}-coverage.md`]: doc('proposal', PROPOSAL, status, proposalLinks, body),
    ...others,
  }
  for (const [path, text] of Object.entries(tree)) {
    mkdirSync(dirname(join(dir, path)), { recursive: true })
    writeFileSync(join(dir, path), text)
  }
  return dir
}

const task = (id: string, status: string, links: string[] = []): Record<string, string> => ({
  [`${FOLDER}/tasks/${id}-work.md`]: doc('task', id, status, links),
})

const proposalOf = async (root: string): Promise<Row> => {
  const tasks = ((await call(root, 'list')).result?.tasks ?? []) as Row[]
  return tasks.find((candidate) => candidate.id === PROPOSAL)!
}

describe('coverage', () => {
  test('three features and two included tasks, one implementing F1: names F2 and F3 and the other task', async () => {
    const root = repoOf(THREE_FEATURES, includes('ts-0001-aa', 'ts-0002-bb'), {
      ...task('ts-0001-aa', 'ready', [implementsOf('F1')]),
      ...task('ts-0002-bb', 'ready'),
    })
    expect((await proposalOf(root)).coverage).toEqual({
      featuresWithoutTask: ['F2', 'F3'],
      tasksWithoutFeature: ['ts-0002-bb'],
    })
  })

  test('get reports the same coverage as list', async () => {
    const root = repoOf(THREE_FEATURES, includes('ts-0001-aa', 'ts-0002-bb'), {
      ...task('ts-0001-aa', 'ready', [implementsOf('F1')]),
      ...task('ts-0002-bb', 'ready'),
    })
    const { result } = await call(root, 'get', { id: PROPOSAL })
    expect(result?.task.coverage).toEqual({ featuresWithoutTask: ['F2', 'F3'], tasksWithoutFeature: ['ts-0002-bb'] })
  })

  test('every feature has a task and every included task implements one: both lists empty, key stays', async () => {
    const root = repoOf(THREE_FEATURES, includes('ts-0001-aa'), {
      ...task('ts-0001-aa', 'ready', [implementsOf('F1'), implementsOf('F2'), implementsOf('F3')]),
    })
    expect((await proposalOf(root)).coverage).toEqual({ featuresWithoutTask: [], tasksWithoutFeature: [] })
  })

  test('a dropped included task is not named', async () => {
    const root = repoOf(THREE_FEATURES, includes('ts-0001-aa', 'ts-0002-bb'), {
      ...task('ts-0001-aa', 'ready', [implementsOf('F1')]),
      ...task('ts-0002-bb', 'dropped'),
    })
    expect((await proposalOf(root)).coverage.tasksWithoutFeature).toEqual([])
  })

  test('a dropped task that implements a feature covers nothing', async () => {
    const root = repoOf(THREE_FEATURES, [], {
      ...task('ts-0001-aa', 'dropped', [implementsOf('F1')]),
    })
    expect((await proposalOf(root)).coverage.featuresWithoutTask).toEqual(['F1', 'F2', 'F3'])
  })

  test('a task that implements a feature without being included counts for it and is not named', async () => {
    const root = repoOf(THREE_FEATURES, includes('ts-0002-bb'), {
      ...task('ts-0001-aa', 'ready', [implementsOf('F2')]),
      ...task('ts-0002-bb', 'ready', [implementsOf('F1')]),
    })
    expect((await proposalOf(root)).coverage).toEqual({ featuresWithoutTask: ['F3'], tasksWithoutFeature: [] })
  })

  test('a proposal with features and no tasks names every feature and no task', async () => {
    const root = repoOf(THREE_FEATURES, [])
    expect((await proposalOf(root)).coverage).toEqual({
      featuresWithoutTask: ['F1', 'F2', 'F3'],
      tasksWithoutFeature: [],
    })
  })

  test('a proposal with includes and no features names every included task', async () => {
    const root = repoOf('## Goal\n', includes('ts-0001-aa', 'ts-0002-bb'), {
      ...task('ts-0001-aa', 'ready'),
      ...task('ts-0002-bb', 'active'),
    })
    expect((await proposalOf(root)).coverage).toEqual({
      featuresWithoutTask: [],
      tasksWithoutFeature: ['ts-0001-aa', 'ts-0002-bb'],
    })
  })

  test('an included question, spike or epic is not a task and is never named', async () => {
    const root = repoOf(THREE_FEATURES, includes('qu-0001-aa', 'sp-0001-aa', 'ts-0001-aa'), {
      ...task('ts-0001-aa', 'ready', [implementsOf('F1')]),
      [`${FOLDER}/questions/qu-0001-aa-why.md`]: doc('question', 'qu-0001-aa', 'open', []),
      [`${ROOT}/spikes/sp-0001-aa-demo/record.md`]: doc('spike', 'sp-0001-aa', 'planned', []),
    })
    expect((await proposalOf(root)).coverage.tasksWithoutFeature).toEqual([])
  })

  test('an included id that no item has is not named', async () => {
    const root = repoOf(THREE_FEATURES, includes('ts-9999-zz'))
    expect((await proposalOf(root)).coverage.tasksWithoutFeature).toEqual([])
  })

  test('an included task that implements a feature of another proposal implements a feature', async () => {
    const other = 'pr-0004-xy'
    const root = repoOf(THREE_FEATURES, includes('ts-0001-aa'), {
      ...task('ts-0001-aa', 'ready', [implementsOf('F1', other)]),
      [`${FOLDER}/proposals/${other}-other.md`]: doc('proposal', other, 'accepted', [], THREE_FEATURES),
    })
    const coverage = (await proposalOf(root)).coverage
    expect(coverage.tasksWithoutFeature).toEqual([])
    expect(coverage.featuresWithoutTask).toEqual(['F1', 'F2', 'F3'])
  })

  test('a task that implements a feature that does not exist implements none and covers nothing', async () => {
    const root = repoOf(THREE_FEATURES, includes('ts-0001-aa'), {
      ...task('ts-0001-aa', 'ready', [implementsOf('F9'), implementsOf('F1', 'pr-0099-zz')]),
    })
    const coverage = (await proposalOf(root)).coverage
    expect(coverage.tasksWithoutFeature).toEqual(['ts-0001-aa'])
    expect(coverage.featuresWithoutTask).toEqual(['F1', 'F2', 'F3'])
  })

  test('the same task included twice is named once, in the order of the includes', async () => {
    const root = repoOf('## Goal\n', includes('ts-0002-bb', 'ts-0001-aa', 'ts-0002-bb'), {
      ...task('ts-0001-aa', 'ready'),
      ...task('ts-0002-bb', 'ready'),
    })
    expect((await proposalOf(root)).coverage.tasksWithoutFeature).toEqual(['ts-0002-bb', 'ts-0001-aa'])
  })

  test('a proposal with no features and no includes has no coverage key, and neither has a task', async () => {
    const root = repoOf('## Goal\n', [], task('ts-0001-aa', 'ready', [implementsOf('F1')]))
    const tasks = ((await call(root, 'list')).result?.tasks ?? []) as Row[]
    for (const row of tasks) expect(row, row.id).not.toHaveProperty('coverage')
  })

  test('a draft proposal has coverage too, as it is derived from the links and not from the status', async () => {
    const root = repoOf(THREE_FEATURES, [], {}, 'draft')
    expect((await proposalOf(root)).coverage.featuresWithoutTask).toEqual(['F1', 'F2', 'F3'])
  })
})
