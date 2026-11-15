// Tracks (0.11): every run as a horizontal track of phase segments, parallel runs stacked so the whole fleet fits one
// screen. A run is named by its id label (`R-118`), never by colour, which is reserved for state, and its state is
// written in words. The real-time axis (segment width is duration, a now line, hatched waits, gates, zoom) is Stage 9b;
// until then each segment is the same width.
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { cn } from '@/lib/utils'
import type { Highlight, Lens } from '../../ui/index.ts'
import { LensBanner } from '../LensBanner.tsx'
import { ComparePane } from '../compare/ComparePane.tsx'
import type { PhaseSummary, RunSummary } from '../types.ts'
import { runStateWords } from '../words.ts'

export interface TracksSurfaceProps {
  readonly runs: readonly RunSummary[]
  readonly lens: Lens | null
  readonly highlight: Highlight | null
  readonly onOpenRun: (runId: string) => void
  readonly onCompare: (leftId: string, rightId: string) => void
  readonly onClearLens: () => void
  /** Two finished runs to show side by side, or `null` when none are ticked. */
  readonly compare: readonly [RunSummary, RunSummary] | null
}

const isFinished = (run: RunSummary) => run.state.state === 'ended' || run.state.state === 'failed' || run.state.state === 'stopped'

export function TracksSurface(props: TracksSurfaceProps) {
  const live = props.runs.filter((r) => !isFinished(r))
  const finished = props.runs.filter(isFinished)
  const marked = new Set(props.highlight?.ids ?? [])
  return (
    <section aria-labelledby="tracks-heading" className="flex flex-col gap-4">
      <h2 id="tracks-heading" className="text-xl font-semibold">
        Tracks
      </h2>
      <LensBanner lens={props.lens} highlight={props.highlight} onClearLens={props.onClearLens} />
      <div className="flex flex-col gap-3">
        {live.length === 0 ? <p className="text-sm text-muted-foreground">No run is working right now.</p> : null}
        {live.map((run) => (
          <Track key={run.id} run={run} marked={marked.has(run.id)} onOpen={() => props.onOpenRun(run.id)} />
        ))}
      </div>
      {finished.length > 0 ? (
        <div className="flex flex-col gap-2">
          <h3 className="text-sm font-medium text-muted-foreground">Finished runs</h3>
          {finished.map((run) => (
            <Track key={run.id} run={run} marked={marked.has(run.id)} onOpen={() => props.onOpenRun(run.id)} />
          ))}
          {finished.length >= 2 ? (
            <div>
              <Button variant="outline" onClick={() => props.onCompare(finished[0]!.id, finished[1]!.id)}>
                Compare the first two
              </Button>
            </div>
          ) : null}
        </div>
      ) : null}
      {props.compare ? <ComparePane left={props.compare[0]} right={props.compare[1]} /> : null}
    </section>
  )
}

interface TrackProps {
  readonly run: RunSummary
  readonly marked: boolean
  readonly onOpen: () => void
}

function Track({ run, marked, onOpen }: TrackProps) {
  return (
    <Card size="sm">
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          {run.id}
          <Badge variant={run.state.state === 'failed' ? 'destructive' : 'secondary'}>{runStateWords(run.state)}</Badge>
          {marked ? <Badge variant="outline">Highlighted</Badge> : null}
        </CardTitle>
        <CardDescription>
          {run.workflow}, on {run.line}
        </CardDescription>
      </CardHeader>
      <CardContent>
        <ol className="flex gap-1" aria-label={`Phases of ${run.id}`}>
          {run.phases.map((phase) => (
            <Segment key={`${phase.name}-${phase.round}`} phase={phase} />
          ))}
        </ol>
        <div>
          <Button variant="ghost" size="sm" onClick={onOpen}>
            Open {run.id}
          </Button>
        </div>
      </CardContent>
    </Card>
  )
}

function Segment({ phase }: { readonly phase: PhaseSummary }) {
  const first = phase.instances[0]?.state.state ?? 'pending'
  return (
    <li
      className={cn(
        'flex-1 rounded-md border px-2 py-1 text-xs',
        first === 'running' && 'bg-primary text-primary-foreground',
        first === 'pending' && 'border-dashed text-muted-foreground',
        first === 'ended' && 'bg-muted',
        first === 'failed' && 'border-destructive text-destructive',
      )}
    >
      <span className="font-medium">{phase.name}</span>
      <span className="block">{first === 'ended' ? 'done' : first}</span>
    </li>
  )
}
