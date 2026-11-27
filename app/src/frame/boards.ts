// How the top bar names a board: the folder's own name, and the default line, which is the first of its lines.
import type { BoardSummary, MissingReason } from '../loop/types.ts'

export function boardName(board: BoardSummary): string {
  return board.root.split(/[\\/]/).filter((part) => part !== '').pop() ?? board.root
}

/** The board's default line, or null when it lists none. */
export function defaultLine(board: BoardSummary): string | null {
  return board.lines[0]?.ref_name ?? null
}

/** The board's folder is gone or is no longer its checkout. Nothing of the board is deleted; it can be located. */
export function isMissing(board: BoardSummary): boolean {
  return board.missing === true
}

/** Why a missing board is missing. A daemon from before reasons gave none, and knew only a folder that is gone. */
export function missingReason(board: BoardSummary): MissingReason {
  return board.missing_reason ?? 'gone'
}
