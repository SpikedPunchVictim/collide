import { describe, expect, test } from 'vitest'
import { call, makeRepo } from './helpers.ts'

type Row = Record<string, any>

const declared = async (): Promise<Row[]> => {
  const { result } = await call(makeRepo({}), 'initialize', { config: {} })
  return result?.links as Row[]
}

describe('the link types the provider declares', () => {
  test('each says what the other end calls it', async () => {
    const inverses = Object.fromEntries((await declared()).map((decl) => [decl.name, decl.inverse]))
    expect(inverses).toEqual({
      'blocked-by': 'blocks',
      'part-of': 'has part',
      implements: 'implemented by',
      informs: 'informed by',
      includes: 'included in',
      supersedes: 'superseded by',
      'raised-by': 'raised',
    })
  })

  test('the inverse is said once, on the declaration, and the declaration keeps its direction and target', async () => {
    const links = await declared()
    expect(links.find((decl) => decl.name === 'implements')).toEqual({
      name: 'implements',
      dir: 'out',
      blocking: false,
      target: 'feature',
      inverse: 'implemented by',
    })
    expect(links.find((decl) => decl.name === 'raised-by')).toEqual({
      name: 'raised-by',
      dir: 'out',
      blocking: false,
      inverse: 'raised',
    })
    expect(links.find((decl) => decl.name === 'blocked-by')).toMatchObject({ dir: 'out', blocking: true })
  })
})
