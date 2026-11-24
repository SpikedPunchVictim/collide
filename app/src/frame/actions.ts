// The app's named actions, each with its key (Stage 6a, Named actions). Keys are bound from this list and nowhere
// else, so a key is changed here once. The palette (Stage 9d) and the in-app agent's tools (Stage 6b) use the names.
// A later item adds its actions to the list.
import type { TabId } from './tabs.ts'

/** Where a key is heard: anywhere in the window, or on the Field's board. */
export type ActionScope = 'window' | 'field'

export interface NamedAction {
  /** The name the palette and the agent use. */
  readonly name: string
  /** What it does, in a few words. */
  readonly label: string
  /** The `KeyboardEvent.key` that does it. */
  readonly key: string
  readonly scope: ActionScope
}

export const ACTIONS = [
  { name: 'show-field', label: 'Show the Field', key: '1', scope: 'window' },
  { name: 'show-tracks', label: 'Show Tracks', key: '2', scope: 'window' },
  { name: 'show-queue', label: 'Show the Queue', key: '3', scope: 'window' },
  { name: 'show-planning', label: 'Show Planning', key: '4', scope: 'window' },
  { name: 'show-comms', label: 'Show Comms', key: '5', scope: 'window' },
  { name: 'card-down', label: 'Go to the card below', key: 'ArrowDown', scope: 'field' },
  { name: 'card-up', label: 'Go to the card above', key: 'ArrowUp', scope: 'field' },
  { name: 'card-right', label: 'Go to the next column', key: 'ArrowRight', scope: 'field' },
  { name: 'card-left', label: 'Go to the previous column', key: 'ArrowLeft', scope: 'field' },
  { name: 'card-open', label: 'Select the card', key: 'Enter', scope: 'field' },
] as const satisfies readonly NamedAction[]

const SHOW = 'show-'

export type ActionName = (typeof ACTIONS)[number]['name']

/** The action a key does in a scope, or null. */
export function actionForKey(key: string, scope: ActionScope): ActionName | null {
  return ACTIONS.find((action) => action.scope === scope && action.key === key)?.name ?? null
}

/** The key of an action, as the interface shows it. */
export function keyOf(name: ActionName): string {
  return ACTIONS.find((action) => action.name === name)!.key
}

/** The tab an action shows, or null for an action that shows none. */
export function tabOfAction(name: ActionName | null): TabId | null {
  return name !== null && name.startsWith(SHOW) ? (name.slice(SHOW.length) as TabId) : null
}
