// The page of a board whose folder is gone, or is no longer its checkout (ts-0191-nv, ts-0221-pp; note 15; the mock's
// `missingHtml`). It says which, where Collide looked, that nothing was deleted and that the tasks are kept, and offers
// to locate the folder. Removing a board arrives later, so that button is there and off.
import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'
import type { MissingReason } from '../loop/types.ts'

export interface MissingBoardProps {
  readonly name: string
  /** The folder the board was in, as the daemon holds it. */
  readonly folder: string
  readonly reason: MissingReason
  readonly onLocate: () => void
}

/** What the page says for each reason: its heading, and what Collide found at the folder. */
const WHAT: Record<MissingReason, { readonly title: string; readonly found: string }> = {
  gone: { title: 'its folder is gone', found: ' and found nothing there. It was moved or deleted.' },
  'not-a-checkout': {
    title: 'its folder is no longer a git checkout',
    found: ' and found a folder that is not a git checkout now. It may have been moved and made again.',
  },
  'other-repository': {
    title: 'its folder now holds another repository',
    found: " and found a different repository there. The board's own repository may have moved.",
  },
}

export function MissingBoard({ name, folder, reason, onLocate }: MissingBoardProps) {
  return (
    <Card aria-label="Board folder not found" className="mx-auto w-full max-w-xl items-center px-6 text-center">
      <h1 className="text-xl font-semibold">{`${name}: ${WHAT[reason].title}`}</h1>
      <p>
        {'Collide looked for '}
        <code className="font-mono [overflow-wrap:anywhere]">{folder}</code>
        {WHAT[reason].found}
      </p>
      <p className="text-muted-foreground">
        Nothing has been deleted. Its tasks are kept, and come back as they were when you point Collide at the
        folder.
      </p>
      <div className="flex gap-2">
        <Button onClick={onLocate}>Locate the folder…</Button>
        <Button variant="outline" disabled title="Arrives with board removal">
          Remove from Collide
        </Button>
      </div>
    </Card>
  )
}
