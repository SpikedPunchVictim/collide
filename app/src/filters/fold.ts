// Whether the filter bar is folded to its header, kept for each board in the webview's storage (ts-0190-tz; note 38).
// The store is the Field's (`../field/folds.ts`), passed in; a missing or broken one is a bar that starts open.
import type { FoldStore } from '../field/folds.ts'

const key = (boardId: string) => `collide.field.filters.folded:${boardId}`

export function readBarFolded(store: FoldStore | null, boardId: string): boolean {
  try {
    return store?.getItem(key(boardId)) === 'folded'
  } catch {
    return false
  }
}

export function setBarFolded(store: FoldStore | null, boardId: string, folded: boolean): void {
  try {
    store?.setItem(key(boardId), folded ? 'folded' : 'open')
  } catch {
    // A store that cannot be written is a bar that is open again next time.
  }
}
