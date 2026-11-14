import test from 'node:test'
import assert from 'node:assert/strict'
import { toCsv } from '../src/report/csv.js'

test('writes a header and one line per item', () => {
  const out = toCsv([{ name: 'salt', amount: 1, unit: 'kg', shelf: 'pantry', expires: null }])
  assert.equal(out.split('\n').length, 3)
})
