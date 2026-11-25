import { describe, expect, test } from 'vitest'
import { FOLDS_KEY, STARTS_SHUT, readShut, toggleShut } from '../src/drawer/folds.ts'
import {
  BIG_STEP,
  DEFAULT_WIDTH,
  MIN_WIDTH,
  STEP,
  WIDTH_KEY,
  clampWidth,
  maxWidth,
  readWidth,
  writeWidth,
} from '../src/drawer/width.ts'
import { type FoldStore, memoryStore } from '../src/field/folds.ts'

// What the drawer keeps in the webview's storage (ts-0184-hv): which parts are folded, and how wide it is. Plain
// functions over a store the test gives, so there is no browser here and no real storage.

/** A store that fails as a blocked one does: `getItem` throws, or `setItem` does. */
function broken(on: 'get' | 'set' | 'both'): FoldStore {
  const fail = () => {
    throw new Error('storage is blocked')
  }
  return {
    getItem: on === 'set' ? () => null : fail,
    setItem: on === 'get' ? () => undefined : fail,
  }
}

describe('the width of the drawer', () => {
  test('is 460 px until the person sets it, between 360 px and 70% of the window', () => {
    expect(DEFAULT_WIDTH).toBe(460)
    expect(MIN_WIDTH).toBe(360)
    expect(maxWidth(1000)).toBe(700)
    expect(maxWidth(1280)).toBe(896)
  })

  test.each([
    [100, 1280, 360],
    [359.4, 1280, 360],
    [360, 1280, 360],
    [500.4, 1280, 500],
    [896, 1280, 896],
    [897, 1280, 896],
    [5000, 1280, 896],
  ])('a width of %d in a window of %d is %d', (width, room, kept) => {
    expect(clampWidth(width, room)).toBe(kept)
  })

  test('is never below 360 px, though the window is so narrow that 70% is less', () => {
    expect(maxWidth(400)).toBe(360)
    expect(clampWidth(900, 400)).toBe(360)
  })

  test('steps by 40 px on an arrow key, and by 120 px with Shift', () => {
    expect([STEP, BIG_STEP]).toEqual([40, 120])
  })

  test('is kept in the store and read back', () => {
    const store = memoryStore()
    expect(readWidth(store)).toBeNull()
    writeWidth(store, 612)
    expect(store.getItem(WIDTH_KEY)).toBe('612')
    expect(readWidth(store)).toBe(612)
  })

  test.each(['', 'wide', 'NaN', 'Infinity', '-5', '0', '12px', '[]', 'null'])('ignores %j in the store', (text) => {
    const store = memoryStore()
    store.setItem(WIDTH_KEY, text)
    expect(readWidth(store)).toBeNull()
  })

  test('is the default when storage cannot be read, and a write that fails is not an error', () => {
    expect(readWidth(null)).toBeNull()
    expect(readWidth(broken('get'))).toBeNull()
    expect(() => writeWidth(broken('set'), 500)).not.toThrow()
    expect(() => writeWidth(null, 500)).not.toThrow()
  })
})

describe('the folds of the drawer', () => {
  test('start with Details for engineers folded and nothing else', () => {
    expect(readShut(memoryStore())).toEqual(['details'])
    expect(readShut(memoryStore())).toEqual([...STARTS_SHUT])
  })

  test('a part the person folds stays folded, and the others are as they were', () => {
    const store = memoryStore()
    expect(toggleShut(store, 'description')).toEqual(['details', 'description'])
    expect(readShut(store)).toEqual(['details', 'description'])
  })

  test('a part the person opens stays open, Details for engineers too', () => {
    const store = memoryStore()
    expect(toggleShut(store, 'details')).toEqual([])
    expect(readShut(store)).toEqual([])
  })

  test('folding a part twice opens it again', () => {
    const store = memoryStore()
    toggleShut(store, 'waits')
    expect(toggleShut(store, 'waits')).toEqual(['details'])
  })

  test('are one set for every task: a second reader of the store sees them', () => {
    const store = memoryStore()
    toggleShut(store, 'fields')
    expect(readShut(store)).toContain('fields')
  })

  test.each(['', 'nope', '[]', 'null', '{"shut":"description"}'])('read %j as the start', (text) => {
    const store = memoryStore()
    store.setItem(FOLDS_KEY, text)
    expect(readShut(store)).toEqual(['details'])
  })

  test('read only the names in the store', () => {
    const store = memoryStore()
    store.setItem(FOLDS_KEY, JSON.stringify({ shut: ['waits', 7, null, 'copies'] }))
    expect(readShut(store)).toEqual(['waits', 'copies'])
  })

  test('are the start when storage cannot be read, and a write that fails is not an error', () => {
    expect(readShut(null)).toEqual(['details'])
    expect(readShut(broken('get'))).toEqual(['details'])
    expect(() => toggleShut(broken('both'), 'waits')).not.toThrow()
    // The fold holds for this call, though it could not be kept.
    expect(toggleShut(broken('set'), 'waits')).toEqual(['details', 'waits'])
  })
})
