// The id peek (ts-0189-fg; 0.11, Task drawer, "An id peeks"; note 34 of the board mock). `Peek` is provided once, near
// the top of the app. It holds the book of ids for the board (`book.ts`) and the window: the pointer over an id shows
// it after a short wait, a click keeps it open, an id inside it opens in the same window with Back, and Escape or a
// click away closes it. The window is not modal and has no dialog of its own, so it takes Escape itself, before the
// drawer does, with a listener in the capture phase that the drawer's own never sees.
import { type ReactNode, useCallback, useEffect, useMemo, useReducer, useRef, useSyncExternalStore } from 'react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { blockingLinks, categoryWords, linkWords } from '../drawer/model.ts'
import { DOT, TEXT } from '../field/colours.ts'
import { sourceOf, tailOf } from '../field/model.ts'
import type { SourceDecl } from '../field/types.ts'
import { Markdown } from '../markdown/Markdown.tsx'
import type { OwnerClient } from '../owner/client.ts'
import { IdBook } from './book.ts'
import { IdBookContext, InPeekContext, type PeekActions, PeekActionsContext } from './context.ts'
import { compilePatterns } from './ids.ts'
import { type Anchor, CLOSED, peekReducer } from './peek.ts'

/** How long the pointer rests on an id before the peek shows, and how long it is off before the peek goes. */
export const SHOW_AFTER_MS = 350
export const HIDE_AFTER_MS = 250

const WIDTH = 380
const GAP = 6
const EDGE = 8

export interface PeekProps {
  readonly owner: OwnerClient
  /** The board shown, or null when there is none: no word is then an id. */
  readonly boardId: string | null
  /** The board's sources: their patterns say which words could be ids. */
  readonly sources: readonly SourceDecl[]
  /** When the Field last changed: the ids are asked again with it, so a peek never shows an old task. */
  readonly stamp: number | null
  readonly children: ReactNode
}

const anchorOf = (element: Element): Anchor => {
  const box = element.getBoundingClientRect()
  return { left: box.left, top: box.top, bottom: box.bottom }
}

export function Peek({ owner, boardId, sources, stamp, children }: PeekProps) {
  const patterns = useMemo(() => compilePatterns(sources), [sources])
  // A new book when the board or its reading changes: an id is asked once for as long as the board is as it was.
  const book = useMemo(
    () => (boardId === null ? null : new IdBook(owner, boardId, patterns)),
    // eslint-disable-next-line react-hooks/exhaustive-deps -- the stamp is what makes the book new
    [owner, boardId, patterns, stamp],
  )
  const [state, dispatch] = useReducer(peekReducer, CLOSED)
  const latest = useRef(state)
  latest.current = state
  const showTimer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const hideTimer = useRef<ReturnType<typeof setTimeout> | null>(null)

  const clearTimers = useCallback(() => {
    if (showTimer.current !== null) clearTimeout(showTimer.current)
    if (hideTimer.current !== null) clearTimeout(hideTimer.current)
    showTimer.current = null
    hideTimer.current = null
  }, [])
  useEffect(() => clearTimers, [clearTimers])
  // Another board, or the board read again, may no longer have what the peek shows.
  useEffect(() => {
    clearTimers()
    dispatch({ type: 'close' })
  }, [book, clearTimers])

  const actions = useMemo<PeekActions>(
    () => ({
      hover: (id, from) => {
        clearTimers()
        if (latest.current.pinned) return
        const at = anchorOf(from)
        showTimer.current = setTimeout(() => dispatch({ type: 'hover', id, at }), SHOW_AFTER_MS)
      },
      leave: () => {
        clearTimers()
        hideTimer.current = setTimeout(() => {
          if (!latest.current.pinned) dispatch({ type: 'close' })
        }, HIDE_AFTER_MS)
      },
      pin: (id, from) => {
        clearTimers()
        dispatch({ type: 'pin', id, at: anchorOf(from) })
      },
      inner: (id) => dispatch({ type: 'inner', id }),
    }),
    [clearTimers],
  )

  const open = state.stack.length > 0
  useEffect(() => {
    if (!open) return
    // Capture phase: the key is taken before the drawer's listener, which then never sees it.
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== 'Escape' || event.defaultPrevented) return
      event.preventDefault()
      event.stopImmediatePropagation()
      clearTimers()
      dispatch({ type: 'close' })
    }
    const onClick = (event: MouseEvent) => {
      const target = event.target instanceof Element ? event.target : null
      if (target?.closest('[data-peek], [data-id]') != null) return
      clearTimers()
      dispatch({ type: 'close' })
    }
    document.addEventListener('keydown', onKey, true)
    document.addEventListener('click', onClick, true)
    return () => {
      document.removeEventListener('keydown', onKey, true)
      document.removeEventListener('click', onClick, true)
    }
  }, [open, clearTimers])

  return (
    <IdBookContext value={book}>
      <PeekActionsContext value={actions}>
        {children}
        {book === null || !open ? null : (
          <PeekWindow
            book={book}
            sources={sources}
            stack={state.stack}
            at={state.at}
            pinned={state.pinned}
            onEnter={clearTimers}
            onLeave={actions.leave}
            onBack={() => dispatch({ type: 'back' })}
            onClose={() => dispatch({ type: 'close' })}
          />
        )}
      </PeekActionsContext>
    </IdBookContext>
  )
}

