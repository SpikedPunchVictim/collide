import { createHash } from 'node:crypto'
import { readFileSync, renameSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, test } from 'vitest'
import { call, copyTree, keysOf, listAll, taskOf, tree } from './helpers.ts'

const ROOT = tree('basic')
const TASKS = '03-harbor-index/tasks'
const sha256 = (path: string) => createHash('sha256').update(readFileSync(path)).digest('hex')

describe('list over the basic tree', () => {
  test('lists every task file once, in short id order, and nothing else', async () => {
    const result = await listAll(ROOT)
    expect(keysOf(result)).toEqual([
      'plan:T-03-001', 'plan:T-03-002', 'plan:T-03-003', 'plan:T-03-004', 'plan:T-03-005',
      'plan:T-05-001', 'plan:T-05-002', 'plan:T-05-003',
    ])
    expect(result.invalid).toEqual([])
    expect(result.nextCursor).toBeUndefined()
  })

  test('gives a task its key, long id, title, labelId, path, checkout and fields', async () => {
    const task = taskOf(await listAll(ROOT), 'T-03-002')
    expect(task).toMatchObject({
      key: 'plan:T-03-002',
      id: 'T-03-002-map-the-moorings',
      title: 'Map the moorings',
      templateFields: { labelId: 't03-002' },
      path: `arch/proposals/${TASKS}/T-03-002-map-the-moorings.md`,
      checkout: ROOT,
      fields: { proposal: '03', epic: 'E-07' },
    })
    expect(task.aliases ?? []).toEqual([])
  })

  test('maps each status to its category', async () => {
    const result = await listAll(ROOT)
    const stateOf = (id: string) => taskOf(result, id).state
    expect(stateOf('T-03-004')).toEqual({ status: 'specified', category: 'backlog' })
    expect(stateOf('T-03-003')).toEqual({ status: 'ready', category: 'ready' })
    expect(stateOf('T-03-002')).toEqual({ status: 'doing', category: 'active' })
    expect(stateOf('T-05-001')).toEqual({ status: 'in_review', category: 'review' })
    expect(stateOf('T-03-001')).toEqual({ status: 'done', category: 'done' })
    expect(stateOf('T-03-005')).toEqual({ status: 'dropped', category: 'dropped' })
  })

  test('turns depends_on into blocked-by links by short id, from a flow list, a block list or a long id', async () => {
    const result = await listAll(ROOT)
    expect(taskOf(result, 'T-03-001').links).toEqual([])
    expect(taskOf(result, 'T-03-003').links).toEqual([
      { type: 'blocked-by', to: 'T-03-001' },
      { type: 'blocked-by', to: 'T-03-002' },
    ])
    expect(taskOf(result, 'T-03-004').links).toEqual([{ type: 'blocked-by', to: 'T-03-002' }])
    expect(taskOf(result, 'T-05-002').links).toEqual([{ type: 'blocked-by', to: 'T-05-001' }])
  })

  test('the version is the sha256 of the file, and the body is not in list', async () => {
    const task = taskOf(await listAll(ROOT), 'T-03-003')
    expect(task.version).toBe(sha256(join(ROOT, `arch/proposals/${TASKS}/T-03-003-price-the-berths.md`)))
    expect(task).not.toHaveProperty('body')
  })

  test('is empty for a checkout with no arch folder', async () => {
    const result = await listAll(join(tree('basic'), 'arch', 'work'))
    expect(result).toMatchObject({ tasks: [], collections: [], invalid: [] })
  })

  test('names one checkout only: the root is fine, another path is an error', async () => {
    expect((await call(ROOT, 'list', { checkout: ROOT })).result?.tasks).toHaveLength(8)
    expect((await call(ROOT, 'list', { checkout: '/elsewhere' })).error?.message).toMatch(/checkout/)
  })
})

