// How the top bar names a board: the folder's own name, and the default line, which is the first of its lines.
import type { BoardSummary } from '../loop/types.ts'

export function boardName(board: BoardSummary): string {
  return board.root.split(/[\\/]/).filter((part) => part !== '').pop() ?? board.root
}

/** The board's default line, or null when it lists none. */
export function defaultLine(board: BoardSummary): string | null {
  return board.lines[0]?.ref_name ?? null
}
