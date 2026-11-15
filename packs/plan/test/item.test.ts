import { expect, test } from 'vitest'
import { readItem } from '../src/item.ts'

const valid = { id: 'ts-0001-aa', kind: 'task', status: 'ready', title: 'Made-up item' }
const reason = (changes: Record<string, unknown>): string | undefined => {
  const result = readItem({ ...valid, ...changes })
  return result.ok ? undefined : result.reason
}

test('the smallest valid frontmatter has defaults for owners, links and fields', () => {
  expect(readItem(valid)).toEqual({ ok: true, item: { ...valid, alias: null, owners: [], links: [], fields: {} } })
})

test.each([
  ['a number id', { id: 1234 }, 'id must be a string'],
  ['a null id', { id: null }, 'id must be a string, not null'],
  ['a boolean title', { title: true }, 'title must be a non-empty string'],
  ['a blank title', { title: '  ' }, 'title must be a non-empty string'],
  ['a number alias', { alias: 5 }, 'alias must be a string'],
  ['a list kind', { kind: ['task'] }, 'kind must be a non-empty string, not a list'],
  ['a number status', { status: 1 }, 'status must be one of'],
  ['owners that is not a list', { owners: 'me' }, 'owners must be a list of strings'],
  ['an owner that is a number', { owners: [1] }, 'owners must be a list of strings'],
  ['links that is not a list', { links: 'none' }, 'links must be a list'],
  ['a link with an extra key', { links: [{ type: 'blocked-by', to: 'ts-0002-aa', blocking: true }] }, 'only type and to'],
  ['a link target that is a number', { links: [{ type: 'blocked-by', to: 1e10 }] }, 'links[0].to must be a string'],
  ['a mapping field', { extra: { a: 1 } }, 'field "extra" must be a string'],
  ['a number field', { extra: 12 }, 'field "extra" must be a string'],
  ['a null field', { extra: null }, 'field "extra" must be a string, a boolean or a list of strings, not null'],
  ['a list of numbers field', { extra: [1, 2] }, 'field "extra"'],
])('%s is refused', (_name, changes, expected) => {
  expect(reason(changes)).toContain(expected)
})

test('a field that is a string, a boolean or a list of strings is kept as it is', () => {
  const result = readItem({ ...valid, hash: '3e10', done: true, tags: ['a', 'b'] })
  expect(result).toMatchObject({ ok: true, item: { fields: { hash: '3e10', done: true, tags: ['a', 'b'] } } })
})

test('a key named __proto__ is refused, so it can never reach the fields object', () => {
  const frontmatter = JSON.parse('{"id":"ts-0001-aa","kind":"task","status":"ready","title":"t","__proto__":"x"}')
  expect(readItem(frontmatter)).toEqual({ ok: false, reason: 'the key "__proto__" is reserved' })
})
