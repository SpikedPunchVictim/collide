// The Field's state for one board (ts-0181-ft): its views, the one selected, and what the last read of it gave. The
// page and the view picker in the trail share one, so `App` holds it. A board's view is forgotten when the board
// changes. The board is read again when the daemon says a task changed, and the cards that changed flash
// (ts-0182-ad; `live.ts`).
import { useCallback, useEffect, useRef, useState } from 'react'
import { type Filters, NO_FILTERS, celOf } from '../filters/model.ts'
import type { OwnerClient } from '../owner/client.ts'
import type { ViewInput } from '../owner/generated.ts'
import { FieldFeed, type FieldRead } from './live.ts'
import { loadBoardViews } from './load.ts'
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
  /**
   * What the filter bar has chosen for this view: the epics, where a task stands, its source, its labels and a search.
   * It is the view's, so it goes with the view. The Epic box and the epics strip both set `epics`.
   */
  readonly filters: Filters
  readonly setFilters: (filters: Filters) => void
  /** The board's line has a Collide folder, so a personal view can be shared into it. */
  readonly shared: boolean
  /** The views have been read, so `shared` is the daemon's answer and not its default (`ts-0194-qp`). */
  readonly sharedRead: boolean
  /** Reads the board and its views again, as after the repo's trust changed (`ts-0194-qp`). */
  readonly reload: () => void
  /** Reads the board's views again, as when the picker is opened. */
  readonly reloadViews: () => void
  /** Writes a view of the person's own, lists it and selects it. A refusal is thrown with the daemon's words. */
  readonly saveView: (view: ViewInput) => Promise<void>
  /** Writes a view to the repo's Collide folder, for the person to commit, and gives the file. */
  readonly shareView: (view: ViewInput) => Promise<string>
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

/** The filter bar's choices, for the board and view they were made on. */
interface ChosenFilters {
  readonly boardId: string | null
  readonly view: string | null
  readonly filters: Filters
}

/** A search is read this long after the last key, so typing is not a read for each letter. */
export const SEARCH_DEBOUNCE_MS = 250

export function useField(owner: OwnerClient, boardId: string | null): FieldState {
  const [pick, setPick] = useState<{ boardId: string | null; view: string | null }>({ boardId, view: null })
  const [views, setViews] = useState<{ boardId: string; views: readonly ViewEntry[]; shared: boolean } | null>(null)
  // Counted up to read the views again, after one is saved or shared.
  const [listed, setListed] = useState(0)
  // Counted up to start the feed again, which reads the board afresh.
  const [fed, setFed] = useState(0)
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

  // The filter bar's choices are the view's, so they are forgotten with it.
  const [chosenFilters, setChosenFilters] = useState<ChosenFilters>({
    boardId,
    view,
    filters: NO_FILTERS,
  })
  const filters = chosenFilters.boardId === boardId && chosenFilters.view === view ? chosenFilters.filters : NO_FILTERS
  const setFilters = useCallback(
    (next: Filters) => setChosenFilters({ boardId, view, filters: next }),
    [boardId, view],
  )
  // What the feed reads with: the choices as they stand, except that a search waits for the typing to stop.
  const expression = celOf(filters)
  const [applied, setApplied] = useState<{ boardId: string | null; view: string | null; expression: string | null }>({
    boardId,
    view,
    expression: null,
  })
  const appliedText = useRef('')
  useEffect(() => {
    const text = filters.text.trim()
    const wait = text === appliedText.current ? 0 : SEARCH_DEBOUNCE_MS
    const timer = setTimeout(() => {
      appliedText.current = text
      setApplied({ boardId, view, expression })
    }, wait)
    return () => clearTimeout(timer)
    // `filters.text` is in `expression`.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [boardId, view, expression])
  const filter = applied.boardId === boardId && applied.view === view ? applied.expression : null

  useEffect(() => {
    if (boardId === null) return
    let current = true
    loadBoardViews(owner, boardId).then(
      (found) => current && setViews({ boardId, ...found }),
      // The picker then lists only the whole board; the page's own read says what is wrong.
      () => current && setViews({ boardId, views: [], shared: false }),
    )
    return () => {
      current = false
    }
  }, [owner, boardId, listed])

  const reloadViews = useCallback(() => setListed((count) => count + 1), [])
  const reload = useCallback(() => {
    setListed((count) => count + 1)
    setFed((count) => count + 1)
  }, [])
  const saveView = useCallback(
    async (input: ViewInput) => {
      if (boardId === null) return
      await owner.rpc('task.save_view', { board: boardId, target: 'personal', view: input })
      setListed((count) => count + 1)
      setPick({ boardId, view: input.name })
    },
    [owner, boardId],
  )
  const shareView = useCallback(
    async (input: ViewInput) => {
      if (boardId === null) throw new Error('no board is open')
      const { path } = await owner.rpc('task.save_view', { board: boardId, target: 'repo', view: input })
      setListed((count) => count + 1)
      return path
    },
    [owner, boardId],
  )

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
      filter,
      onRead: (found) => setRead({ boardId, view, ...found }),
    })
    void feed.start()
    return () => feed.stop()
  }, [owner, boardId, view, goal, filter, fed])

  const shown = read !== null && read.boardId === boardId && read.view === view ? read : null
  return {
    views: views !== null && views.boardId === boardId ? views.views : [],
    view,
    select,
    goal,
    chooseGoal,
    filters,
    setFilters,
    shared: views !== null && views.boardId === boardId && views.shared,
    sharedRead: views !== null && views.boardId === boardId,
    reload,
    reloadViews,
    saveView,
    shareView,
    data: shown?.data ?? null,
    problem: shown?.problem ?? null,
    flashed: shown?.flashed ?? NONE,
    updatedAt: shown?.updatedAt ?? null,
  }
}
