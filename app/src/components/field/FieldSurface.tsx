// Field (0.11): the corpus through a view, tasks as the primary objects. Select tasks, then Launch. A read-only Field
// ships first (Stage 6a) and Launch arrives with the run UI (Stage 9a); here the board is placeholder rendering over the
// generated `Task` type, grouped by the category a person would name ("Ready to start") before any id.
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { cn } from '@/lib/utils'
import type { Highlight, Lens } from '../../ui/index.ts'
import { LensBanner } from '../LensBanner.tsx'
import type { Task } from '../types.ts'
import { CATEGORY_WORDS } from '../words.ts'

export interface FieldSurfaceProps {
  readonly tasks: readonly Task[]
  /** Keys of the tasks a person selected, from the reducer's `selectedTasks`. */
  readonly selected: readonly string[]
  readonly lens: Lens | null
  readonly highlight: Highlight | null
  readonly onToggleTask: (key: string) => void
  readonly onLaunch: (keys: readonly string[]) => void
  readonly onClearLens: () => void
}

const CATEGORIES = ['ready', 'active', 'review', 'backlog', 'done'] as const

export function FieldSurface(props: FieldSurfaceProps) {
  const { tasks, selected, lens, highlight } = props
  if (tasks.length === 0) return <EmptyField />
  const marked = new Set(highlight?.ids ?? [])
  return (
    <section aria-labelledby="field-heading" className="flex flex-col gap-4">
      <header className="flex items-center justify-between gap-2">
        <h2 id="field-heading" className="text-xl font-semibold">
          Field
        </h2>
        <Button disabled={selected.length === 0} onClick={() => props.onLaunch(selected)}>
          {selected.length === 0 ? 'Launch' : `Launch ${selected.length} ${selected.length === 1 ? 'task' : 'tasks'}`}
        </Button>
      </header>
      <LensBanner lens={lens} highlight={highlight} onClearLens={props.onClearLens} />
      <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
        {CATEGORIES.map((category) => {
          const inCategory = tasks.filter((t) => t.state.category === category)
          if (inCategory.length === 0) return null
          return (
            <div key={category} className="flex flex-col gap-2">
              <h3 className="text-sm font-medium text-muted-foreground">{CATEGORY_WORDS[category]}</h3>
              {inCategory.map((task) => (
                <TaskCard
                  key={task.key}
                  task={task}
                  selected={selected.includes(task.key)}
                  marked={marked.has(task.key)}
                  onToggle={() => props.onToggleTask(task.key)}
                />
              ))}
            </div>
          )
        })}
      </div>
    </section>
  )
}

interface TaskCardProps {
  readonly task: Task
  readonly selected: boolean
  readonly marked: boolean
  readonly onToggle: () => void
}

/** A task being worked on carries a lit border (0.11); the ring here is the placeholder for it. */
function TaskCard({ task, selected, marked, onToggle }: TaskCardProps) {
  return (
    <Card size="sm" className={cn(selected && 'ring-2 ring-primary', task.state.category === 'active' && 'ring-2 ring-ring')}>
      <CardHeader>
        <CardTitle>{task.title}</CardTitle>
        <CardDescription>{task.key}</CardDescription>
      </CardHeader>
      <CardContent className="flex-row items-center justify-between">
        <span className="flex flex-wrap gap-1">
          {task.labels.map((label) => (
            <Badge key={label} variant="secondary">
              {label}
            </Badge>
          ))}
          {marked ? <Badge variant="outline">Highlighted</Badge> : null}
        </span>
        <Button variant={selected ? 'default' : 'outline'} size="sm" aria-pressed={selected} onClick={onToggle}>
          {selected ? 'Selected' : 'Select'}
        </Button>
      </CardContent>
    </Card>
  )
}

/** An empty board still offers a way in (0.11): a prompt box, New session, the sources detected, and Add a pack. */
function EmptyField() {
  return (
    <section aria-labelledby="field-heading" className="mx-auto flex max-w-xl flex-col gap-4">
      <h2 id="field-heading" className="text-xl font-semibold">
        Field
      </h2>
      <Card>
        <CardHeader>
          <CardTitle>Nothing to work on yet</CardTitle>
          <CardDescription>Describe a change, then launch it.</CardDescription>
        </CardHeader>
        <CardContent>
          <textarea
            aria-label="Describe a change"
            className="min-h-20 rounded-md border bg-background p-2 text-sm"
            placeholder="What should change?"
          />
          <div className="flex gap-2">
            <Button disabled>Launch</Button>
            <Button variant="outline">New session</Button>
            <Button variant="outline">Add a pack</Button>
          </div>
          <p className="text-sm text-muted-foreground">
            Sources Collide found in this folder appear here once the board is open (Stage 6a).
          </p>
        </CardContent>
      </Card>
    </section>
  )
}
