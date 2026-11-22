// The run: its state and its phase in words, with the prompt. The engineer's detail (ids, branch, folder, session) is
// behind a disclosure (0.11, "Who it is for").
import { Badge } from '@/components/ui/badge'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { runStateWords } from '../components/words.ts'
import type { Followed, RunSummary } from './types.ts'
import { phaseWords } from './words.ts'

export interface RunCardProps {
  readonly run: RunSummary
  readonly followed: Followed
}

/** What the Task row says. A file run has no task: it says what it ran from. */
function taskWords(run: RunSummary, followed: Followed): string {
  if (run.tasks.length === 0) {
    return run.task_file == null ? 'none: a task file' : `none: a task file, ${run.task_file}`
  }
  if (followed.task === null || followed.board === null) {
    return `${followed.task ?? run.tasks[0]?.key}, not found on any board`
  }
  return followed.task
}

/** A row of the details. A row with no value is not shown, so nothing reads as a failed load. */
function Row({ label, value }: { readonly label: string; readonly value: string }) {
  if (value === '') return null
  return (
    <>
      <dt className="text-muted-foreground">{label}</dt>
      <dd>{value}</dd>
    </>
  )
}

export function RunCard({ run, followed }: RunCardProps) {
  const phase = run.phases[run.phases.length - 1]
  const instance = phase?.instances[0]
  const sessions = run.phases.flatMap((p) => p.instances.map((i) => i.session_id).filter((id) => id != null))
  return (
    <Card>
      <CardHeader>
        <CardTitle>
          <h2 className="text-lg font-semibold">The run</h2>
        </CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-3">
        {followed.prompt === null ? null : (
          <blockquote className="border-l-2 pl-3 text-sm whitespace-pre-wrap">{followed.prompt}</blockquote>
        )}
        <p className="flex flex-wrap items-center gap-2 text-sm">
          <Badge variant="secondary">{runStateWords(run.state)}</Badge>
          {phase === undefined || instance === undefined ? null : (
            <span>{`Step: ${phase.name}. ${phaseWords(instance.state)}.`}</span>
          )}
        </p>
        <details className="text-sm">
          <summary className="cursor-pointer font-medium">Details</summary>
          <dl className="mt-2 grid grid-cols-[auto_1fr] gap-x-3 gap-y-1">
            <Row label="Run" value={run.id} />
            <Row label="Task" value={taskWords(run, followed)} />
            <Row label="Workflow" value={run.workflow} />
            <Row label="Branch the work started from" value={run.line} />
            {run.workspace == null ? null : (
              <>
                <dt className="text-muted-foreground">Work folder</dt>
                <dd className="font-mono text-xs">{run.workspace}</dd>
              </>
            )}
            {sessions.map((id) => (
              <div key={id} className="contents">
                <dt className="text-muted-foreground">Session</dt>
                <dd className="font-mono text-xs">{id}</dd>
              </div>
            ))}
          </dl>
        </details>
      </CardContent>
    </Card>
  )
}
