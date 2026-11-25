import { describe, expect, test } from 'vitest'
import { isSearchKey } from '../src/filters/keys.ts'

// The key that goes to the search box (ts-0190-tz): `/`, off while a text box has the focus, as the tab keys are.

const press = (over: Partial<Parameters<typeof isSearchKey>[0]> = {}) =>
  isSearchKey({ key: '/', metaKey: false, ctrlKey: false, altKey: false, target: { tagName: 'BODY' }, ...over })

describe('the search key', () => {
  test('is / with the focus on the page', () => {
    expect(press()).toBe(true)
    expect(press({ target: { tagName: 'BUTTON' } })).toBe(true)
    expect(press({ target: null })).toBe(true)
  })

  test.each(['INPUT', 'TEXTAREA', 'SELECT'])('is off while a %s has the focus, so a / can be typed', (tagName) => {
    expect(press({ target: { tagName } })).toBe(false)
  })

  test('is off in a field that is edited in place', () => {
    expect(press({ target: { tagName: 'DIV', isContentEditable: true } })).toBe(false)
  })

  test('is off with a modifier, and for any other key', () => {
    expect(press({ metaKey: true })).toBe(false)
    expect(press({ ctrlKey: true })).toBe(false)
    expect(press({ altKey: true })).toBe(false)
    expect(press({ key: '1' })).toBe(false)
  })
})
