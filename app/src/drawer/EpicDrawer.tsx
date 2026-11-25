// A picked epic in the drawer (ts-0216-ve; note 60 of the board mock): Run it, with Add to Up next off until Stage
// 11, the epic's open tasks in run order with the tasks of other epics they wait on, and Details for engineers. It
// sits in the same box as a task (`DrawerShell`), folds its parts as a task's fold, and opening one of its tasks
// leaves the epic picked. `EpicView` draws what was read and keeps nothing, so a test draws it from a scripted read.
import { useMemo, useState } from 'react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { type EpicCard, type EpicRun, cardOf, epicTitleOf, outsideSteps, runOf } from '../epics/model.ts'
import { DOT } from '../field/colours.ts'
import { type FoldStore, browserStore, memoryStore } from '../field/folds.ts'
import type { LayoutKind } from '../field/layout.ts'
import { shortIdOf } from '../field/model.ts'
import type { Collection, FieldData, Task } from '../field/types.ts'
import type { OwnerClient } from '../owner/client.ts'
import { DrawerShell } from './Drawer.tsx'
import { PartSection } from './DrawerView.tsx'
import { useEpicWalk } from './epic-walk.ts'
import { readShut, toggleShut } from './folds.ts'

export interface EpicDrawerProps {
  readonly owner: OwnerClient
  readonly boardId: string
  readonly collection: Collection
  readonly data: FieldData
  /** When the Field last changed: the epic's walk is read again with it. */
  readonly stamp: number | null
  /** Where folds and the width are kept. The webview's own, unless a test gives another. */
  readonly store?: FoldStore | null
  /** A task of the epic opens in the drawer, by its key, and the epic stays picked. */
  readonly onOpenTask: (key: string) => void
  /** See the run order and See the graph: the Field changes its layout. */
  readonly onLayout: (layout: LayoutKind) => void
  readonly onClose: () => void
}

export function EpicDrawer(props: EpicDrawerProps) {
  const { owner, boardId, collection, data, stamp, store, onOpenTask, onLayout, onClose } = props
  const kept = useMemo(() => store ?? browserStore() ?? memoryStore(), [store])
  // The folds are read each time, so a change to them is seen with no state of their own to keep in step.
  const [, remember] = useState(0)
  const { report, problem } = useEpicWalk(owner, boardId, collection.id, stamp)
  const card = useMemo(() => cardOf(collection, data.corpus.length > 0 ? data.corpus : data.tasks), [collection, data])
  return (
    <DrawerShell store={kept} onClose={onClose}>
      <EpicView
        card={card}
        data={data}
        run={runOf(collection.id, data, report)}
        read={report !== null}
        problem={problem}
        shut={readShut(kept)}
        onFlip={(part) => {
          toggleShut(kept, part)
          remember((count) => count + 1)
        }}
        onOpenTask={onOpenTask}
        onLayout={onLayout}
        onClose={onClose}
      />
    </DrawerShell>
  )
}

export interface EpicViewProps {
  readonly card: EpicCard
  readonly data: FieldData
  readonly run: EpicRun
  /** Whether the walk has been read yet. */
  readonly read: boolean
  /** Why the daemon refused the walk, or null. */
  readonly problem: string | null
  /** The parts that are folded. */
  readonly shut: readonly string[]
  readonly onFlip: (part: string) => void
  readonly onOpenTask: (key: string) => void
  readonly onLayout: (layout: LayoutKind) => void
  readonly onClose: () => void
}

const UP_NEXT_WORDS = 'Add to Up next comes in Stage 11, with groups and Autopilot. It is off until then.'

const plural = (count: number, one: string, many: string) => `${count} ${count === 1 ? one : many}`

