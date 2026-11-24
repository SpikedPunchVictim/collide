import { describe, expect, test } from 'vitest'
import { neighbour } from '../src/field/navigation.ts'

describe('arrow keys between cards', () => {
  const sizes = [3, 0, 2, 0, 1]

  test('down and up move within the column, and stop at its ends', () => {
    expect(neighbour(sizes, { column: 0, row: 0 }, 'down')).toEqual({ column: 0, row: 1 })
    expect(neighbour(sizes, { column: 0, row: 2 }, 'down')).toEqual({ column: 0, row: 2 })
    expect(neighbour(sizes, { column: 0, row: 2 }, 'up')).toEqual({ column: 0, row: 1 })
    expect(neighbour(sizes, { column: 0, row: 0 }, 'up')).toEqual({ column: 0, row: 0 })
  })

  test('right goes to the next column that has a card, skipping empty and folded ones', () => {
    expect(neighbour(sizes, { column: 0, row: 1 }, 'right')).toEqual({ column: 2, row: 1 })
    expect(neighbour(sizes, { column: 2, row: 0 }, 'right')).toEqual({ column: 4, row: 0 })
  })

  test('left goes to the previous column that has a card', () => {
    expect(neighbour(sizes, { column: 4, row: 0 }, 'left')).toEqual({ column: 2, row: 0 })
    expect(neighbour(sizes, { column: 2, row: 1 }, 'left')).toEqual({ column: 0, row: 1 })
  })

  test('a shorter column takes the focus to its last card', () => {
    expect(neighbour(sizes, { column: 0, row: 2 }, 'right')).toEqual({ column: 2, row: 1 })
    expect(neighbour(sizes, { column: 2, row: 1 }, 'right')).toEqual({ column: 4, row: 0 })
  })

  test('at the last column, right stays; at the first, left stays', () => {
    expect(neighbour(sizes, { column: 4, row: 0 }, 'right')).toEqual({ column: 4, row: 0 })
    expect(neighbour(sizes, { column: 0, row: 0 }, 'left')).toEqual({ column: 0, row: 0 })
  })
})