describe('get', () => {
  test('finds a task by its short id, its long id and its key, with the body', async () => {
    for (const id of ['T-03-002', 'T-03-002-map-the-moorings', 'plan:T-03-002']) {
      const { result } = await call(ROOT, 'get', { id })
      expect(result?.task.key, id).toBe('plan:T-03-002')
      expect(result?.task.body, id).toContain('Made-up body for T-03-002.')
    }
  })

  test('answers no task for a missing id, as an answer and not an error', async () => {
    const reply = await call(ROOT, 'get', { id: 'T-99-999' })
    expect(reply.error).toBeUndefined()
    expect(reply.result).toEqual({})
  })

  test('a long id whose slug is wrong finds nothing', async () => {
    expect((await call(ROOT, 'get', { id: 'T-03-002-another-slug' })).result).toEqual({})
  })

  test('a stem that carries its epic (T-03-002-e-07-<slug>) is the long id, and the task keeps its epic', async () => {
    const root = copyTree('basic')
    const stem = 'T-03-002-e-07-map-the-moorings'
    renameSync(
      join(root, `arch/proposals/${TASKS}/T-03-002-map-the-moorings.md`),
      join(root, `arch/proposals/${TASKS}/${stem}.md`),
    )
    const listed = taskOf(await listAll(root), 'T-03-002')
    expect(listed.id).toBe(stem)
    expect(listed.collections).toContain('plan:E-07')
    for (const id of ['T-03-002', stem, 'plan:T-03-002']) {
      const { result } = await call(root, 'get', { id })
      expect(result?.task.id, id).toBe(stem)
    }
  })
})

describe('collections', () => {
  test('lists each epic with its slices and streams, in the order of the epic file', async () => {
    const result = await listAll(ROOT)
    expect(result.collections).toEqual([
      {
        id: 'plan:E-07',
        title: 'Harbor plan',
        groups: [
          { name: 'Slice 1: first berths', tasks: ['plan:T-03-001', 'plan:T-03-002'] },
          { name: 'Slice 2: the price list', tasks: ['plan:T-03-003', 'plan:T-05-003'] },
          { name: 'S1: quay work', tasks: ['plan:T-03-001', 'plan:T-03-002'] },
          { name: 'S2: pricing', tasks: ['plan:T-03-003', 'plan:T-05-003'] },
        ],
      },
      { id: 'plan:E-08', title: 'Signal lamps', groups: [] },
    ])
  })

  test('plan:E-07 is on exactly the tasks the epic tables list', async () => {
    const result = await listAll(ROOT)
    const tasks = result.tasks as Array<Record<string, any>>
    const inEpic = (epic: string) => tasks.filter((task) => task.collections.includes(epic)).map((task) => task.key)
    expect(inEpic('plan:E-07')).toEqual([
      'plan:T-03-001', 'plan:T-03-002', 'plan:T-03-003', 'plan:T-03-005', 'plan:T-05-003',
    ])
    expect(inEpic('plan:E-08')).toEqual(['plan:T-03-004', 'plan:T-05-001', 'plan:T-05-002', 'plan:T-05-003'])
  })

  test('a task carries each epic that names it, in epic order, and no slice or stream', async () => {
    const result = await listAll(ROOT)
    expect(taskOf(result, 'T-03-002').collections).toEqual(['plan:E-07'])
    expect(taskOf(result, 'T-05-003').collections).toEqual(['plan:E-07', 'plan:E-08'])
    expect(taskOf(result, 'T-05-002').collections).toEqual(['plan:E-08'])
  })

  test('a gaps table, prose and a code fence name tasks but join no group', async () => {
    const result = await listAll(ROOT)
    expect(taskOf(result, 'T-03-004').collections).toEqual(['plan:E-08'])
    expect(taskOf(result, 'T-05-001').collections).toEqual(['plan:E-08'])
  })

  test('a task whose epic: names the epic belongs to it even when the generated table does not list it', async () => {
    const root = copyTree('basic')
    const epic = join(root, 'arch/work/epics/E-07-harbor-plan.md')
    writeFileSync(epic, readFileSync(epic, 'utf8').replace(/^\| \[T-03-005\].*\n/m, ''))
    const result = await listAll(root)
    expect(taskOf(result, 'T-03-005').collections).toContain('plan:E-07')
  })
})

describe('versions', () => {
  test('editing one task file changes that task version and no other, and editing the epic changes none', async () => {
    const root = copyTree('basic')
    const before = await listAll(root)
    const versions = (result: Record<string, any>) =>
      Object.fromEntries(result.tasks.map((task: any) => [task.key, task.version]))

    const task = join(root, `arch/proposals/${TASKS}/T-03-003-price-the-berths.md`)
    writeFileSync(task, `${readFileSync(task, 'utf8')}\nOne more line.\n`)
    const afterTask = versions(await listAll(root))
    const changed = Object.keys(afterTask).filter((key) => afterTask[key] !== versions(before)[key])
    expect(changed).toEqual(['plan:T-03-003'])

    const epic = join(root, 'arch/work/epics/E-07-harbor-plan.md')
    writeFileSync(epic, `${readFileSync(epic, 'utf8')}\nOne more line.\n`)
    expect(versions(await listAll(root))).toEqual(afterTask)
  })
})
