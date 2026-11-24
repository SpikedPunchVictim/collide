// A still preview of Tracks, which Stage 9b builds (note 2 of the board mock; `tracksHtml`). The runs are made up, and
// the page takes no data and calls nothing: its rows and phases are not controls. The colours are the app's own tokens.
import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'
import { cn } from '@/lib/utils'

type PhaseState = 'done' | 'run' | 'wait'

interface PreviewRun {
  readonly id: string
  readonly label: string
  readonly live: boolean
  readonly task: string
  readonly title: string
  readonly when: string
  readonly phases: readonly { readonly name: string; readonly state: PhaseState; readonly mins: number }[]
}

/** The mock's two runs (`RUNS`), with the title of each run's task. */
const RUNS: readonly PreviewRun[] = [
  {
    id: 'r121',
    label: 'Run R-121',
    live: true,
    task: 'ts-0178',
    title: "Loop page: a file run's card says its task is unknown",
    when: 'Today 09:20',
    phases: [
      { name: 'Plan', state: 'done', mins: 3 },
      { name: 'Build', state: 'run', mins: 17 },
      { name: 'Review', state: 'wait', mins: 6 },
    ],
  },
  {
    id: 'r119',
    label: 'Run R-119',
    live: false,
    task: 'ts-0160',
    title: 'Discard: a refusal goes unseen, and a Review item whose repository is gone stays',
    when: 'Yesterday 15:02',
    phases: [
      { name: 'Plan', state: 'done', mins: 4 },
      { name: 'Build', state: 'done', mins: 31 },
      { name: 'Review', state: 'done', mins: 9 },
    ],
  },
]

const PHASE_STYLE: Record<PhaseState, string> = {
  done: 'bg-primary/15',
  run: 'bg-primary/25 ring-1 ring-primary ring-inset',
  wait: 'bg-secondary text-muted-foreground',
}

function phaseWords(name: string, state: PhaseState, mins: number): string {
  if (state === 'wait') return `${name} · next`
  return `${name} · ${mins} m${state === 'run' ? ' so far' : ''}`
}

export function TracksPreview({ onOpenLoop }: { onOpenLoop: () => void }) {
  const running = RUNS.filter((run) => run.live).length
  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center gap-3 rounded-lg border border-primary/25 bg-primary/5 px-3.5 py-2.5">
        <p className="min-w-56 flex-1">
          <b>A preview of Stage 9b.</b> Tracks arrives with the run UI; these runs are made up. Until then, the Loop
          page shows a run and what it needs from you.
        </p>
        <Button variant="outline" size="sm" onClick={onOpenLoop}>
          Open the Loop page
        </Button>
      </div>
      <Card aria-label="Runs, a preview" className="gap-2.5 px-4 py-3.5">
        <div className="flex flex-wrap items-baseline gap-3">
          <h1 className="text-xl font-semibold">Runs</h1>
          <span className="text-muted-foreground">{`${running} running`}</span>
        </div>
        <ul className="flex flex-col gap-1.5">
          {RUNS.map((run) => (
            <li
              key={run.id}
              className="grid grid-cols-[minmax(180px,260px)_1fr_auto] items-center gap-3 rounded-lg border px-2.5 py-2"
            >
              <div className="flex min-w-0 flex-col">
                <span className="inline-flex items-center gap-1.5 font-semibold">
                  {run.label}
                  {run.live ? (
                    <span role="img" aria-label="Running" className="size-2 rounded-full bg-primary" />
                  ) : null}
                </span>
                <span className="truncate text-xs text-primary underline underline-offset-2">
                  {`${run.task} · ${run.title}`}
                </span>
              </div>
              <ol className="flex h-6 min-w-0 gap-[3px]">
                {run.phases.map((phase) => (
                  <li
                    key={phase.name}
                    style={{ flex: phase.mins }}
                    className={cn(
                      'flex min-w-0 items-center overflow-hidden rounded-[5px] px-1.5 text-xs whitespace-nowrap',
                      PHASE_STYLE[phase.state],
                    )}
                  >
                    {phaseWords(phase.name, phase.state, phase.mins)}
                  </li>
                ))}
              </ol>
              <span className="text-xs text-muted-foreground">{run.when}</span>
            </li>
          ))}
        </ul>
      </Card>
    </div>
  )
}
