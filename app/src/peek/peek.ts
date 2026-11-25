// What the id peek is showing (ts-0189-fg; note 34 of the board mock), as a reducer: the ids opened in it, the oldest
// first, whether a click has kept it open, and where the id it came from is on the page. The timers of a hover and the
// pointer are the component's; nothing here reads the page.

/** Where the id that opened the peek is, in the window's own coordinates. */
export interface Anchor {
  readonly left: number
  readonly top: number
  readonly bottom: number
}

export interface PeekState {
  /** The ids opened in the window, the first the one a person pointed at. The last is shown. Empty is closed. */
  readonly stack: readonly string[]
  /** True once a click kept it: a pointer leaving no longer closes it. */
  readonly pinned: boolean
  readonly at: Anchor | null
}

export type PeekAction =
  | { readonly type: 'hover'; readonly id: string; readonly at: Anchor }
  | { readonly type: 'pin'; readonly id: string; readonly at: Anchor }
  | { readonly type: 'inner'; readonly id: string }
  | { readonly type: 'back' }
  | { readonly type: 'close' }

export const CLOSED: PeekState = { stack: [], pinned: false, at: null }

export function peekReducer(state: PeekState, action: PeekAction): PeekState {
  switch (action.type) {
    case 'hover':
      return state.pinned ? state : { stack: [action.id], pinned: false, at: action.at }
    case 'pin':
      return { stack: [action.id], pinned: true, at: action.at }
    case 'inner':
      // A click on an id inside the window keeps it too, and goes one deeper: Back returns to the one before.
      if (state.stack.length === 0 || state.stack[state.stack.length - 1] === action.id) return state
      return { ...state, stack: [...state.stack, action.id], pinned: true }
    case 'back':
      return state.stack.length > 1 ? { ...state, stack: state.stack.slice(0, -1) } : state
    case 'close':
      return state.stack.length === 0 ? state : CLOSED
  }
}
