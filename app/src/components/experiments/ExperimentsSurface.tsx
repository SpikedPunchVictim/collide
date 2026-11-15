// Experiments (0.11, 0.14): pick a task, then two arms; a preview before the run; a result with its scope. The generated
// types have no experiment record yet (Stage 15 defines it), so the props are typed here. The result states its scope
// ("on this task") beside the verdict, so one run is not read as a general rule (S6). Running one is Stage 15.
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'

export interface ExperimentPreview {
  readonly task: string
  readonly armA: string
  readonly armB: string
  /** `git diff --stat A...B`. */
  readonly diffStat: string
  readonly protectedPathsHit: readonly string[]
  /** True when B is not A plus its change (the merge base is not A). */
  readonly mergeBaseWarning: boolean
  readonly primaryMetric: string
  readonly otherRunsLive: number
  /** `null` reads "unknown": fewer than three earlier runs of the workflow have timings. */
  readonly expectedDurationWords: string | null
}

export interface ExperimentResult {
  readonly verdict: string
  readonly criticalPathDelta: string
  readonly outcomesEqual: boolean
}

export interface ExperimentView {
  readonly preview: ExperimentPreview
  readonly result: ExperimentResult | null
}

export interface ExperimentsSurfaceProps {
  readonly experiment: ExperimentView
  readonly onStart: () => void
}

export function ExperimentsSurface({ experiment, onStart }: ExperimentsSurfaceProps) {
  const { preview, result } = experiment
  return (
    <section aria-labelledby="experiments-heading" className="flex flex-col gap-4">
      <h2 id="experiments-heading" className="text-xl font-semibold">
        Experiments
      </h2>
      <div className="grid gap-4 md:grid-cols-2">
        <Card size="sm">
          <CardHeader>
            <CardTitle>Arm A</CardTitle>
            <CardDescription>{preview.armA}</CardDescription>
          </CardHeader>
        </Card>
        <Card size="sm">
          <CardHeader>
            <CardTitle>Arm B</CardTitle>
            <CardDescription>{preview.armB}</CardDescription>
          </CardHeader>
        </Card>
      </div>
      <Card>
        <CardHeader>
          <CardTitle>Before you run it</CardTitle>
          <CardDescription>The same task, {preview.task}, once on each arm.</CardDescription>
        </CardHeader>
        <CardContent className="text-sm">
          <p>What the arms change: </p>
          <pre className="overflow-x-auto rounded-md bg-muted p-2 font-mono text-xs">{preview.diffStat}</pre>
          {preview.protectedPathsHit.length > 0 ? <p>Protected files touched: {preview.protectedPathsHit.join(', ')}</p> : null}
          {preview.mergeBaseWarning ? <p className="font-medium">Warning: arm B is not arm A plus one change.</p> : null}
          <p>It will compare: {preview.primaryMetric}</p>
          <p>Other runs working right now: {preview.otherRunsLive}</p>
          <p>Expected duration: {preview.expectedDurationWords ?? 'unknown'}</p>
          <div>
            <Button onClick={onStart}>Run the experiment</Button>
          </div>
        </CardContent>
      </Card>
      {result ? (
        <Card>
          <CardHeader>
            <CardTitle>Result</CardTitle>
            <CardDescription>{result.verdict} (on this task only)</CardDescription>
          </CardHeader>
          <CardContent className="text-sm">
            <p>Critical path: {result.criticalPathDelta}</p>
            <p>{result.outcomesEqual ? 'Both arms ended the same way.' : 'The arms ended differently.'}</p>
          </CardContent>
        </Card>
      ) : null}
    </section>
  )
}
