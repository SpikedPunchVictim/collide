// Which columns of the Field are folded, kept per board and view in the webview's storage (ts-0181-ft; note 4). Done
// and Dropped start folded; a column the person opens or folds is remembered in `open` or `shut`. The store is passed
// in, so a test needs no browser, and a missing or broken one is a board with the starting folds.
import type { Column } from './model.ts'

export interface FoldStore {
  getItem(key: string): string | null
  setItem(key: string, value: string): void
}

export interface Folds {
  /** Columns the person opened, which would start folded. */
  readonly open: readonly string[]
  /** Columns the person folded, which would start open. */
  readonly shut: readonly string[]
}

const NONE: Folds = { open: [], shut: [] }

/** The key of a board's view. The whole board, which no view names, is the view `null`. */
export function foldKey(boardId: string, view: string | null): string {
  return `collide.field.folds:${boardId}:${view === null ? '' : `view:${view}`}`
}

/** The webview's own storage, or null when there is none to use (a private window, or storage that is blocked). */
export function browserStore(): FoldStore | null {
  try {
    return window.localStorage
  } catch {
    return null
  }
}

/** A store that lives as long as the page, for when the webview's own cannot be used. */
export function memoryStore(): FoldStore {
  const items = new Map<string, string>()
  return { getItem: (key) => items.get(key) ?? null, setItem: (key, value) => void items.set(key, value) }
}

function strings(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((each): each is string => typeof each === 'string') : []
}

export function readFolds(store: FoldStore | null, boardId: string, view: string | null): Folds {
  try {
    const text = store?.getItem(foldKey(boardId, view)) ?? null
    if (text === null) return NONE
    const parsed: unknown = JSON.parse(text)
    if (typeof parsed !== 'object' || parsed === null) return NONE
    const { open, shut } = parsed as { open?: unknown; shut?: unknown }
    return { open: strings(open), shut: strings(shut) }
  } catch {
    return NONE
  }
}

/** Whether a column shows folded: a fold the person made wins, and else Done and Dropped are. */
export function isShut(column: Column, folds: Folds): boolean {
  if (folds.shut.includes(column.key)) return true
  return !folds.open.includes(column.key) && (column.category === 'done' || column.category === 'dropped')
}

/** Folds a column that shows, or opens one that is folded, keeps it, and gives the folds that now hold. */
export function toggleFold(store: FoldStore | null, boardId: string, view: string | null, column: Column): Folds {
  const before = readFolds(store, boardId, view)
  const gone = (list: readonly string[]) => list.filter((key) => key !== column.key)
  const after: Folds = isShut(column, before)
    ? { open: [...gone(before.open), column.key], shut: gone(before.shut) }
    : { open: gone(before.open), shut: [...gone(before.shut), column.key] }
  try {
    store?.setItem(foldKey(boardId, view), JSON.stringify(after))
  } catch {
    // Storage that cannot be written leaves the fold for this session only.
  }
  return after
}