export function EpicView(props: EpicViewProps) {
  const { card, run, shut, onFlip, onLayout, onClose } = props
  const flip = (part: string) => () => onFlip(part)
  const outside = outsideSteps(run.steps, card.id)
  return (
    <aside
      aria-label="Epic drawer"
      className={cn(
        'flex h-full max-h-[calc(100vh-1.5rem)] w-full min-w-0 flex-col rounded-lg border bg-card shadow-sm',
        'max-[900px]:max-h-none max-[900px]:rounded-none max-[900px]:shadow-xl',
      )}
    >
      <header className="flex shrink-0 flex-col gap-1.5 border-b px-4 py-3">
        <div className="flex items-center gap-2">
          <Badge variant="outline" className="border-primary/40 bg-primary/10 font-normal text-primary">
            Epic
          </Badge>
          <span className="text-sm text-muted-foreground">
            {run.open > 0 ? `${run.open} open of ${card.total}` : 'Finished'}
          </span>
          <span className="flex-1" />
          <span className="font-mono text-xs text-muted-foreground">{card.short}</span>
          <Button variant="ghost" size="sm" onClick={onClose}>
            Close
          </Button>
        </div>
        <h2 className="text-base leading-snug font-semibold break-words">{card.title}</h2>
        <p className="text-sm text-muted-foreground">Read only. Its cards are lit on the board while it is picked.</p>
      </header>
      <div className="min-h-0 flex-1 overflow-y-auto">
        <PartSection id="epic-run" title="Run it" folded={shut.includes('epic-run')} onFlip={flip('epic-run')}>
          <p className="pb-2.5">
            {run.open > 0
              ? `Adds its ${plural(run.open, 'open task', 'open tasks')}${
                  outside > 0 ? `, and the ${outside} of other epics they wait on,` : ''
                } to Up next as one group, in run order.`
              : 'Nothing is open in it.'}
          </p>
          <div className="flex flex-wrap gap-2">
            <Button size="sm" disabled aria-describedby="epic-up-next-words">
              Add to Up next
            </Button>
            <Button variant="outline" size="sm" onClick={() => onLayout('run-order')}>
              See the run order
            </Button>
            <Button variant="outline" size="sm" onClick={() => onLayout('graph')}>
              See the graph
            </Button>
          </div>
          <p id="epic-up-next-words" className="pt-2.5 text-xs text-muted-foreground">
            {UP_NEXT_WORDS}
          </p>
        </PartSection>
        <PartSection
          id="epic-tasks"
          title="Tasks in run order"
          folded={shut.includes('epic-tasks')}
          onFlip={flip('epic-tasks')}
        >
          <Steps {...props} />
        </PartSection>
        <PartSection
          id="details"
          title="Details for engineers"
          folded={shut.includes('details')}
          onFlip={flip('details')}
        >
          <dl className="grid grid-cols-[minmax(0,5rem)_minmax(0,1fr)] gap-x-3 gap-y-1.5">
            {card.path === null ? null : (
              <div className="contents">
                <dt className="text-muted-foreground">File</dt>
                <dd className="font-mono text-xs break-all">{card.path}</dd>
              </div>
            )}
            <div className="contents">
              <dt className="text-muted-foreground">Tasks</dt>
              <dd>{card.total}</dd>
            </div>
          </dl>
        </PartSection>
      </div>
    </aside>
  )
}

function Steps({ card, data, run, read, problem, onOpenTask }: EpicViewProps) {
  if (problem !== null) {
    return <p role="alert" className="text-destructive">{`The run order could not be read: ${problem}`}</p>
  }
  if (!read) return <p role="status" className="text-muted-foreground">Reading the run order…</p>
  return (
    <>
      {run.steps.length === 0 ? <p className="text-muted-foreground">No open tasks.</p> : null}
      {run.steps.map((step, index) => (
        <div key={index} data-step={index + 1} className="flex flex-col gap-0.5 pb-2.5">
          <span className="text-xs text-muted-foreground">
            {step.length > 1 ? `Step ${index + 1}, ${step.length} that can go side by side` : `Step ${index + 1}`}
          </span>
          {step.map((task) => (
            <TaskRow key={task.key} task={task} card={card} data={data} onOpen={onOpenTask} />
          ))}
        </div>
      ))}
      {run.finished.length === 0 ? null : (
        <details>
          <summary className="cursor-pointer text-sm text-muted-foreground">
            {`${run.finished.length} done or dropped`}
          </summary>
          {run.finished.map((task) => (
            <TaskRow key={task.key} task={task} card={card} data={data} onOpen={onOpenTask} />
          ))}
        </details>
      )}
    </>
  )
}

interface TaskRowProps {
  readonly task: Task
  readonly card: EpicCard
  readonly data: FieldData
  readonly onOpen: (key: string) => void
}

/** A task of the epic, or of another epic it waits on, which says whose it is. */
function TaskRow({ task, card, data, onOpen }: TaskRowProps) {
  const own = task.collections.includes(card.id)
  return (
    <button
      type="button"
      data-open={task.key}
      onClick={() => onOpen(task.key)}
      className={cn(
        'flex w-full cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-left',
        'hover:bg-muted focus-visible:outline-2 focus-visible:outline-ring',
      )}
    >
      <span aria-hidden="true" className={cn('size-2 shrink-0 rounded-full', DOT[task.state.category])} />
      <span className="font-mono text-xs text-muted-foreground">{shortIdOf(task, data.sources)}</span>
      <span className="min-w-0 flex-1">{task.title}</span>
      {own ? null : (
        <Badge variant="outline" className="font-normal text-muted-foreground">
          {epicTitleOf(task, data.collections)}
        </Badge>
      )}
      <span className="text-xs text-muted-foreground">{task.state.status}</span>
    </button>
  )
}
