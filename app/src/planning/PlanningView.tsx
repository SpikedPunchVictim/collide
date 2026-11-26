// The Planning tab, drawn from what was read (ts-0195-ju; 0.11, the Planning surface; note 2 of the board mock): a tile
// for each epic, the proposals, the features by section and the readiness of the proposal picked. It is read only:
// nothing here writes, and the one button that would start a planning session is off until Stage 9d. It reads nothing
// and keeps nothing, so a test draws it from a scripted read. Every word from a task is React text.
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { BAR_ORDER } from '../epics/model.ts'
import { DOT } from '../field/colours.ts'
import type { FieldData } from '../field/types.ts'
import {
  type FeatureRow,
  type ProposalRow,
  type Sections,
  defaultProposal,
  featureRowsOf,
  proposalWords,
  proposalsOf,
  readinessOf,
  tileWords,
  tilesOf,
} from './model.ts'

export interface PlanningViewProps {
  readonly data: FieldData
  /** What `task.sections` said, or null before it was read or when it failed. */
  readonly sections: Sections | null
  /** The proposal Readiness shows, by key; null for the first one that is open. */
  readonly picked: string | null
  readonly onPick: (key: string) => void
  /** A task named in a row opens in the drawer, by its key. */
  readonly onOpenTask: (key: string) => void
}

const link = 'cursor-pointer text-left hover:underline focus-visible:outline-2 focus-visible:outline-ring'

export function PlanningView({ data, sections, picked, onPick, onOpenTask }: PlanningViewProps) {
  const tiles = tilesOf(data)
  const proposals = proposalsOf(data)
  const features = featureRowsOf(data, sections)
  const shown = proposals.some((row) => row.key === picked) ? picked : defaultProposal(proposals)
  const proposal = data.corpus.find((task) => task.key === shown) ?? data.tasks.find((task) => task.key === shown)
  return (
    <section aria-label="Planning" className="flex min-w-0 flex-col gap-3">
      <div className="flex flex-wrap items-center gap-3">
        <h1 className="text-xl font-semibold">Planning</h1>
        <Badge variant="outline" className="font-normal">
          read only
        </Badge>
        <p className="min-w-0 flex-1 text-muted-foreground">
          What to build next, and the decisions to make first. Changes are made in the files or by a planning session
          (Stage 9d).
        </p>
        <Button variant="outline" size="sm" disabled title="Planning sessions arrive in Stage 9d.">
          Plan…
        </Button>
        <span className="text-xs text-muted-foreground">Planning sessions arrive in Stage 9d.</span>
      </div>
      <h2 className="text-base font-semibold">Epics</h2>
      {tiles.length === 0 ? (
        <p className="text-muted-foreground">No epic yet.</p>
      ) : (
        <div className="grid grid-cols-[repeat(auto-fill,minmax(14rem,1fr))] gap-2">
          {tiles.map((tile) => (
            <div key={tile.card.id} data-epic={tile.card.id} className="flex flex-col gap-1.5 rounded-md border p-2.5">
              <b className="leading-snug font-semibold">{tile.card.title}</b>
              <span aria-hidden="true" className="flex h-1.5 overflow-hidden rounded-full bg-secondary">
                {BAR_ORDER.filter((category) => tile.card.counts[category] > 0).map((category) => (
                  <span
                    key={category}
                    className={cn('block h-full', DOT[category])}
                    style={{ flex: tile.card.counts[category] }}
                  />
                ))}
              </span>
              <span className="text-xs text-muted-foreground">{tileWords(tile)}</span>
            </div>
          ))}
        </div>
      )}
      <h2 className="text-base font-semibold">Proposals</h2>
      {proposals.length === 0 ? (
        <p className="text-muted-foreground">No proposal yet.</p>
      ) : (
        <div className="flex flex-col gap-1.5">
          {proposals.map((row) => (
            <ProposalButton key={row.key} row={row} on={row.key === shown} onPick={onPick} />
          ))}
        </div>
      )}
      <h2 className="text-base font-semibold">Features by section</h2>
      <FeatureTable rows={features} onOpenTask={onOpenTask} />
      <h2 className="text-base font-semibold">Readiness</h2>
      {proposal === undefined ? (
        <p className="text-muted-foreground">No proposal yet.</p>
      ) : (
        <div className="flex flex-col gap-1.5">
          <p className="text-muted-foreground">{`For ${proposal.title}. Advice only: nothing here is enforced.`}</p>
          {readinessOf(proposal).map((line) => (
            <label key={line.id} data-readiness={line.id} className="flex items-baseline gap-2">
              <input type="checkbox" checked={line.ok} disabled readOnly />
              <span>{line.text}</span>
              {line.why === '' ? null : <span className="text-xs text-muted-foreground">{line.why}</span>}
            </label>
          ))}
        </div>
      )}
    </section>
  )
}

function ProposalButton({ row, on, onPick }: { row: ProposalRow; on: boolean; onPick: (key: string) => void }) {
  return (
    <button
      type="button"
      aria-pressed={on}
      data-proposal={row.key}
      onClick={() => onPick(row.key)}
      className={cn(
        'flex w-full cursor-pointer items-center gap-2 rounded-md border px-2.5 py-1.5 text-left',
        'hover:border-input hover:bg-muted focus-visible:outline-2 focus-visible:outline-ring',
        on && 'border-primary bg-muted',
      )}
    >
      <span className={cn('size-2 shrink-0 rounded-full', row.open ? DOT.active : DOT.done)} />
      <span className="min-w-0 flex-1">
        {row.title} <span className="text-xs text-muted-foreground">{proposalWords(row)}</span>
      </span>
      {row.only === null ? null : (
        <Badge variant="outline" className="font-normal text-waiting">
          {row.only}
        </Badge>
      )}
    </button>
  )
}

function FeatureTable({ rows, onOpenTask }: { rows: readonly FeatureRow[]; onOpenTask: (key: string) => void }) {
  if (rows.length === 0) return <p className="text-muted-foreground">No feature yet.</p>
  return (
    <table className="w-full text-sm">
      <thead>
        <tr className="text-left text-xs text-muted-foreground">
          <th className="py-1 font-normal">Section</th>
          <th className="py-1 font-normal">Feature</th>
          <th className="py-1 font-normal">From</th>
          <th className="py-1 font-normal">State</th>
          <th className="py-1 font-normal">Task</th>
        </tr>
      </thead>
      <tbody>
        {rows.map((row) => (
          <tr key={row.id} data-section={row.section} data-feature={row.id} className="border-t align-top">
            <td className={cn('py-1.5 pr-2', row.unsorted && 'text-muted-foreground italic')}>
              {row.section}
              {row.looksLike.map((name) => (
                <Badge key={name} variant="outline" className="ml-1.5 font-normal text-waiting">
                  {`looks like ${name}`}
                </Badge>
              ))}
            </td>
            <td className="py-1.5 pr-2">{row.title}</td>
            <td className="py-1.5 pr-2">
              <button type="button" className={link} onClick={() => onOpenTask(row.proposalKey)}>
                {row.proposal}
              </button>
            </td>
            <td className="py-1.5 pr-2">{row.state}</td>
            <td className="py-1.5">
              {row.tasks.length === 0 ? (
                <span className="text-muted-foreground">no task yet</span>
              ) : (
                row.tasks.map((task) => (
                  <button
                    key={task.key}
                    type="button"
                    className={cn(link, 'mr-2 font-mono text-xs')}
                    onClick={() => onOpenTask(task.key)}
                  >
                    {task.shortId}
                  </button>
                ))
              )}
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  )
}
