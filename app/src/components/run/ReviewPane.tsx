// Review (0.11): the run's report, its gates, the findings of reviewer A and reviewer B, and the diff, then Accept,
// Send back (re-prompt) or Discard. "Review x2" is two reviewers who work in parallel, not two rounds (S6, Q5). The
// generated types carry the gate verdicts only, so the report and findings are typed here until Stage 9a fills them.
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import type { RunSummary } from '../types.ts'

export type ReviewDecision = 'accept' | 'reprompt' | 'discard'

export interface ReviewData {
  readonly report: string
  readonly reviewerA: readonly string[]
  readonly reviewerB: readonly string[]
  readonly diffSummary: string
}

export interface ReviewPaneProps {
  readonly run: RunSummary
  readonly review: ReviewData
  readonly onDecide: (runId: string, decision: ReviewDecision) => void
}

export function ReviewPane({ run, review, onDecide }: ReviewPaneProps) {
  const gates = run.phases.flatMap((p) => Object.entries(p.gate_results).map(([name, status]) => ({ phase: p.name, name, status })))
  return (
    <div className="flex flex-col gap-4">
      <section className="flex flex-col gap-1">
        <h4 className="text-sm font-medium">Report</h4>
        <p className="text-sm whitespace-pre-wrap">{review.report}</p>
      </section>
      {gates.length > 0 ? (
        <section className="flex flex-col gap-1">
          <h4 className="text-sm font-medium">Checks</h4>
          <ul className="flex flex-wrap gap-2">
            {gates.map((g) => (
              <li key={`${g.phase}-${g.name}`}>
                <Badge variant={g.status === 'ok' ? 'secondary' : 'destructive'}>
                  {g.name}: {g.status}
                </Badge>
              </li>
            ))}
          </ul>
        </section>
      ) : null}
      <div className="grid gap-4 md:grid-cols-2">
        <Findings title="Reviewer A" findings={review.reviewerA} />
        <Findings title="Reviewer B" findings={review.reviewerB} />
      </div>
      <details className="text-sm">
        <summary className="cursor-pointer font-medium">Changes</summary>
        <pre className="mt-2 overflow-x-auto rounded-md bg-muted p-2 font-mono text-xs">{review.diffSummary}</pre>
      </details>
      <div className="flex flex-wrap gap-2">
        <Button onClick={() => onDecide(run.id, 'accept')}>Accept</Button>
        <Button variant="outline" onClick={() => onDecide(run.id, 'reprompt')}>
          Send back
        </Button>
        <Button variant="outline" onClick={() => onDecide(run.id, 'discard')}>
          Discard
        </Button>
      </div>
    </div>
  )
}

function Findings({ title, findings }: { readonly title: string; readonly findings: readonly string[] }) {
  return (
    <section className="flex flex-col gap-1">
      <h4 className="text-sm font-medium">{title}</h4>
      {findings.length === 0 ? (
        <p className="text-sm text-muted-foreground">No findings.</p>
      ) : (
        <ul className="flex list-disc flex-col gap-1 pl-4 text-sm">
          {findings.map((f) => (
            <li key={f}>{f}</li>
          ))}
        </ul>
      )}
    </section>
  )
}
