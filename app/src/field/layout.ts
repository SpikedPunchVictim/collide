// Which layout the Field shows a view in: the board's columns, the run order, or the graph (ts-0187-hb; 0.10, Views,
// `layout.default`). A view starts in its own `layout.default`, else the board. What the person picks is kept per board
// and view in the webview's storage, as the folds are (`folds.ts`), and wins from then on. A store that is missing or
// broken is a view that starts as it says.
import type { ViewInput } from '../owner/generated.ts'
import type { FoldStore } from './folds.ts'
import type { ViewEntry } from './types.ts'

export type LayoutKind = NonNullable<NonNullable<ViewInput['layout']>['default']>

/** The layouts, in the order the switch lists them, with the words it shows. */
export const LAYOUTS: readonly { readonly id: LayoutKind; readonly word: string }[] = [
  { id: 'board', word: 'Board' },
  { id: 'run-order', word: 'Run order' },
  { id: 'graph', word: 'Graph' },
]

const isLayout = (value: unknown): value is LayoutKind => LAYOUTS.some((each) => each.id === value)

/** The key of a board's view. The whole board, which no view names, is the view `null`. */
export function layoutKey(boardId: string, view: string | null): string {
  return `collide.field.layout:${boardId}:${view === null ? '' : `view:${view}`}`
}

/** The layout the person kept for a view, if one is kept and is one. */
function kept(store: FoldStore | null, boardId: string, view: string | null): LayoutKind | null {
  try {
    const text = store?.getItem(layoutKey(boardId, view)) ?? null
    return isLayout(text) ? text : null
  } catch {
    return null
  }
}

/** The layout a view shows in: the one kept, else its own default, else the board. */
export function layoutOf(
  store: FoldStore | null,
  boardId: string,
  view: string | null,
  entry: ViewEntry | undefined,
): LayoutKind {
  return kept(store, boardId, view) ?? entry?.view.layout?.default ?? 'board'
}

/** Keeps the person's choice. Storage that cannot be written leaves it for this session only. */
export function writeLayout(store: FoldStore | null, boardId: string, view: string | null, layout: LayoutKind): void {
  try {
    store?.setItem(layoutKey(boardId, view), layout)
  } catch {
    // The choice holds until the page is closed.
  }
}
