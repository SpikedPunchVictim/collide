import test from 'node:test'
import assert from 'node:assert/strict'
import { expiringSoon } from '../src/notify/expiry.js'

test('an item that expires tomorrow is flagged', () => {
  const now = new Date('2026-03-01T08:00:00Z')
  const items = [{ name: 'milk', expires: new Date('2026-03-02T00:00:00Z') }]
  assert.equal(expiringSoon(items, now).length, 1)
})
