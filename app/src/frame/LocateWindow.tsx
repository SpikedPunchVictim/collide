// The window that locates a board whose folder moved (ts-0191-nv; the mock's `locateModal`). The person types the
// folder or, in the shell, chooses it with the system's picker; `onLocate` sends it to the daemon, which checks that it
// is the same repository. A refusal stays in the window with the daemon's words and changes nothing. A folder that
// was taken closes the window, unless the daemon had a note to add, which the window then shows. It is the
// browser's own `<dialog>`, as `markdown/ExternalLinks.tsx` has it, so it holds the focus and closes on Escape.
import { type FormEvent, useEffect, useId, useRef, useState } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import type { LocateOutcome } from '../loop/index.ts'
import { toOwnerError } from '../owner/index.ts'

/** The window's look: centred, with the page behind it dimmed. */
const WINDOW = [
  'm-auto w-[min(32rem,calc(100vw-2rem))] rounded-lg border bg-card p-5',
  'text-sm text-card-foreground shadow-lg backdrop:bg-black/40',
].join(' ')

export interface LocateWindowProps {
  readonly boardName: string
  /** The system's folder picker, in the shell. Without it the person types the folder. */
  readonly pick?: () => Promise<string | null>
  readonly onLocate: (path: string) => Promise<LocateOutcome>
  /** The window has closed: by Cancel, by Escape, or once the folder was taken. */
  readonly onClosed: () => void
}

export function LocateWindow({ boardName, pick, onLocate, onClosed }: LocateWindowProps) {
  const dialog = useRef<HTMLDialogElement>(null)
  const titleId = useId()
  const inputId = useId()
  const [path, setPath] = useState('')
  const [problem, setProblem] = useState<string | null>(null)
  const [note, setNote] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  useEffect(() => {
    const element = dialog.current
    if (element !== null && !element.open) element.showModal()
  }, [])
  const shut = () => dialog.current?.close()
  const choose = async () => {
    if (pick === undefined) return
    try {
      const chosen = await pick()
      if (chosen !== null) setPath(chosen)
    } catch (failure) {
      setProblem(`Collide could not open the folder picker: ${toOwnerError(failure).message}`)
    }
  }
  const submit = async (event: FormEvent) => {
    event.preventDefault()
    setBusy(true)
    setProblem(null)
    try {
      const outcome = await onLocate(path)
      if (outcome.problem !== null) setProblem(outcome.problem)
      else if (outcome.note !== null) setNote(outcome.note)
      else shut()
    } finally {
      setBusy(false)
    }
  }
  return (
    <dialog ref={dialog} aria-labelledby={titleId} onClose={onClosed} className={WINDOW}>
      {note === null ? (
        <form onSubmit={submit} className="flex flex-col gap-3">
          <h2 id={titleId} className="text-base font-semibold">{`Locate ${boardName}`}</h2>
          <label htmlFor={inputId}>Where is the folder now?</label>
          <div className="flex gap-2">
            <Input
              id={inputId}
              placeholder="/path/to/the/moved/checkout"
              value={path}
              onChange={(event) => setPath(event.target.value)}
            />
            {pick === undefined ? null : (
              <Button type="button" variant="outline" onClick={() => void choose()}>
                Choose a folder…
              </Button>
            )}
          </div>
          <p className="text-muted-foreground">Collide checks that it is the same repository. Nothing is deleted.</p>
          {problem === null ? null : (
            <p role="alert" className="text-destructive">
              {problem}
            </p>
          )}
          <div className="flex justify-end gap-2">
            <Button type="button" variant="outline" onClick={shut}>
              Cancel
            </Button>
            <Button type="submit" disabled={busy || path.trim() === ''}>
              Use this folder
            </Button>
          </div>
        </form>
      ) : (
        <div className="flex flex-col gap-3">
          <h2 id={titleId} className="text-base font-semibold">{`${boardName} is back`}</h2>
          <p>{note}</p>
          <div className="flex justify-end">
            <Button onClick={shut}>Close</Button>
          </div>
        </div>
      )}
    </dialog>
  )
}
