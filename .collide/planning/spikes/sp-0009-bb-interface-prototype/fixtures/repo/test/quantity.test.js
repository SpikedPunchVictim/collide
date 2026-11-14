import test from 'node:test'
import assert from 'node:assert/strict'
import { parseQuantity } from '../src/parse/quantity.js'

test('reads an amount, a unit and a name', () => {
  assert.deepEqual(parseQuantity('2 cups flour'), { amount: 2, unit: 'cups', name: 'flour' })
})

test('a bare number means each', () => {
  assert.equal(parseQuantity('3 eggs').unit, 'eggs')
})
