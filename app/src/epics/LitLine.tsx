// The line over the board, the run order and the graph while an epic is picked (ts-0216-ve; note 59 of the board mock):
// which epic, how many of its cards are lit and where the others are, a way to open it in the drawer, and Clear.
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'

export interface LitLineProps {
  readonly title: string
  /** What is lit and where the rest is (`model.ts` `boardWords` and `walkWords`). Empty while it is not known. */
  readonly words: string
  /** The drawer shows the epic, so there is nothing to open. */
  readonly drawerOpen: boolean
  readonly onOpen: () => void
  readonly onClear: () => void
}

export function LitLine({ title, words, drawerOpen, onOpen, onClear }: LitLineProps) {
  return (
    <div role="status" aria-label="Picked epic" className="flex flex-wrap items-center gap-2.5 text-sm">
      <Badge variant="outline" className="border-primary/40 bg-primary/10 text-primary">
        Epic
      </Badge>
      <b>{title}</b>
      <span className="text-muted-foreground">{words}</span>
      <span className="flex-1" />
      {drawerOpen ? null : (
        <Button variant="outline" size="sm" onClick={onOpen}>
          Open the epic
        </Button>
      )}
      <Button variant="ghost" size="sm" onClick={onClear}>
        Clear
      </Button>
    </div>
  )
}
