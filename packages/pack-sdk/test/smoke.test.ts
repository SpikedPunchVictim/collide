import { expect, test } from 'vitest'
import { name } from '../src/index'

test('package identifies itself', () => {
  expect(name).toBe('@collide/pack-sdk')
})
