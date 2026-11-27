// The banner of a window started with defaults (ts-0215-gt; plan, Stage 6a, Start with defaults), the window that lists
// what Reset moves, and the notice that names the folder it moved them to. Each is a function of its props, so a test
// renders them to text; `App.tsx` holds the state and the calls.
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Modal } from '../firstrun/Modal.tsx'

/** The words of the banner. */
export const DEFAULTS_WORDS = 'Started with defaults. Your settings, views and packs are set aside for this window.'

/** Where Reset moves things: under the folder of the user's config, in a folder named for the date and time. */
export const SET_ASIDE_WHERE = '~/.config/collide/set-aside/<date and time>/'

export interface DefaultsBannerProps {
  /** Leave defaults mode: the window starts normally. */
  readonly onNormal: () => void
  readonly onReset: () => void
  /**
   * Whether Reset… may be pressed. It is off until the first read of the boards has answered, so the window it opens
   * names the board that is about to show and not "No board is open" (ts-0218-sc).
   */
  readonly canReset: boolean
}

export function DefaultsBanner({ onNormal, onReset, canReset }: DefaultsBannerProps) {
  return (
    <div
      role="status"
      aria-label="Started with defaults"
      className="flex flex-wrap items-center gap-3 rounded-lg border border-waiting/35 bg-waiting/10 px-3.5 py-2.5"
    >
      <span className="min-w-0 flex-1">{DEFAULTS_WORDS}</span>
      <Button variant="outline" size="sm" onClick={onNormal}>
        Start normally
      </Button>
      <Button variant="outline" size="sm" onClick={onReset} disabled={!canReset}>
        Reset…
      </Button>
    </div>
  )
}

/** What Reset moves, one line each. The board's line names the board. */
export function movedLines(boardName: string | null): readonly string[] {
  return [
    'Your settings file (config.yaml).',
    'Your packs (packs.yaml) and their lock (packs.lock).',
    boardName === null
      ? 'No board is open, so no board folder.'
      : `The folder of this board, ${boardName}: your views.`,
    'This window’s stored state: the folds, the layouts and the width of the drawer.',
  ]
}

/** What Reset leaves where it is. */
export const LEFT_LINES: readonly string[] = [
  'The folders of your other boards.',
  'The repo, with its .collide folder.',
  'Your secrets in the keychain.',
]

export interface ResetWindowProps {
  readonly boardName: string | null
  /** Moves the files, clears the stored state and starts normally. Resolves with the words of what went wrong. */
  readonly onConfirm: () => Promise<string | null>
  readonly onClosed: () => void
}

export function ResetWindow({ boardName, onConfirm, onClosed }: ResetWindowProps) {
  const [problem, setProblem] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  return (
    <Modal title="Reset your settings, views and packs?" onClosed={onClosed}>
      {(shut) => (
        <>
          <p>
            Reset moves these into <code>{SET_ASIDE_WHERE}</code>. Nothing is deleted, and the folder keeps the paths,
            so you can move any of it back.
          </p>
          <ul aria-label="Reset moves" className="list-disc pl-5">
            {movedLines(boardName).map((line) => (
              <li key={line}>{line}</li>
            ))}
          </ul>
          <p>Reset leaves these as they are:</p>
          <ul aria-label="Reset leaves" className="list-disc pl-5">
            {LEFT_LINES.map((line) => (
              <li key={line}>{line}</li>
            ))}
          </ul>
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
                void onConfirm()
                  .then((failed) => {
                    if (failed !== null) setProblem(failed)
                  })
                  .finally(() => setBusy(false))
              }}
            >
              Move them aside
            </Button>
          </div>
        </>
      )}
    </Modal>
  )
}

/** The notice of a window that started normally after a Reset: it names the folder the files went to. */
export function ResetNotice({ folder, onDismiss }: { readonly folder: string; readonly onDismiss: () => void }) {
  return (
    <div
      role="status"
      aria-label="Reset done"
      className="flex flex-wrap items-center gap-3 rounded-lg border border-border bg-muted px-3.5 py-2.5"
    >
      <span className="min-w-0 flex-1">
        <b>Reset done.</b> Your settings, views and packs were moved to <code className="break-all">{folder}</code>.
        Nothing was deleted.
      </span>
      <Button variant="outline" size="sm" onClick={onDismiss}>
        Dismiss
      </Button>
    </div>
  )
}
