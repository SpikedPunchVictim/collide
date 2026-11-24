// Arrow keys between the cards of the Field (ts-0181-ft): from a card, to the one below or above it in its column, or
// to the nearest card of the next column that has one.
export type Direction = 'up' | 'down' | 'left' | 'right'

/** Where a card is: its column, and its place in that column. */
export interface Place {
  readonly column: number
  readonly row: number
}

/**
 * Where the arrow takes the focus. `sizes` is how many cards each column shows, a folded column showing none. Up and
 * down stop at the ends of a column. Left and right skip columns with no card, keep the row when the next column is as
 * long, and else land on its last card. Staying where it is means there is nowhere to go.
 */
export function neighbour(sizes: readonly number[], from: Place, direction: Direction): Place {
  if (direction === 'up') return { column: from.column, row: Math.max(0, from.row - 1) }
  if (direction === 'down') return { column: from.column, row: Math.min((sizes[from.column] ?? 1) - 1, from.row + 1) }
  const step = direction === 'right' ? 1 : -1
  for (let column = from.column + step; column >= 0 && column < sizes.length; column += step) {
    const size = sizes[column] ?? 0
    if (size > 0) return { column, row: Math.min(from.row, size - 1) }
  }
  return from
}
