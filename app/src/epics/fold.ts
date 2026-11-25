// Whether the epics strip is folded to its header, kept for each board and view in the webview's storage (ts-0216-ve;
// note 58). The store is the Field's (`../field/folds.ts`), passed in; a missing or broken one is a strip that starts
// open. The whole board, which no view names, is the view `null`.
import type { FoldStore } from '../field/folds.ts'

const key = (boardId: string, view: string | null) =>
  `collide.field.epics.folded:${boardId}:${view === null ? '' : `view:${view}`}`

export function readStripFolded(store: FoldStore | null, boardId: string, view: string | null): boolean {
  try {
    return store?.getItem(key(boardId, view)) === 'folded'
  } catch {
    return false
  }
}

export function setStripFolded(store: FoldStore | null, boardId: string, view: string | null, folded: boolean): void {
  try {
    store?.setItem(key(boardId, view), folded ? 'folded' : 'open')
  } catch {
    // A store that cannot be written is a strip that is open again next time.
  }
}
