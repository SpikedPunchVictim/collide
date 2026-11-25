import { describe, expect, test } from 'vitest'
import { type FoldStore, memoryStore } from '../src/field/folds.ts'
import { layoutKey, layoutOf, writeLayout } from '../src/field/layout.ts'
import { view } from './support/field-world.ts'

// The layout of a board's view, kept in the webview's storage like the folds (ts-0187-hb).

const broken: FoldStore = {
  getItem: () => {
    throw new Error('storage is blocked')
  },
  setItem: () => {
    throw new Error('storage is blocked')
  },
}

describe('the layout of a view', () => {
  test('starts as the board when the view says nothing and the person chose nothing', () => {
    expect(layoutOf(memoryStore(), 'b-alpha', 'ready', view('ready', { type: 'repo' }))).toBe('board')
    expect(layoutOf(memoryStore(), 'b-alpha', null, undefined)).toBe('board')
  })

  test('starts as the view`s own `layout.default`', () => {
    const graph = view('plan', { type: 'repo' }, { layout: { default: 'graph' } })
    const run = view('plan', { type: 'repo' }, { layout: { default: 'run-order' } })
    expect(layoutOf(memoryStore(), 'b-alpha', 'plan', graph)).toBe('graph')
    expect(layoutOf(memoryStore(), 'b-alpha', 'plan', run)).toBe('run-order')
  })

  test('is the one the person chose, over the view`s default', () => {
    const store = memoryStore()
    const graph = view('plan', { type: 'repo' }, { layout: { default: 'graph' } })
    writeLayout(store, 'b-alpha', 'plan', 'run-order')
    expect(layoutOf(store, 'b-alpha', 'plan', graph)).toBe('run-order')
    writeLayout(store, 'b-alpha', 'plan', 'board')
    expect(layoutOf(store, 'b-alpha', 'plan', graph)).toBe('board')
  })

  test('is kept for each board and view on its own', () => {
    const store = memoryStore()
    writeLayout(store, 'b-alpha', 'plan', 'graph')
    expect(layoutOf(store, 'b-alpha', 'plan', undefined)).toBe('graph')
    expect(layoutOf(store, 'b-alpha', 'mine', undefined)).toBe('board')
    expect(layoutOf(store, 'b-beta', 'plan', undefined)).toBe('board')
    expect(layoutOf(store, 'b-alpha', null, undefined)).toBe('board')
    writeLayout(store, 'b-alpha', null, 'run-order')
    expect(layoutOf(store, 'b-alpha', null, undefined)).toBe('run-order')
  })

  test('keeps the key apart from the folds', () => {
    expect(layoutKey('b-alpha', 'plan')).not.toBe(layoutKey('b-alpha', null))
    expect(layoutKey('b-alpha', 'plan')).toContain('layout')
  })

  test('ignores something kept that is not a layout, and then starts as the view does', () => {
    const store = memoryStore()
    store.setItem(layoutKey('b-alpha', 'plan'), 'sideways')
    const graph = view('plan', { type: 'repo' }, { layout: { default: 'graph' } })
    expect(layoutOf(store, 'b-alpha', 'plan', graph)).toBe('graph')
  })

  test('is the starting one when there is no storage, or it cannot be read or written', () => {
    const run = view('plan', { type: 'repo' }, { layout: { default: 'run-order' } })
    expect(layoutOf(null, 'b-alpha', 'plan', run)).toBe('run-order')
    expect(layoutOf(broken, 'b-alpha', 'plan', run)).toBe('run-order')
    expect(() => writeLayout(broken, 'b-alpha', 'plan', 'graph')).not.toThrow()
    expect(() => writeLayout(null, 'b-alpha', 'plan', 'graph')).not.toThrow()
  })
})
