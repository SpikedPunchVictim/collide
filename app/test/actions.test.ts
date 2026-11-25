import { describe, expect, test } from 'vitest'
import { ACTIONS, actionForKey, keyOf, tabOfAction } from '../src/frame/actions.ts'
import { TABS } from '../src/frame/tabs.ts'

// The named actions of the window (Stage 6a, Named actions): one list, each with its key, and the keys bound from it.

describe('the list of named actions', () => {
  test('every name is its own, and every key is one per scope', () => {
    expect(new Set(ACTIONS.map((action) => action.name)).size).toBe(ACTIONS.length)
    const keys = ACTIONS.map((action) => `${action.scope}:${action.key}`)
    expect(new Set(keys).size).toBe(keys.length)
  })

  test('each has words for the palette', () => {
    expect(ACTIONS.length).toBeGreaterThan(0)
    for (const action of ACTIONS) expect(action.label.length).toBeGreaterThan(3)
  })

  test('the five tabs are shown by the keys 1 to 5, in the order of the tabs', () => {
    expect(TABS.map((tab) => keyOf(`show-${tab.id}` as never))).toEqual(['1', '2', '3', '4', '5'])
    expect(TABS.map((tab) => tabOfAction(`show-${tab.id}` as never))).toEqual(TABS.map((tab) => tab.id))
  })

  test.each([
    ['1', 'show-field'],
    ['2', 'show-tracks'],
    ['3', 'show-queue'],
    ['4', 'show-planning'],
    ['5', 'show-comms'],
  ])('the key %s does %s in the window', (key, name) => {
    expect(actionForKey(key, 'window')).toBe(name)
  })

  test.each(['0', '6', '12', 'a', ''])('the key %j does nothing in the window', (key) => {
    expect(actionForKey(key, 'window')).toBeNull()
  })

  test('the key / goes to the search of the filter bar, from anywhere in the window', () => {
    expect(actionForKey('/', 'window')).toBe('filter-search')
    expect(actionForKey('/', 'field')).toBeNull()
    expect(keyOf('filter-search')).toBe('/')
    expect(tabOfAction('filter-search')).toBeNull()
  })

  test('an action that is not a tab shows no tab', () => {
    expect(tabOfAction(null)).toBeNull()
    expect(tabOfAction('card-open')).toBeNull()
  })
})

describe('the Field actions', () => {
  test.each([
    ['ArrowDown', 'card-down'],
    ['ArrowUp', 'card-up'],
    ['ArrowRight', 'card-right'],
    ['ArrowLeft', 'card-left'],
    ['Enter', 'card-open'],
  ])('the key %s does %s on the board', (key, name) => {
    expect(actionForKey(key, 'field')).toBe(name)
  })

  test('the board hears none of the tab keys, and the window none of the card keys', () => {
    expect(actionForKey('1', 'field')).toBeNull()
    expect(actionForKey('ArrowDown', 'window')).toBeNull()
  })
})

describe('the drawer actions', () => {
  test('Escape closes the drawer, wherever the focus is in the window', () => {
    expect(actionForKey('Escape', 'drawer')).toBe('drawer-close')
  })

  test.each([
    ['ArrowLeft', 'drawer-wider'],
    ['ArrowRight', 'drawer-narrower'],
  ])('the key %s does %s on the drawer edge', (key, name) => {
    expect(actionForKey(key, 'drawer-edge')).toBe(name)
  })

  test('the edge hears no other key, and the board and the window hear none of the drawer keys', () => {
    expect(actionForKey('ArrowUp', 'drawer-edge')).toBeNull()
    expect(actionForKey('Escape', 'field')).toBeNull()
    expect(actionForKey('Escape', 'window')).toBeNull()
    expect(actionForKey('ArrowLeft', 'drawer')).toBeNull()
  })

  test('none of them shows a tab', () => {
    expect(tabOfAction('drawer-close')).toBeNull()
    expect(tabOfAction('drawer-wider')).toBeNull()
  })
})