interface PeekWindowProps {
  readonly book: IdBook
  readonly sources: readonly SourceDecl[]
  readonly stack: readonly string[]
  readonly at: Anchor | null
  readonly pinned: boolean
  readonly onEnter: () => void
  readonly onLeave: () => void
  readonly onBack: () => void
  readonly onClose: () => void
}

/** Where the window goes: under the id, or over it when the id is low on the screen, and not off either side. */
function placement(at: Anchor | null): { left: number; top?: number; bottom?: number } {
  const room = typeof window === 'undefined' ? { w: 1280, h: 800 } : { w: window.innerWidth, h: window.innerHeight }
  const left = Math.max(EDGE, Math.min(at?.left ?? EDGE, room.w - WIDTH - EDGE))
  if (at === null) return { left, top: EDGE }
  return at.bottom > room.h * 0.55 ? { left, bottom: room.h - at.top + GAP } : { left, top: at.bottom + GAP }
}

function PeekWindow({ book, sources, stack, at, pinned, onEnter, onLeave, onBack, onClose }: PeekWindowProps) {
  const id = stack[stack.length - 1]!
  // Drawn again when an id is found, which is how a peek that opened before its task was read gets it.
  useSyncExternalStore(book.subscribe, book.linked, book.linked)
  useEffect(() => {
    void book.ensure([id])
  }, [book, id])
  const got = book.get(id)
  const task = got?.task ?? null
  const source = task === null ? undefined : sources.find((each) => each.name === sourceOf(task.key))
  const waits = got === undefined || got === null ? [] : (blockingLinks(got, source)?.waits ?? [])
  return (
    <InPeekContext value={true}>
      <div
        role="dialog"
        aria-label={`Task ${id}`}
        data-peek={id}
        onMouseEnter={onEnter}
        onMouseLeave={() => {
          if (!pinned) onLeave()
        }}
        style={{ width: WIDTH, ...placement(at) }}
        className={cn(
          'fixed z-50 flex max-h-[min(24rem,60vh)] max-w-[calc(100vw-1rem)] flex-col rounded-lg border bg-popover',
          'text-sm text-popover-foreground shadow-lg',
        )}
      >
        <header className="flex shrink-0 items-center gap-2 border-b px-3 py-2">
          {stack.length > 1 ? (
            <Button variant="ghost" size="sm" onClick={onBack}>
              Back
            </Button>
          ) : null}
          <span className="min-w-0 flex-1 font-mono text-xs break-all text-muted-foreground">{id}</span>
          {pinned ? (
            <Button variant="ghost" size="sm" onClick={onClose}>
              Close
            </Button>
          ) : null}
        </header>
        {task === null ? (
          <p role="status" className="px-3 py-2 text-muted-foreground">
            {got === null ? 'No task has this id.' : 'Reading the task…'}
          </p>
        ) : (
          <>
            <div className="flex shrink-0 flex-col gap-1 px-3 py-2">
              <div className="flex items-center gap-2">
                <Badge variant="outline" className={cn('font-normal', TEXT[task.state.category])}>
                  <span className={cn('size-2 rounded-full', DOT[task.state.category])} />
                  {task.state.status}
                </Badge>
                <span className="text-xs text-muted-foreground">{categoryWords(task.state.category)}</span>
              </div>
              <b className="font-semibold break-words">{task.title}</b>
              <p className="text-xs text-muted-foreground">
                {waits.length === 0
                  ? 'Waits on nothing.'
                  : `Waits on ${waits.map((link) => `${linkWords(link)} (${tailOf(link.key)})`).join(', ')}.`}
              </p>
            </div>
            <div data-peek-body className="min-h-0 flex-1 overflow-y-auto border-t px-3 py-2">
              {task.body === undefined || task.body === null || task.body.trim() === '' ? (
                <span className="text-muted-foreground">No description.</span>
              ) : (
                <Markdown text={task.body} />
              )}
            </div>
          </>
        )}
      </div>
    </InPeekContext>
  )
}
