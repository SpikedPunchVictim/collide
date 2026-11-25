// The page of a board whose folder is gone (ts-0191-nv; note 15; the mock's `missingHtml`). It says where Collide
// looked, that nothing was deleted and that the tasks are kept, and offers to locate the folder. Removing a board
// arrives later, so that button is there and off.
import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'

export interface MissingBoardProps {
  readonly name: string
  /** The folder the board was in, as the daemon holds it. */
  readonly folder: string
  readonly onLocate: () => void
}

export function MissingBoard({ name, folder, onLocate }: MissingBoardProps) {
  return (
    <Card aria-label="Board folder not found" className="mx-auto w-full max-w-xl items-center px-6 text-center">
      <h1 className="text-xl font-semibold">{`${name}: its folder is gone`}</h1>
      <p>
        {'Collide looked for '}
        <code className="font-mono [overflow-wrap:anywhere]">{folder}</code>
        {' and found nothing there. It was moved or deleted.'}
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
