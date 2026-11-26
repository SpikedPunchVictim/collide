// The Packs tab of Settings (ts-0193-gk): each pack of the board with where it is from, its source, and how many
// statuses the board's sources of it declare. "Add a pack…" asks `onAdd` to open the window that adds one.
import { Button } from '@/components/ui/button'
import type { SourceDecl } from '../field/types.ts'
import { type Pack, packRows, statusesWord } from './model.ts'

export interface PacksTabProps {
  readonly packs: readonly Pack[]
  /** The Field's sources, which say how many statuses a pack's tasks have. */
  readonly sources: readonly SourceDecl[]
  readonly onAdd: () => void
}

export function PacksTab({ packs, sources, onAdd }: PacksTabProps) {
  const rows = packRows(packs, sources)
  return (
    <div className="flex flex-col gap-3 p-4">
      <ul aria-label="Packs" className="flex flex-col gap-2">
        {rows.map((row) => (
          <li key={`${row.name}:${row.source ?? ''}`} className="flex items-center gap-3 rounded-md border px-3 py-2">
            <span className="grow">
              <b>{row.name}</b>
              <span className="block text-xs text-muted-foreground">
                {row.from}
                {row.source === null ? null : (
                  <>
                    {' · '}
                    <span className="font-mono">{row.source}</span>
                  </>
                )}
              </span>
            </span>
            {row.enabled ? null : <span className="rounded-4xl bg-muted px-2 py-0.5 text-xs">Off</span>}
            {row.statuses === null ? null : (
              <span className="rounded-4xl bg-secondary px-2 py-0.5 text-xs">{statusesWord(row.statuses)}</span>
            )}
          </li>
        ))}
      </ul>
      <div>
        <Button variant="outline" size="sm" onClick={onAdd}>
          Add a pack…
        </Button>
      </div>
    </div>
  )
}
