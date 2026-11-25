// The task drawer beside the board (ts-0184-hv; 0.11, Task drawer; note 8 of the board mock): it reads the selected
// task, keeps which parts are folded and how wide it is across tasks, and closes on Escape. Below 900 px it lies over
// the board. Its edge is dragged, or moved by the arrow keys (named in `../frame/actions.ts`), or reset by a
// double-click. The width is set with a CSS variable, which the content security policy allows; the drawing itself is
// `DrawerView`.
import {
  type CSSProperties,
  type KeyboardEvent,
  type PointerEvent,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react'
import { type FoldStore, browserStore, memoryStore } from '../field/folds.ts'
import type { FieldData, Task } from '../field/types.ts'
import { actionForKey } from '../frame/actions.ts'
import type { OwnerClient } from '../owner/client.ts'
import type { DiffTarget } from './changes.ts'
import { DiffWindow } from './DiffWindow.tsx'
import { DrawerView } from './DrawerView.tsx'
import { readShut, toggleShut } from './folds.ts'
import { useTaskGet } from './load.ts'
import type { BoardNote } from './types.ts'
import { useChanges } from './useChanges.ts'
import {
  BIG_STEP,
  DEFAULT_WIDTH,
  MIN_WIDTH,
  STEP,
  clampWidth,
  maxWidth,
  readWidth,
  writeWidth,
} from './width.ts'

export interface DrawerProps {
  readonly owner: OwnerClient
  readonly boardId: string
  /** The task as the board lists it. */
  readonly shown: Task
  readonly data: FieldData
  readonly board: BoardNote | null
  /** When the Field last changed: the drawer reads its task again with it. */
  readonly stamp: number | null
  /** Where folds and the width are kept. The webview's own, unless a test gives another. */
  readonly store?: FoldStore | null
  readonly onOpenTask: (key: string) => void
  readonly onClose: () => void
}

const roomNow = () => (typeof window === 'undefined' ? 1280 : window.innerWidth)

/** The width the person set, held within what the window allows, and the window's own width to hold it by. */
function useWidth(store: FoldStore | null) {
  const [want, setWant] = useState(() => readWidth(store) ?? DEFAULT_WIDTH)
  const [room, setRoom] = useState(roomNow)
  useEffect(() => {
    const onResize = () => setRoom(roomNow())
    window.addEventListener('resize', onResize)
    return () => window.removeEventListener('resize', onResize)
  }, [])
  const set = (next: number, keep: boolean) => {
    const width = clampWidth(next, roomNow())
    setWant(width)
    if (keep) writeWidth(store, width)
  }
  return { width: clampWidth(want, room), room, set }
}

/** Escape closes the drawer, unless something above it took the key: the link window, or a menu that closed. */
function useEscape(onClose: () => void) {
  useEffect(() => {
    const onKey = (event: globalThis.KeyboardEvent) => {
      if (event.defaultPrevented || event.metaKey || event.ctrlKey || event.altKey) return
      if (actionForKey(event.key, 'drawer') !== 'drawer-close') return
      if (document.querySelector('dialog[open]') !== null) return
      onClose()
    }
    document.addEventListener('keydown', onKey)
    return () => document.removeEventListener('keydown', onKey)
  }, [onClose])
}

export function Drawer({ owner, boardId, shown, data, board, stamp, store, onOpenTask, onClose }: DrawerProps) {
  const kept = useMemo(() => store ?? browserStore() ?? memoryStore(), [store])
  // The folds are read each time, so a change to them is seen with no state of their own to keep in step.
  const [, remember] = useState(0)
  const { got, problem } = useTaskGet(owner, boardId, shown.key, stamp)
  const { width, room, set } = useWidth(kept)
  const shut = readShut(kept)
  // Changes is read while its part is open, for a task the daemon names (a collided key has no changes to ask for).
  const changes = useChanges(owner, boardId, shown.key, got?.task != null && !shut.includes('changes'))
  // The diff window is for one task: another task shown closes it.
  const [diff, setDiff] = useState<{ key: string; target: DiffTarget } | null>(null)
  useEscape(onClose)
  return (
    // Below 900 px it is fixed, and a fixed box follows `align-self` too: left at `start`, it would be as tall as its
    // text, not as the window, and the end of a long task would be out of reach.
    <div
      style={{ '--drawer-w': `${width}px` } as CSSProperties}
      className={
        'sticky top-3 z-40 w-(--drawer-w) shrink-0 self-start ' +
        'max-[900px]:fixed max-[900px]:inset-y-0 max-[900px]:top-0 max-[900px]:right-0 max-[900px]:self-stretch ' +
        'max-[900px]:w-[min(460px,100vw)]'
      }
    >
      <Edge width={width} room={room} onSet={set} />
      <DrawerView
        shown={shown}
        got={got}
        problem={problem}
        data={data}
        board={board}
        shut={shut}
        changes={changes}
        onFlip={(part) => {
          toggleShut(kept, part)
          remember((count) => count + 1)
        }}
        onOpenTask={onOpenTask}
        onOpenFile={(target) => setDiff({ key: shown.key, target })}
        onClose={onClose}
      />
      {diff === null || diff.key !== shown.key ? null : (
        <DiffWindow
          key={`${shown.key}|${JSON.stringify(diff.target.source)}|${diff.target.file.path}`}
          owner={owner}
          boardId={boardId}
          task={shown.key}
          target={diff.target}
          onClosed={() => setDiff(null)}
        />
      )}
    </div>
  )
}

interface EdgeProps {
  readonly width: number
  readonly room: number
  readonly onSet: (width: number, keep: boolean) => void
}

/** The drawer's left edge, which is a splitter: dragged, moved by the arrow keys, and reset by a double-click. */
function Edge({ width, room, onSet }: EdgeProps) {
  // Where a drag began: the pointer's place and the width then, so the drawer follows the pointer by how far it moved.
  const from = useRef<{ x: number; width: number } | null>(null)
  const widthAt = (event: PointerEvent) =>
    from.current === null ? width : from.current.width + (from.current.x - event.clientX)
  const onKeyDown = (event: KeyboardEvent) => {
    const action = actionForKey(event.key, 'drawer-edge')
    if (action === null) return
    event.preventDefault()
    const step = event.shiftKey ? BIG_STEP : STEP
    onSet(width + (action === 'drawer-wider' ? step : -step), true)
  }
  return (
    // A window splitter is focusable and takes the arrow keys; its value is on it for a reader of the page.
    // eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex
    <div
      role="separator"
      aria-orientation="vertical"
      aria-label="Task drawer width"
      aria-valuemin={MIN_WIDTH}
      aria-valuemax={maxWidth(room)}
      aria-valuenow={width}
      tabIndex={0}
      onPointerDown={(event) => {
        event.currentTarget.setPointerCapture(event.pointerId)
        from.current = { x: event.clientX, width }
      }}
      onPointerMove={(event) => {
        if (from.current !== null) onSet(widthAt(event), false)
      }}
      onPointerUp={(event) => {
        if (from.current === null) return
        onSet(widthAt(event), true)
        from.current = null
      }}
      onPointerCancel={() => {
        from.current = null
      }}
      onDoubleClick={() => onSet(DEFAULT_WIDTH, true)}
      onKeyDown={onKeyDown}
      className={
        'absolute inset-y-0 -left-1.5 z-10 w-3 cursor-col-resize touch-none outline-none max-[900px]:hidden ' +
        'after:absolute after:inset-y-0 after:left-1/2 after:w-0.5 after:-translate-x-1/2 after:rounded-full ' +
        'hover:after:bg-ring focus-visible:after:bg-ring'
      }
    />
  )
}
