// The trust prompt (ts-0194-qp; note 10; the mock's `trustModal`), for a repo whose Collide folder brings packs, views
// or settings and whose line is not trusted. It says what the repo brings, in the daemon's review: where each pack is
// from and what it runs, the views and settings by name. The built-in packs need no trust. Open without it opens the
// board with the built-in packs only, and a banner names what is not applied. Trust and open sends the review's hash
// to `board.trust`. It is a function of its props.
import { useRef, useState } from 'react'
import { Button } from '@/components/ui/button'
import { PackReview } from '../settings/PackReview.tsx'
import { Modal } from './Modal.tsx'
import { type Brought, type Review, bringsWords, kindsWords } from './trust.ts'

export interface TrustWindowProps {
  readonly boardName: string
  readonly review: Review
  readonly brought: Brought
  /** Trusts the line the review is of. Resolves with the daemon's words when it did not. */
  readonly onTrust: () => Promise<string | null>
  /** The person chose to open without it, or closed the window. */
  readonly onWithout: () => void
}

export function TrustWindow({ boardName, review, brought, onTrust, onWithout }: TrustWindowProps) {
  const [problem, setProblem] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  // A window that closes without a yes is Open without it: Escape never trusts.
  const trusted = useRef(false)
  return (
    <Modal
      title={`Trust the ${kindsWords(brought)} this repo brings?`}
      onClosed={() => {
        if (!trusted.current) onWithout()
      }}
    >
      {(shut) => (
        <>
          <p>
            <b>{boardName}</b> brings {bringsWords(brought)}.
            {brought.packs.length === 0
              ? null
              : ' A pack runs code on this Mac, with your permissions, to read its tasks.'}
          </p>
          {(review.packs ?? []).map((pack) => (
            <PackReview key={pack.source} source={pack.source} dev={false} review={pack.review} />
          ))}
          {brought.views.length === 0 ? null : (
            <p>
              Views: {brought.views.map((view, index) => (
                <span key={view}>
                  {index === 0 ? '' : ', '}
                  <code>{view}</code>
                </span>
              ))}
            </p>
          )}
          {brought.settings ? (
            <p>
              Settings: the repo&apos;s <code>.collide/config.yaml</code>
            </p>
          ) : null}
          {(review.problems ?? []).map((text) => (
            <p key={text} role="alert" className="whitespace-pre-wrap text-destructive">
              {text}
            </p>
          ))}
          <p className="text-muted-foreground">
            The built-in packs need no trust. Until you trust what the repo brings, it is not applied.
          </p>
          {problem === null ? null : (
            <p role="alert" className="whitespace-pre-wrap text-destructive">
              {problem}
            </p>
          )}
          <div className="flex justify-end gap-2">
            <Button variant="outline" onClick={shut}>
              Open without it
            </Button>
            <Button
              disabled={busy}
              onClick={() => {
                setBusy(true)
                setProblem(null)
                void onTrust()
                  .then((failed) => {
                    if (failed === null) {
                      trusted.current = true
                      shut()
                    } else setProblem(failed)
                  })
                  .finally(() => setBusy(false))
              }}
            >
              Trust and open
            </Button>
          </div>
        </>
      )}
    </Modal>
  )
}

/** The banner of a board opened without what its repo brings: it names what is not applied. */
export function TrustBanner({ brought, onReview }: { readonly brought: Brought; readonly onReview: () => void }) {
  return (
    <div
      role="status"
      aria-label="Not trusted"
      className="flex flex-wrap items-center gap-3 rounded-lg border border-waiting/35 bg-waiting/10 px-3.5 py-2.5"
    >
      <span className="min-w-0 flex-1">
        <b>Not applied: {bringsWords(brought)}.</b> This repo brings {kindsWords(brought)} that are not trusted. The
        built-in packs run either way.
      </span>
      <Button variant="outline" size="sm" onClick={onReview}>
        Review and trust…
      </Button>
    </div>
  )
}
