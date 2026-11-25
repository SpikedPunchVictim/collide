// The key that goes to the filter bar's search (ts-0190-tz). `/` is a window key (`../frame/actions.ts`); like the tab
// keys it is not heard while a text box has the focus, so a `/` can be typed into a search.
import { actionForKey } from '../frame/actions.ts'
import { isTyping } from '../frame/tabs.ts'

interface KeyPress {
  readonly key: string
  readonly metaKey: boolean
  readonly ctrlKey: boolean
  readonly altKey: boolean
  readonly target: EventTarget | { tagName?: string; isContentEditable?: boolean } | null
}

export function isSearchKey(event: KeyPress): boolean {
  const target = event.target as { tagName?: string; isContentEditable?: boolean } | null
  if (event.metaKey || event.ctrlKey || event.altKey || isTyping(target)) return false
  return actionForKey(event.key, 'window') === 'filter-search'
}
