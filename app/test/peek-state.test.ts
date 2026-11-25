import { describe, expect, test } from 'vitest'
import { CLOSED, type PeekState, peekReducer } from '../src/peek/peek.ts'

// What the id peek is showing (ts-0189-fg; note 34): hover shows it, a click keeps it, an id inside it opens in the
// same window with Back, and Esc or a click away closes it. The timers and the pointer are in
// `browser/fake-peek.spec.ts`.

const AT = { left: 10, top: 20, bottom: 40 }
const ELSEWHERE = { left: 300, top: 120, bottom: 140 }

describe('hover and click', () => {
  test('hovering an id shows it, not kept, where the id is', () => {
    expect(peekReducer(CLOSED, { type: 'hover', id: 'ts-0001-aa', at: AT })).toEqual({
      stack: ['ts-0001-aa'],
      pinned: false,
      at: AT,
    })
  })

  test('hovering another id while one is shown but not kept shows the other', () => {
    const hovered = peekReducer(CLOSED, { type: 'hover', id: 'ts-0001-aa', at: AT })
    const next = peekReducer(hovered, { type: 'hover', id: 'ts-0002-bb', at: ELSEWHERE })
    expect(next).toEqual({ stack: ['ts-0002-bb'], pinned: false, at: ELSEWHERE })
  })

  test('hovering an id while one is kept changes nothing', () => {
    const kept = peekReducer(CLOSED, { type: 'pin', id: 'ts-0001-aa', at: AT })
    expect(peekReducer(kept, { type: 'hover', id: 'ts-0002-bb', at: ELSEWHERE })).toBe(kept)
  })

  test('a click keeps what hover showed', () => {
    const hovered = peekReducer(CLOSED, { type: 'hover', id: 'ts-0001-aa', at: AT })
    expect(peekReducer(hovered, { type: 'pin', id: 'ts-0001-aa', at: AT })).toEqual({
      stack: ['ts-0001-aa'],
      pinned: true,
      at: AT,
    })
  })

  test('a click on another id while one is kept shows the other, kept, and forgets the way back', () => {
    const kept = peekReducer(CLOSED, { type: 'pin', id: 'ts-0001-aa', at: AT })
    const inner = peekReducer(kept, { type: 'inner', id: 'ts-0003-cc' })
    const next = peekReducer(inner, { type: 'pin', id: 'ts-0002-bb', at: ELSEWHERE })
    expect(next).toEqual({ stack: ['ts-0002-bb'], pinned: true, at: ELSEWHERE })
  })
})

describe('an id inside the peek', () => {
  const kept = peekReducer(CLOSED, { type: 'pin', id: 'ts-0001-aa', at: AT })

  test('opens in the same window, over the one before, which Back returns to', () => {
    const inner = peekReducer(kept, { type: 'inner', id: 'ts-0002-bb' })
    expect(inner.stack).toEqual(['ts-0001-aa', 'ts-0002-bb'])
    expect(inner.at).toBe(AT)
    expect(peekReducer(inner, { type: 'back' }).stack).toEqual(['ts-0001-aa'])
  })

  test('opens kept, even from a peek that hover showed, since a click keeps it', () => {
    const hovered = peekReducer(CLOSED, { type: 'hover', id: 'ts-0001-aa', at: AT })
    expect(peekReducer(hovered, { type: 'inner', id: 'ts-0002-bb' }).pinned).toBe(true)
  })

  test('goes as deep as the person goes, and Back goes back one at a time', () => {
    let state: PeekState = kept
    for (const id of ['ts-0002-bb', 'ts-0003-cc', 'ts-0004-dd']) state = peekReducer(state, { type: 'inner', id })
    expect(state.stack).toHaveLength(4)
    state = peekReducer(state, { type: 'back' })
    expect(state.stack.at(-1)).toBe('ts-0003-cc')
    state = peekReducer(state, { type: 'back' })
    state = peekReducer(state, { type: 'back' })
    expect(state.stack).toEqual(['ts-0001-aa'])
  })

  test('Back never leaves the window with nothing in it', () => {
    expect(peekReducer(kept, { type: 'back' })).toBe(kept)
  })

  test('the id already on top is not stacked again', () => {
    expect(peekReducer(kept, { type: 'inner', id: 'ts-0001-aa' })).toBe(kept)
  })

  test('with no window open there is nothing for an inner id to open in', () => {
    expect(peekReducer(CLOSED, { type: 'inner', id: 'ts-0002-bb' })).toBe(CLOSED)
  })
})

describe('closing', () => {
  test('close empties it, whatever is shown or kept, and its place', () => {
    const deep = peekReducer(peekReducer(CLOSED, { type: 'pin', id: 'ts-0001-aa', at: AT }), {
      type: 'inner',
      id: 'ts-0002-bb',
    })
    expect(peekReducer(deep, { type: 'close' })).toEqual({ stack: [], pinned: false, at: null })
  })

  test('closing what is closed changes nothing', () => {
    expect(peekReducer(CLOSED, { type: 'close' })).toBe(CLOSED)
  })
})
