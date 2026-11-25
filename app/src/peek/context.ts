// What the id links and the peek share (ts-0189-fg; note 34 of the board mock). `Peek` provides all three once, near
// the top of the app; `Markdown` and `IdRef` only read them, so text drawn without a provider shows no id link.
import { createContext } from 'react'
import type { IdBook } from './book.ts'

/** The ids this board's text names, or null where there is no board: then no word is a link. */
export const IdBookContext = createContext<IdBook | null>(null)

/** What an id link does with the pointer. The element is where the peek's window is put. */
export interface PeekActions {
  readonly hover: (id: string, from: Element) => void
  readonly leave: () => void
  readonly pin: (id: string, from: Element) => void
  /** A click on an id inside the peek: it opens in the same window. */
  readonly inner: (id: string) => void
}

const NOTHING = () => {
  // Without a provider (a test that draws text alone) the pointer does nothing.
}

export const PeekActionsContext = createContext<PeekActions>({
  hover: NOTHING,
  leave: NOTHING,
  pin: NOTHING,
  inner: NOTHING,
})

/** True inside the peek's own window, where a click on an id goes one deeper instead of opening a new peek. */
export const InPeekContext = createContext(false)
