// The empty board (ts-0194-qp; note 13; the mock's `emptyHtml`), shown when the board has no task: the choices for a
// first one. Plan it first arrives with planning sessions (Stage 9d), so its button is there and off. Start with a task
// makes a scratch task. The sources Collide found are the ones the board lists; Add a pack opens the window of the
// Settings page. It is a function of its props.
import { useId, useState } from 'react'
import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'
import { Textarea } from '@/components/ui/textarea'
import type { SourceDecl } from '../field/types.ts'

export interface EmptyBoardProps {
  /** The board's folder. */
  readonly folder: string
  readonly sources: readonly SourceDecl[]
  /** Makes a scratch task of the words. Resolves with the daemon's words when it did not. */
  readonly onScratch: (prompt: string) => Promise<string | null>
  readonly onAddPack: () => void
}

export function EmptyBoard({ folder, sources, onScratch, onAddPack }: EmptyBoardProps) {
  const planId = useId()
  const taskId = useId()
  const [task, setTask] = useState('')
  const [problem, setProblem] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const add = async () => {
    setBusy(true)
    setProblem(null)
    try {
      const failed = await onScratch(task.trim())
      if (failed === null) setTask('')
      else setProblem(failed)
    } finally {
      setBusy(false)
    }
  }
  return (
    <Card aria-label="Empty board" className="mx-auto w-full max-w-3xl px-6">
      <h1 className="text-center text-xl font-semibold">Nothing on this board yet</h1>
      <p className="text-center text-muted-foreground">
        Collide opened <b>{folder}</b> with the built-in packs. Plan what to build, add a first task, or bring in the
        tasks you already keep somewhere else.
      </p>
      <div className="grid gap-3 sm:grid-cols-2">
        <section aria-label="Plan it first" className="flex flex-col gap-2 rounded-lg border border-primary/40 p-3">
          <div className="flex items-center gap-2">
            <b>Plan it first</b>
            <span className="rounded-full bg-muted px-2 text-xs">most people start here</span>
          </div>
          <label htmlFor={planId} className="text-sm text-muted-foreground">
            What are you building? A planning session talks it through with Claude Code and writes a proposal:
            sections, features, then tasks.
          </label>
          <Textarea
            id={planId}
            disabled
            placeholder="For example: a pantry app that reminds you before food expires"
          />
          <div className="flex items-center gap-2">
            <Button disabled title="Planning sessions arrive in Stage 9d">
              Start a planning session
            </Button>
            <span className="text-xs text-muted-foreground">
              <span className="rounded bg-muted px-1 font-semibold">9d</span> Arrives in Stage 9d.
            </span>
          </div>
        </section>
        <section aria-label="Start with a task" className="flex flex-col gap-2 rounded-lg border p-3">
          <b>Start with a task</b>
          <label htmlFor={taskId} className="text-sm text-muted-foreground">
            What needs doing? A scratch task stays on your machine until you share it.
          </label>
          <Textarea
            id={taskId}
            value={task}
            onChange={(event) => setTask(event.target.value)}
            placeholder="For example: the sign-in page forgets where you were going"
          />
          {problem === null ? null : (
            <p role="alert" className="whitespace-pre-wrap text-destructive">
              {problem}
            </p>
          )}
          <div>
            <Button variant="outline" disabled={busy || task.trim() === ''} onClick={() => void add()}>
              Add as a scratch task
            </Button>
          </div>
        </section>
      </div>
      <div className="grid gap-3 sm:grid-cols-2">
        <section aria-label="Sources Collide found" className="flex flex-col gap-1 rounded-lg bg-muted p-3">
          <b>Sources Collide found</b>
          {sources.length === 0 ? (
            <span className="text-sm text-muted-foreground">Reading the sources…</span>
          ) : (
            <ul className="text-sm">
              {sources.map((source) => (
                <li key={source.name}>
                  {source.name}
                  {source.pack == null ? null : <span className="text-muted-foreground">{` · ${source.pack}`}</span>}
                </li>
              ))}
            </ul>
          )}
        </section>
        <section aria-label="Add a pack" className="flex flex-col items-start gap-2 rounded-lg bg-muted p-3">
          <b>Add a pack</b>
          <span className="text-sm text-muted-foreground">
            A pack brings tasks from another place, or a workflow. GitHub arrives in Stage 5.
          </span>
          <Button variant="outline" size="sm" onClick={onAddPack}>
            Add a pack…
          </Button>
        </section>
      </div>
    </Card>
  )
}
