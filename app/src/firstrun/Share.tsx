// Share with team (ts-0194-qp; note 10; the mock's banner and `shareModal`). Until the board's line holds a Collide
// folder, a banner says its settings are the person's only and offers Share with team. The window lists the files
// `board.init` would write (its dry run), says nothing is committed, and writes them on Write .collide/. The daemon
// records the hash of what it wrote as trusted for this person, so their own commit of exactly these files asks them
// nothing, and a teammate's clone asks.
import { useEffect, useState } from 'react'
import { Button } from '@/components/ui/button'
import { type OwnerClient, toOwnerError } from '../owner/index.ts'
import { Modal } from './Modal.tsx'

/** What the banner says: nothing, the offer, or that the files are written and wait for the person's commit. */
export type ShareNotice = 'none' | 'offer' | 'written'

/**
 * The banner of a board. The offer shows only once the views have been read (`sharedRead`), so it does not flash for a
 * board that is shared, and only while the line holds no Collide folder. A board whose files this window wrote shows
 * that they wait for a commit, until the line holds them.
 */
export function shareNotice(input: {
  readonly sharedRead: boolean
  readonly shared: boolean
  readonly written: boolean
}): ShareNotice {
  if (!input.sharedRead || input.shared) return 'none'
  return input.written ? 'written' : 'offer'
}

export function ShareBanner({ notice, onShare }: { readonly notice: ShareNotice; readonly onShare: () => void }) {
  if (notice === 'none') return null
  return (
    <div
      role="status"
      aria-label="Share with team"
      className="flex flex-wrap items-center gap-3 rounded-lg border bg-muted px-3.5 py-2.5"
    >
      {notice === 'offer' ? (
        <>
          <span className="min-w-0 flex-1">
            <b>This board&apos;s settings are yours only.</b> Share with team writes them to <code>.collide/</code> in
            the repo, so a teammate&apos;s clone opens the same board.
          </span>
          <Button size="sm" onClick={onShare}>
            Share with team
          </Button>
        </>
      ) : (
        <span className="min-w-0 flex-1">
          <b>
            <code>.collide/</code> is written in the repo folder.
          </b>{' '}
          Commit it, like any change, and a teammate&apos;s clone opens the same board.
        </span>
      )}
    </div>
  )
}

export interface ShareWindowProps {
  readonly owner: OwnerClient
  readonly boardId: string
  /** The files were written: the banner then says they wait for a commit. */
  readonly onWritten: () => void
  readonly onClosed: () => void
}

/** What the window knows: the files to write, or why it cannot. */
type Plan =
  | { readonly state: 'reading' }
  | { readonly state: 'ready'; readonly files: readonly string[] }
  | { readonly state: 'refused'; readonly problem: string }

export function ShareWindow({ owner, boardId, onWritten, onClosed }: ShareWindowProps) {
  const [plan, setPlan] = useState<Plan>({ state: 'reading' })
  const [problem, setProblem] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  useEffect(() => {
    let current = true
    owner.rpc('board.init', { board: boardId, dry_run: true }).then(
      (result) => current && setPlan({ state: 'ready', files: result.files }),
      (failure) => current && setPlan({ state: 'refused', problem: toOwnerError(failure).message }),
    )
    return () => {
      current = false
    }
  }, [owner, boardId])
  const write = async (shut: () => void) => {
    setBusy(true)
    setProblem(null)
    try {
      await owner.rpc('board.init', { board: boardId, dry_run: false })
      onWritten()
      shut()
    } catch (failure) {
      setProblem(toOwnerError(failure).message)
    } finally {
      setBusy(false)
    }
  }
  return (
    <Modal title="Share this board with your team?" onClosed={onClosed}>
      {(shut) => (
        <>
          {plan.state === 'reading' ? (
            <p role="status" className="text-muted-foreground">
              Reading what would be written…
            </p>
          ) : null}
          {plan.state === 'refused' ? (
            <p role="alert" className="whitespace-pre-wrap text-destructive">
              {plan.problem}
            </p>
          ) : null}
          {plan.state === 'ready' ? (
            <>
              <p>
                Collide writes these files into the repo. Nothing is committed; you commit them like any change.
              </p>
              <ul aria-label="Files to write" className="font-mono text-xs">
                {plan.files.map((file) => (
                  <li key={file}>{file}</li>
                ))}
              </ul>
              {plan.files.some((file) => file.includes('/views/')) ? (
                <p className="text-muted-foreground">The views are copies of your saved views of this board.</p>
              ) : (
                <p className="text-muted-foreground">
                  You have no saved views of this board, so none are copied.
                </p>
              )}
              <p className="text-muted-foreground">
                A teammate&apos;s clone then opens the same board. If the repo brings its own packs, views or
                settings, they are asked to trust them first. Your own commit of exactly these files asks you
                nothing. No setting is per board yet, so there is no <code>config.yaml</code>.
              </p>
            </>
          ) : null}
          {problem === null ? null : (
            <p role="alert" className="whitespace-pre-wrap text-destructive">
              {problem}
            </p>
          )}
          <div className="flex justify-end gap-2">
            <Button variant="outline" onClick={shut}>
              Not now
            </Button>
            <Button disabled={busy || plan.state !== 'ready'} onClick={() => void write(shut)}>
              Write .collide/
            </Button>
          </div>
        </>
      )}
    </Modal>
  )
}
