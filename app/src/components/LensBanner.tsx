// What the in-app agent has put on a surface (0.11): a lens that groups tasks or runs with a reason for each group, and
// highlights. The agent's words are cleaned by the reducer and shown here as text, marked as the agent's.
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import type { Highlight, Lens } from '../ui/index.ts'

export interface LensBannerProps {
  readonly lens: Lens | null
  readonly highlight: Highlight | null
  readonly onClearLens: () => void
}

export function LensBanner({ lens, highlight, onClearLens }: LensBannerProps) {
  if (lens === null && highlight === null) return null
  return (
    <div className="flex flex-col gap-2 rounded-lg border bg-muted/40 p-3 text-sm">
      {lens ? (
        <div className="flex flex-col gap-2">
          <div className="flex items-center justify-between gap-2">
            <span className="flex items-center gap-2 font-medium">
              {lens.title}
              <Badge variant="outline">From the agent, unverified</Badge>
            </span>
            <Button variant="ghost" size="sm" onClick={onClearLens}>
              Clear grouping
            </Button>
          </div>
          <ul className="flex flex-col gap-1">
            {lens.groups.map((group) => (
              <li key={group.name}>
                <span className="font-medium">{group.name}</span>
                <span className="text-muted-foreground">
                  {' '}
                  ({group.ids.length}): {group.reason}
                </span>
              </li>
            ))}
          </ul>
        </div>
      ) : null}
      {highlight ? (
        <p>
          <span className="font-medium">{highlight.title ?? 'Highlighted'}</span>
          {highlight.note ? <span className="text-muted-foreground">: {highlight.note}</span> : null}
        </p>
      ) : null}
    </div>
  )
}
