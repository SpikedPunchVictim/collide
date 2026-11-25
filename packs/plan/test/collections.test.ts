import { renameSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, test } from 'vitest'
import { call, copyFixture, fixture, frontmatter, makeRepo } from './helpers.ts'

// Each epic is a collection (0.10; ts-0187-hb): `plan:<epic id>`, with the epic's title, and each of its items names it
// in `collections`. The epic's own item is in none, and a spike belongs to no epic. The folder stays the membership:
// the `part-of` link is the same as before.

const EPICS = '.collide/planning/epics'

const taskOf = (result: Record<string, any> | undefined, id: string): Record<string, any> =>
  (result?.tasks as Array<Record<string, any>>).find((task) => task.id === id)!

describe('initialize', () => {
  test('says that the provider has collections', async () => {
    const { result } = await call(fixture('basic'), 'initialize', { config: {} })
    expect(result?.capabilities.collections).toBe(true)
  })
})

describe('list: collections', () => {
  test('lists each epic folder with an epic.md as a collection of its id and its title, in folder order', async () => {
    const { result } = await call(fixture('basic'), 'list')
    expect(result?.collections).toEqual([
      { id: 'plan:ep-001-aa', title: 'Alpha epic', groups: [], path: expect.any(String) },
      { id: 'plan:ep-002-bb', title: expect.any(String), groups: [], path: expect.any(String) },
    ])
  })

  test('names the epic.md of each epic as its path, relative to the line as a task\'s path is', async () => {
    const { result } = await call(fixture('basic'), 'list')
    expect(result?.collections.map((each: { path: string }) => each.path)).toEqual([
      `${EPICS}/ep-001-aa-alpha/epic.md`,
      `${EPICS}/ep-002-bb-beta/epic.md`,
    ])
    expect(taskOf(result, 'ep-001-aa').path).toBe(`${EPICS}/ep-001-aa-alpha/epic.md`)
  })

  test('puts the epic in the collections of every item of its folder, whatever its kind', async () => {
    const { result } = await call(fixture('basic'), 'list')
    for (const id of ['ts-0001-aa', 'ts-0002-bb', 'pr-0001-aa', 'qu-0001-aa']) {
      expect(taskOf(result, id).collections, id).toEqual(['plan:ep-001-aa'])
    }
    for (const id of ['ts-0003-cc', 'ts-0004-dd']) {
      expect(taskOf(result, id).collections, id).toEqual(['plan:ep-002-bb'])
    }
  })

  test('an epic item is in no collection, and a spike is in none either', async () => {
    const { result } = await call(fixture('basic'), 'list')
    expect(taskOf(result, 'ep-001-aa').collections).toBeUndefined()
    expect(taskOf(result, 'sp-0001-aa').collections).toBeUndefined()
  })

  test('keeps the part-of link as it was', async () => {
    const { result } = await call(fixture('basic'), 'list')
    expect(taskOf(result, 'ts-0001-aa').links[0]).toEqual({ type: 'part-of', to: 'ep-001-aa' })
  })

  test('a task file moved into another epic folder is in that epic', async () => {
    const repo = copyFixture('basic')
    renameSync(
      join(repo, EPICS, 'ep-002-bb-beta/tasks/ts-0003-cc-third.md'),
      join(repo, EPICS, 'ep-001-aa-alpha/tasks/ts-0003-cc-third.md'),
    )
    const { result } = await call(repo, 'list')
    expect(taskOf(result, 'ts-0003-cc').collections).toEqual(['plan:ep-001-aa'])
  })

  test('an epic folder with no epic.md is no collection, and its items name none', async () => {
    const repo = makeRepo({ 'ep-003-cc-bare/tasks/ts-0001-aa-x.md': frontmatter('ts-0001-aa') })
    const { result } = await call(repo, 'list')
    expect(result?.collections).toEqual([])
    expect(taskOf(result, 'ts-0001-aa').collections).toBeUndefined()
  })

  test('a repository with no planning folder lists no collection', async () => {
    const { result } = await call(makeRepo({}), 'list')
    expect(result).toEqual({ tasks: [], collections: [], invalid: [] })
  })

  test('get gives the same collections as list', async () => {
    const { result } = await call(fixture('basic'), 'get', { id: 'ts-0001-aa' })
    expect(result?.task.collections).toEqual(['plan:ep-001-aa'])
  })
})
