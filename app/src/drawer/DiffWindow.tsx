// The window that shows one file's diff (ts-0189-fg; 0.11, Task drawer, Changes; note 29 of the board mock), opened
// from a file of the Changes part. "Changes" shows the patch, "Whole file" the file after the changes; each line is
// numbered, an added or removed line is marked, and the window scrolls to the first change. It is the browser's own
// modal `<dialog>`, as the link window is: it holds the focus, Escape and Close shut it, and the drawer's own Escape
// sees it open and leaves the key to it. What it shows is read with `task.diff` when it opens and when the view is
// switched.
import { useLayoutEffect, useEffect, useId, useMemo, useRef, useState } from 'react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import type { OwnerClient } from '../owner/client.ts'
import type { TaskDiffResultOutput } from '../owner/generated.ts'
import { type DiffTarget, sizeWords } from './changes.ts'
import { type DiffLine, firstChange, patchLines, wholeLines } from './diff.ts'

const WINDOW = [
  'm-auto w-[min(64rem,calc(100vw-2rem))] max-h-[calc(100vh-3rem)] flex-col rounded-lg border bg-card p-0',
  'text-sm text-card-foreground shadow-lg backdrop:bg-black/40 open:flex',
].join(' ')

const ROW: Record<DiffLine['kind'], string> = {
  meta: 'text-muted-foreground',
  hunk: 'bg-muted text-muted-foreground',
  add: 'bg-c-done/10',
  del: 'bg-destructive/10',
  ctx: '',
}

const MARK: Record<DiffLine['kind'], string> = { meta: ' ', hunk: ' ', add: '+', del: '−', ctx: ' ' }

const words = (failure: unknown) => (failure instanceof Error ? failure.message : String(failure))

interface Held {
  readonly key: string
  readonly result: TaskDiffResultOutput | null
  readonly problem: string | null
}

export interface DiffWindowProps {
  readonly owner: OwnerClient
  readonly boardId: string
  /** The task's key, as `task.changes` was asked for it. */
  readonly task: string
  readonly target: DiffTarget
  /** The window has closed: by Close, by Escape. */
  readonly onClosed: () => void
}

export function DiffWindow({ owner, boardId, task, target, onClosed }: DiffWindowProps) {
  const dialog = useRef<HTMLDialogElement>(null)
  const body = useRef<HTMLDivElement>(null)
  const titleId = useId()
  const [whole, setWhole] = useState(false)
  const [held, setHeld] = useState<Held | null>(null)
  const { source, file } = target
  const key = `${'run' in source ? `run:${source.run}` : `commit:${source.commit}`}|${file.path}|${whole}`

  useEffect(() => {
    const element = dialog.current
    if (element !== null && !element.open) element.showModal()
  }, [])

  useEffect(() => {
    let current = true
    owner.rpc('task.diff', { board: boardId, task, ...source, path: file.path, whole }).then(
      (result) => current && setHeld({ key, result, problem: null }),
      (failure) => current && setHeld({ key, result: null, problem: words(failure) }),
    )
    return () => {
      current = false
    }
  }, [owner, boardId, task, source, file.path, whole, key])

  const now = held !== null && held.key === key ? held : null
  const lines = useMemo(() => linesOf(now?.result ?? null), [now])
  const first = firstChange(lines)

  // Scroll to the first change once the lines are drawn; a diff with none starts at the top.
  useLayoutEffect(() => {
    const element = body.current
    if (element === null) return
    const row = element.querySelector<HTMLElement>('[data-first-change]')
    element.scrollTop = row === null ? 0 : Math.max(0, row.offsetTop - 24)
  }, [now])

  const shut = () => dialog.current?.close()
  return (
    <dialog ref={dialog} aria-labelledby={titleId} onClose={onClosed} className={WINDOW}>
      <header className="flex shrink-0 flex-col gap-2 border-b px-4 py-3">
        <div className="flex items-center gap-2">
          <h2 id={titleId} className="min-w-0 flex-1 text-base font-semibold break-all">
            <span className="font-mono text-sm">{file.path}</span>
          </h2>
          <Button variant="ghost" size="sm" onClick={shut}>
            Close
          </Button>
        </div>
        <p className="text-xs text-muted-foreground">{target.heading}</p>
        <div role="group" aria-label="What to show" className="flex gap-1.5">
          <Toggle on={!whole} onPick={() => setWhole(false)}>
            Changes
          </Toggle>
          <Toggle on={whole} onPick={() => setWhole(true)}>
            Whole file
          </Toggle>
        </div>
      </header>
      <div ref={body} className="relative min-h-0 flex-1 overflow-auto">
        <Notes now={now} />
        {lines.length === 0 ? null : (
          <div data-diff={whole ? 'whole' : 'changes'} className="py-1">
            {lines.map((line, at) => (
              <Row key={at} line={line} first={at === first} />
            ))}
          </div>
        )}
      </div>
    </dialog>
  )
}

function Toggle({ on, onPick, children }: { on: boolean; onPick: () => void; children: string }) {
  return (
    <Button variant={on ? 'secondary' : 'outline'} size="sm" aria-pressed={on} onClick={onPick}>
      {children}
    </Button>
  )
}

/** The lines of what was read: the patch's, or the whole file's. */
function linesOf(result: TaskDiffResultOutput | null): DiffLine[] {
  if (result === null) return []
  if (result.patch !== undefined && result.patch !== null) return patchLines(result.patch.text)
  if (result.whole !== undefined && result.whole !== null) return wholeLines(result.whole.lines)
  return []
}

/** What the read says besides lines: that it is on its way, that it failed, that it was cut, or why it is empty. */
function Notes({ now }: { now: Held | null }) {
  if (now === null) {
    return (
      <p role="status" className="px-4 py-3 text-muted-foreground">
        Reading the file…
      </p>
    )
  }
  if (now.result === null) {
    return <p role="alert" className="px-4 py-3 text-destructive">{`The diff could not be read: ${now.problem}`}</p>
  }
  const { patch, whole, note } = now.result
  const cut =
    patch?.truncated === true
      ? `The patch is cut here: it is ${sizeWords(patch.bytes)} in all.`
      : whole?.truncated === true
        ? `The file is cut here: it is ${sizeWords(whole.bytes)} in all.`
        : null
  const binary =
    whole?.binary === true ? `This file is not text, so there are no lines to show (${sizeWords(whole.bytes)}).` : null
  const empty = patch === undefined && whole === undefined ? 'There is nothing to show for this file.' : null
  const shown = [cut, binary, empty, note].filter((each): each is string => each !== null && each !== undefined)
  if (shown.length === 0) return null
  return (
    <div className="flex flex-col gap-1 border-b px-4 py-2 text-xs text-muted-foreground">
      {shown.map((text) => (
        <p key={text} role={text === cut ? 'status' : undefined}>
          {text}
        </p>
      ))}
    </div>
  )
}

function Row({ line, first }: { line: DiffLine; first: boolean }) {
  return (
    <div
      data-kind={line.kind}
      data-line={line.n ?? undefined}
      data-first-change={first ? '' : undefined}
      className={cn('flex font-mono text-xs leading-5', ROW[line.kind])}
    >
      <span className="w-14 shrink-0 pr-2 text-right text-muted-foreground select-none">{line.n ?? ''}</span>
      <span aria-hidden="true" className="w-4 shrink-0 select-none">
        {MARK[line.kind]}
      </span>
      <span className="min-w-0 flex-1 pr-3 break-all whitespace-pre-wrap">{line.text}</span>
    </div>
  )
}
