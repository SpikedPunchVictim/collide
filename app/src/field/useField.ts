// The Field's state for one board (ts-0181-ft): its views, the one selected, and what the last read of it gave. The
// page and the view picker in the trail share one, so `App` holds it. A board's view is forgotten when the board
// changes. The board is read again when the daemon says a task changed, and the cards that changed flash
// (ts-0182-ad; `live.ts`).
import { useCallback, useEffect, useRef, useState } from 'react'
import type { OwnerClient } from '../owner/client.ts'
import { FieldFeed, type FieldRead } from './live.ts'
import { loadViews } from './load.ts'
import type { FieldData, ViewEntry } from './types.ts'
import { inlineView } from './walk.ts'

export interface FieldState {
  /** The views of the board, built-in first. Empty until they are read. */
  readonly views: readonly ViewEntry[]
  /** The selected view's name, or null for the whole board. */
  readonly view: string | null
  readonly select: (view: string | null) => void
  /** The collection or task the run order and graph walk toward, or null when none is picked. */
  readonly goal: string | null
  readonly chooseGoal: (goal: string | null) => void
  /** What the last read gave, for the board and view shown, or null while the first read is on its way. */
  readonly data: FieldData | null
  /** The words of a read that failed. */
  readonly problem: string | null
  /** The cards that changed in the last reads and are still flashing, by card id. */
  readonly flashed: ReadonlySet<string>
  /** When the board last changed, in ms since the epoch. Null before the first read. */
  readonly updatedAt: number | null
}

interface Read extends FieldRead {
  readonly boardId: string
  readonly view: string | null
}

const NONE: ReadonlySet<string> = new Set()

export function useField(owner: OwnerClient, boardId: string | null): FieldState {
  const [pick, setPick] = useState<{ boardId: string | null; view: string | null }>({ boardId, view: null })
  const [views, setViews] = useState<{ boardId: string; views: readonly ViewEntry[] } | null>(null)
  const [read, setRead] = useState<Read | null>(null)
  const view = pick.boardId === boardId ? pick.view : null
  const select = useCallback((next: string | null) => setPick({ boardId, view: next }), [boardId])
  // The goal of the run order and graph, for a view with none of its own. It is the view's, so it goes with the view.
  const [chosen, setChosen] = useState<{ boardId: string | null; view: string | null; goal: string | null }>({
    boardId,
    view,
    goal: null,
  })
  const goal = chosen.boardId === boardId && chosen.view === view ? chosen.goal : null
  const chooseGoal = useCallback((next: string | null) => setChosen({ boardId, view, goal: next }), [boardId, view])

  useEffect(() => {
    if (boardId === null) return
    let current = true
    loadViews(owner, boardId).then(
      (found) => current && setViews({ boardId, views: found }),
      // The picker then lists only the whole board; the page's own read says what is wrong.
      () => current && setViews({ boardId, views: [] }),
    )
    return () => {
      current = false
    }
  }, [owner, boardId])

  // What the selected view groups by decides a card's column, so a read says which cards moved by it.
  const groupBy = useRef<string | null>(null)
  groupBy.current = views?.views.find((each) => each.view.name === view)?.view.layout?.groupBy ?? null
  // The view a goal's walk is asked of is read at each read, so the views loading does not start the feed again.
  const entry = useRef<ViewEntry | undefined>(undefined)
  entry.current = views?.views.find((each) => each.view.name === view)

  useEffect(() => {
    if (boardId === null) return
    const feed = new FieldFeed({
      owner,
      boardId,
      view,
      groupBy: () => groupBy.current,
      walk: () => (goal === null ? null : inlineView(entry.current, goal)),
      onRead: (found) => setRead({ boardId, view, ...found }),
    })
    void feed.start()
    return () => feed.stop()
  }, [owner, boardId, view, goal])

  const shown = read !== null && read.boardId === boardId && read.view === view ? read : null
  return {
    views: views !== null && views.boardId === boardId ? views.views : [],
    view,
    select,
    goal,
    chooseGoal,
    data: shown?.data ?? null,
    problem: shown?.problem ?? null,
    flashed: shown?.flashed ?? NONE,
    updatedAt: shown?.updatedAt ?? null,
  }
}
