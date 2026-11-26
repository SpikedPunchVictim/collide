// The window that asks before `git init` (ts-0194-qp; the mock's `gitInitModal`). It says what the command does and
// where. Run git init opens the folder with `init_git`; a refusal stays in the window in the daemon's words.
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Modal } from './Modal.tsx'

export interface GitInitWindowProps {
  readonly folder: string
  /** Makes the folder a repository and opens it. Resolves with the daemon's words when it did not. */
  readonly onRun: () => Promise<string | null>
  readonly onClosed: () => void
}

export function GitInitWindow({ folder, onRun, onClosed }: GitInitWindowProps) {
  const [problem, setProblem] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  return (
    <Modal title="Make this folder a git repository?" onClosed={onClosed}>
      {(shut) => (
        <>
          <p>
            This runs <code>git init</code> in <code className="font-mono [overflow-wrap:anywhere]">{folder}</code>. It
            adds a <code>.git</code> folder and changes nothing else.
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
                void onRun()
                  .then((failed) => {
                    if (failed === null) shut()
                    else setProblem(failed)
                  })
                  .finally(() => setBusy(false))
              }}
            >
              Run git init
            </Button>
          </div>
        </>
      )}
    </Modal>
  )
}
