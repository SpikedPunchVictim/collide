// The Planning tab (ts-0195-ju): what the board holds of the plan, read only. It reads the sections, keeps which
// proposal Readiness shows and which task the drawer shows, and draws `PlanningView` beside the drawer. It sends no
// method that writes: `task.sections` and the drawer's own reads are all it asks.
import { useState } from 'react'
import { Drawer } from '../drawer/Drawer.tsx'
import type { BoardNote } from '../drawer/types.ts'
import type { FoldStore } from '../field/folds.ts'
import type { FieldState } from '../field/useField.ts'
import type { OwnerClient } from '../owner/client.ts'
import { PlanningView } from './PlanningView.tsx'
import { useSections } from './useSections.ts'

export interface PlanningPageProps {
  readonly owner: OwnerClient
  readonly boardId: string | null
  readonly board?: BoardNote | null
  readonly field: FieldState
  /** Where the drawer keeps its folds and width. The webview's own, unless a test gives another. */
  readonly store?: FoldStore | null
}

export function PlanningPage({ owner, boardId, board = null, field, store }: PlanningPageProps) {
  const sections = useSections(owner, boardId, field.updatedAt)
  const [picked, setPicked] = useState<string | null>(null)
  const [selected, setSelected] = useState<string | null>(null)
  const { data } = field
  if (boardId === null) return <p className="text-muted-foreground">No board is open.</p>
  if (data === null) {
    return field.problem === null ? (
      <p role="status" aria-label="Planning" className="text-muted-foreground">
        Reading the plan…
      </p>
    ) : (
      <p role="alert" aria-label="Planning" className="text-destructive">
        {`The plan could not be read: ${field.problem}`}
      </p>
    )
  }
  const every = data.corpus.length > 0 ? data.corpus : data.tasks
  const chosen = selected === null ? null : (every.find((task) => task.key === selected) ?? null)
  return (
    <div className="flex items-start gap-3">
      <div className="min-w-0 flex-1">
        <PlanningView
          data={data}
          sections={sections}
          picked={picked}
          onPick={(key) => {
            setPicked(key)
            setSelected(key)
          }}
          onOpenTask={setSelected}
        />
      </div>
      {chosen === null ? null : (
        <Drawer
          owner={owner}
          boardId={boardId}
          shown={chosen}
          data={data}
          board={board}
          stamp={field.updatedAt}
          store={store}
          onOpenTask={setSelected}
          onClose={() => setSelected(null)}
        />
      )}
    </div>
  )
}
