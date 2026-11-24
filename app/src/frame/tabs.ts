// The window's five tabs (0.11; note 2 of the board mock), in the order of the keys 1 to 5, which `actions.ts` binds.
// The list is the window's own: `SURFACES` of `../ui/state.ts` is what the in-app agent's `open` tool names, and
// Stage 6b aligns the two.

export const TABS = [
  { id: 'field', name: 'Field', subtitle: 'the whole board' },
  { id: 'tracks', name: 'Tracks', subtitle: "what's running now" },
  { id: 'queue', name: 'Queue', subtitle: 'what needs you' },
  { id: 'planning', name: 'Planning', subtitle: 'what to build next' },
  { id: 'comms', name: 'Comms', subtitle: 'sessions with an agent' },
] as const

export type TabId = (typeof TABS)[number]['id']

export function isTab(value: string): value is TabId {
  return TABS.some((tab) => tab.id === value)
}

/** Whether a key press at `target` is typing into a text box, which the tab keys leave alone. */
export function isTyping(target: { tagName?: string; isContentEditable?: boolean } | null): boolean {
  if (target === null) return false
  return target.isContentEditable === true || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName ?? '')
}
