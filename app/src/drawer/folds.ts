// Which parts of the drawer are folded (0.11, Task drawer): one set for every task, kept in the webview's storage, so
// a part the person folds stays folded on the next task. Details for engineers starts folded. The store is passed in,
// and one that fails is the starting folds.
import type { FoldStore } from '../field/folds.ts'

export const FOLDS_KEY = 'collide.drawer.folds'
export const STARTS_SHUT: readonly string[] = ['details']

export function readShut(store: FoldStore | null): string[] {
  try {
    const text = store?.getItem(FOLDS_KEY) ?? null
    if (text === null) return [...STARTS_SHUT]
    const parsed: unknown = JSON.parse(text)
    const shut = typeof parsed === 'object' && parsed !== null ? (parsed as { shut?: unknown }).shut : undefined
    if (!Array.isArray(shut)) return [...STARTS_SHUT]
    return shut.filter((each): each is string => typeof each === 'string')
  } catch {
    return [...STARTS_SHUT]
  }
}

/** Folds an open part, or opens a folded one, keeps it, and gives the parts that are now folded. */
export function toggleShut(store: FoldStore | null, part: string): string[] {
  const before = readShut(store)
  const after = before.includes(part) ? before.filter((each) => each !== part) : [...before, part]
  try {
    store?.setItem(FOLDS_KEY, JSON.stringify({ shut: after }))
  } catch {
    // Storage that cannot be written leaves the fold for this session only.
  }
  return after
}
