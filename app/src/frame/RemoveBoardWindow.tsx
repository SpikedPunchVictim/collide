// The window that asks before a board is removed from Collide (ts-0222-gx; the mock's missing page and board menu). It
// says the folder is not touched and that opening the folder again brings the board back. Removing hides the board
// and keeps what the person made, so the window says what is kept. A refusal (a run is in progress, say) stays in the
// window in the daemon's words, and the board stays. A removal that went through closes the window.
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Modal } from '../firstrun/Modal.tsx'

export interface RemoveBoardWindowProps {
  readonly boardName: string
  /** The folder the board is in, as the daemon holds it. */
  readonly folder: string
  /** Removes the board. Resolves with the daemon's words when it was refused, or null when it was removed. */
  readonly onRemove: () => Promise<string | null>
  /** The window has closed: by Cancel, by Escape, or once the board was removed. */
  readonly onClosed: () => void
}

export function RemoveBoardWindow({ boardName, folder, onRemove, onClosed }: RemoveBoardWindowProps) {
  const [problem, setProblem] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  return (
    <Modal title={`Remove ${boardName} from Collide?`} onClosed={onClosed}>
      {(shut) => (
        <>
          <p>
            The folder <code className="font-mono [overflow-wrap:anywhere]">{folder}</code> is not touched. The board
            leaves the list of boards.
          </p>
          <p className="text-muted-foreground">
            Its scratch tasks, its runs and what you trusted are kept. Opening the folder again brings the board back,
            as it was.
          </p>
          {problem === null ? null : (
            <p role="alert" className="whitespace-pre-wrap text-destructive">
              {problem}
            </p>
          )}
          <div className="flex justify-end gap-2">
            <Button variant="outline" onClick={shut}>
              Cancel
            </Button>
            <Button
              disabled={busy}
              onClick={() => {
                setBusy(true)
                setProblem(null)
                void onRemove()
                  .then((failed) => {
                    if (failed === null) shut()
                    else setProblem(failed)
                  })
                  .finally(() => setBusy(false))
              }}
            >
              Remove from Collide
            </Button>
          </div>
        </>
      )}
    </Modal>
  )
}
