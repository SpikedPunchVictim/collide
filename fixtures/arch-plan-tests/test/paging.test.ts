import { describe, expect, test } from 'vitest'
import { keysOf, start, tree } from './helpers.ts'

const ROOT = tree('basic')

describe('paging', () => {
  test('a page size of 2 walks every task once, in order, with nextCursor until the last page', async () => {
    const client = await start(ROOT, { pageSize: 2 })
    try {
      const keys: string[] = []
      const sizes: number[] = []
      let cursor: string | undefined
      for (let page = 0; page < 10; page++) {
        const { result } = await client.request('list', cursor === undefined ? {} : { cursor })
        keys.push(...keysOf(result as Record<string, any>))
        sizes.push(result?.tasks.length)
        cursor = result?.nextCursor
        if (cursor === undefined) break
      }
      expect(sizes).toEqual([2, 2, 2, 2])
      expect(keys).toEqual([
        'plan:T-03-001', 'plan:T-03-002', 'plan:T-03-003', 'plan:T-03-004', 'plan:T-03-005',
        'plan:T-05-001', 'plan:T-05-002', 'plan:T-05-003',
      ])
    } finally {
      await client.close()
    }
  })

  test('the collections come with the first page only', async () => {
    const client = await start(ROOT, { pageSize: 2 })
    try {
      const first = await client.request('list')
      const second = await client.request('list', { cursor: first.result?.nextCursor })
      expect(first.result?.collections).toHaveLength(2)
      expect(second.result?.collections).toEqual([])
    } finally {
      await client.close()
    }
  })

  test('the default page is 500 tasks, so the basic tree is one page', async () => {
    const client = await start(ROOT)
    try {
      expect((await client.request('list')).result?.nextCursor).toBeUndefined()
    } finally {
      await client.close()
    }
  })

  test('a cursor that is not one of ours is an error', async () => {
    const client = await start(ROOT, { pageSize: 2 })
    try {
      expect((await client.request('list', { cursor: 'nonsense' })).error?.message).toMatch(/cursor/)
    } finally {
      await client.close()
    }
  })
})
