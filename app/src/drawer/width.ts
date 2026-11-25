// How wide the drawer is (0.11, Task drawer; note 8 of the board mock): between 360 px and 70% of the window, 460 px
// until the person sets it, and kept across tasks in the webview's storage. The functions take the store, so a test
// needs no browser, and a store that fails is a drawer of the default width.
import type { FoldStore } from '../field/folds.ts'

export const MIN_WIDTH = 360
export const DEFAULT_WIDTH = 460
/** What an arrow key on the edge moves it by, and what it moves by with Shift. */
export const STEP = 40
export const BIG_STEP = 120
export const WIDTH_KEY = 'collide.drawer.width'

/** The widest it may be in a window `room` px wide: 70% of it, though never below the narrowest. */
export const maxWidth = (room: number) => Math.max(MIN_WIDTH, Math.round(room * 0.7))

export const clampWidth = (width: number, room: number) =>
  Math.min(maxWidth(room), Math.max(MIN_WIDTH, Math.round(width)))

/** The width the person set, or null when there is none or what is kept is not a width. */
export function readWidth(store: FoldStore | null): number | null {
  try {
    const width = Number(store?.getItem(WIDTH_KEY) ?? 'none')
    return Number.isFinite(width) && width > 0 ? width : null
  } catch {
    return null
  }
}

export function writeWidth(store: FoldStore | null, width: number): void {
  try {
    store?.setItem(WIDTH_KEY, String(Math.round(width)))
  } catch {
    // Storage that cannot be written leaves the width for this session only.
  }
}
