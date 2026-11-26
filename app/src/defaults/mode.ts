// Defaults mode (ts-0215-gt): the window the shell reloaded with `defaults` in the fragment of its address. The
// connection reads only the Default layer, the built-in packs and the built-in views, and the window reads and writes
// no stored state: its folds, its layouts and the drawer's width start as they do on a first run, and stay as they
// were in the webview's storage for the next normal start. A fragment is sent to no server, so the flag stays on the
// page.
import { createContext, useContext, useMemo } from 'react'
import { type FoldStore, browserStore, memoryStore } from '../field/folds.ts'

/** The key of the fragment that says the window is in defaults mode. */
export const DEFAULTS_KEY = 'defaults'

/** Whether a fragment (`location.hash`) has the defaults flag. Other keys (a bridge's token) are not its. */
export function defaultsFromHash(hash: string): boolean {
  return new URLSearchParams(hash.replace(/^#/, '')).has(DEFAULTS_KEY)
}

/** The fragment without the defaults flag, with every other key as it was. Empty when no key is left. */
export function hashWithoutDefaults(hash: string): string {
  const keys = new URLSearchParams(hash.replace(/^#/, ''))
  keys.delete(DEFAULTS_KEY)
  const rest = keys.toString()
  return rest === '' ? '' : `#${rest}`
}

/** The key of the fragment that carries the folder a Reset moved the files to, for the notice of the next start. */
export const RESET_KEY = 'reset'

/** The fragment of the window that starts normally: no defaults flag, and the folder of a Reset when there was one. */
export function hashForNormalStart(hash: string, folder: string | null): string {
  const keys = new URLSearchParams(hashWithoutDefaults(hash).replace(/^#/, ''))
  if (folder !== null) keys.set(RESET_KEY, folder)
  const rest = keys.toString()
  return rest === '' ? '' : `#${rest}`
}

/** The folder a Reset moved the files to, which the fragment of the next start names, or null. */
export function resetFolderFromHash(hash: string): string | null {
  const folder = new URLSearchParams(hash.replace(/^#/, '')).get(RESET_KEY)
  return folder === null || folder === '' ? null : folder
}

/** The fragment with the folder of a Reset taken off, once the person has read the notice. */
export function hashWithoutReset(hash: string): string {
  const keys = new URLSearchParams(hash.replace(/^#/, ''))
  keys.delete(RESET_KEY)
  const rest = keys.toString()
  return rest === '' ? '' : `#${rest}`
}

/**
 * The store the window's state goes to when it is not the webview's own. Defaults mode gives a memory store that lives
 * as long as the page, so nothing stored is read and nothing is written to the webview's storage.
 */
const StateStore = createContext<FoldStore | null>(null)
export const StateStoreProvider = StateStore.Provider

/** The store for the window's stored state: the one a test gave, else defaults mode's, else the webview's own. */
export function useKeptStore(given: FoldStore | null | undefined): FoldStore {
  const mode = useContext(StateStore)
  return useMemo(() => given ?? mode ?? browserStore() ?? memoryStore(), [given, mode])
}

/** The prefix of every key the UI keeps in the webview's storage, as in `collide.drawer.width`. */
export const STATE_PREFIX = 'collide.'

/** What of the storage the reset needs. The webview's `Storage` is this. */
export interface ClearableStorage {
  readonly length: number
  key(index: number): string | null
  removeItem(key: string): void
}

/** Removes every key the UI keeps and no other, and gives how many it removed. */
export function clearStoredState(storage: ClearableStorage): number {
  const keys: string[] = []
  for (let at = 0; at < storage.length; at += 1) {
    const key = storage.key(at)
    if (key !== null && key.startsWith(STATE_PREFIX)) keys.push(key)
  }
  for (const key of keys) storage.removeItem(key)
  return keys.length
}
