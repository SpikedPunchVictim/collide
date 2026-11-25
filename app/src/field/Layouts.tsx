// The Field's other two layouts (ts-0187-hb; 0.11, Field; note 11 of the board mock): the run order, as numbered steps
// of cards, and the graph, as the same steps in columns with a line from each blocker to what it holds up. Both draw
// what the daemon walked (`walk.ts`), under a goal bar that says what the walk is toward. The graph's lines are svg
// attributes and classes, and a task's place is a React `style` prop, so none of it is a `<style>` or a style string
// (the content security policy, `style-src 'self'`).
import { useState } from 'react'
import { NativeSelect } from '@/components/ui/native-select'
import { cn } from '@/lib/utils'
import { CardButton } from './CardButton.tsx'
import { DOT } from './colours.ts'
import { LAYOUTS, type LayoutKind } from './layout.ts'
import { cardIdOf, cardOf, shortIdOf } from './model.ts'
import type { FieldData, Task } from './types.ts'
import { GEOMETRY, type Goal, edgeId, graphOf, lightOf, walkWords } from './walk.ts'

/** The switch between the layouts. The one shown is pressed. */
export function LayoutSwitch({ layout, onChoose }: { layout: LayoutKind; onChoose: (layout: LayoutKind) => void }) {
  return (
    <div role="group" aria-label="Layout" className="ml-auto inline-flex overflow-hidden rounded-md border">
      {LAYOUTS.map((each) => (
        <button
          key={each.id}
          type="button"
          aria-pressed={each.id === layout}
          onClick={() => onChoose(each.id)}
          className={cn(
            'cursor-pointer bg-card px-2.5 py-1 text-sm not-first:border-l',
            'focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-ring',
            each.id === layout && 'bg-primary/10 font-semibold text-primary',
          )}
        >
          {each.word}
        </button>
      ))}
    </div>
  )
}

interface GoalBarProps {
  /** The title of the goal a view has of its own, which is not for the person to change. Null when there is none. */
  readonly own: string | null
  readonly goals: readonly Goal[]
  readonly goal: string | null
  readonly onGoal: (goal: string) => void
  /** What the walk is, in words: how many tasks in how many steps, and what is walked. */
  readonly words: string
}

function GoalBar({ own, goals, goal, onGoal, words }: GoalBarProps) {
  return (
    <div role="group" aria-label="Goal" className="flex flex-wrap items-center gap-3">
      {own !== null ? (
        <>
          <span className="font-semibold">Toward</span>
          <b>{own}</b>
        </>
      ) : (
        <>
          <label htmlFor="goal" className="font-semibold">
            Toward
          </label>
          <NativeSelect
            id="goal"
            className="w-auto min-w-48"
            value={goal ?? goals[0]?.id ?? ''}
            onChange={(event) => onGoal(event.target.value)}
          >
            {goals.map((each) => (
              <option key={each.id} value={each.id}>
                {each.title}
              </option>
            ))}
          </NativeSelect>
        </>
      )}
      <span className="text-muted-foreground">{words}</span>
    </div>
  )
}

export interface WalkViewProps {
  readonly layout: 'run-order' | 'graph'
  readonly data: FieldData
  /** The tasks of each step, which the daemon walked toward the goal. */
  readonly steps: readonly (readonly Task[])[]
  /** The keys of the tasks that wait on each other, which the walk could not put in order. */
  readonly cycle: readonly string[]
  /** The title of the view's own goal, or null for a view that has none and is walked toward a pick. */
  readonly own: string | null
  readonly goals: readonly Goal[]
  readonly goal: string | null
  readonly onGoal: (goal: string) => void
  /** Why the daemon refused the walk, or null. */
  readonly problem: string | null
  /** Whether the walk has been read yet. */
  readonly read: boolean
  readonly selected: string | null
  /** The ids of the cards of the picked epic, which are lit. */
  readonly lit: ReadonlySet<string>
  readonly onSelect: (id: string) => void
}

const RUN_WORDS = 'Only open tasks are walked; finished ones are left out.'
const GRAPH_WORDS = 'A line runs from each blocker to what it holds up. Hover a task to light its lines.'

/** The run order or the graph, under its goal bar, or the words for why there is none. */
export function WalkView(props: WalkViewProps) {
  const { layout, data, steps, own, goals, problem, read } = props
  if (own === null && goals.length === 0) {
    return <p className="text-muted-foreground">Nothing open to put in order.</p>
  }
  const words = `${walkWords(steps)} ${layout === 'graph' ? GRAPH_WORDS : RUN_WORDS}`
  const bar = <GoalBar own={own} goals={goals} goal={props.goal} onGoal={props.onGoal} words={read ? words : ''} />
  if (problem !== null) {
    return (
      <div className="flex min-w-0 flex-1 flex-col gap-3">
        {bar}
        <p role="status" className="text-destructive">{`The tasks could not be put in order: ${problem}`}</p>
      </div>
    )
  }
  if (!read) {
    return (
      <div className="flex min-w-0 flex-1 flex-col gap-3">
        {bar}
        <p role="status" className="text-muted-foreground">
          Putting the tasks in order…
        </p>
      </div>
    )
  }
  return (
    <div className="flex min-w-0 flex-1 flex-col gap-3">
      {bar}
      {steps.length === 0 ? (
        <p className="text-muted-foreground">Nothing open to put in order.</p>
      ) : layout === 'graph' ? (
        <Graph {...props} />
      ) : (
        <RunOrder {...props} />
      )}
    </div>
  )
}

/** A line, and a line that is lit: the colour and width are classes of the stylesheet, so no style string is made. */
const LINE = cn(
  'fill-none stroke-input stroke-[1.6]',
  'data-[hot=true]:stroke-primary data-[hot=true]:stroke-[2.4]',
)

/** A task of the graph, which sits where `style` puts it. */
const NODE = cn(
  'absolute flex cursor-pointer flex-col justify-center gap-0.5 rounded-[10px] border bg-card px-2.5 text-left',
  'shadow-sm hover:border-input focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring',
)

const startWords = (index: number) => (index === 0 ? 'can start now' : `after step ${index}`)

function RunOrder({ data, steps, selected, lit, onSelect }: WalkViewProps) {
  return (
    <ol aria-label="Run order" className="flex flex-col gap-2.5">
      {steps.map((step, index) => (
        <li key={index} className="grid grid-cols-[150px_1fr] items-start gap-3">
          <div className="pt-2">
            <b className="block">{`Step ${index + 1}`}</b>
            <span className="text-sm text-muted-foreground">{startWords(index)}</span>
          </div>
          <div className="grid grid-cols-[repeat(auto-fill,minmax(240px,1fr))] gap-2">
            {step.map((task) => {
              const card = cardOf(task, data, 'category')
              const on = card.id === selected
              return (
                <CardButton
                  key={card.id}
                  card={card}
                  selected={on}
                  flash={false}
                  lit={lit.has(card.id)}
                  onSelect={onSelect}
                />
              )
            })}
          </div>
        </li>
      ))}
    </ol>
  )
}

function Graph({ data, steps, cycle, selected, lit: picked, onSelect }: WalkViewProps) {
  const graph = graphOf(steps, cycle)
  // The task that has the pointer or the focus: its lines are lit and the tasks not at their ends are dimmed.
  const [lit, setLit] = useState<string | null>(null)
  const { hot, near } = lightOf(graph, lit)
  const { left, W, nw, nh } = GEOMETRY
  const knotted = steps.flat().filter((task) => cycle.includes(task.key))
  const names = knotted.map((task) => task.title).join(', ')
  return (
    <>
      {knotted.length === 0 ? null : (
        <p
          role="status"
          aria-label="A cycle"
          className="rounded-lg border border-waiting/35 bg-waiting/10 px-3.5 py-2.5"
        >
          {`These tasks wait on each other, so no line is drawn among them: ${names}.`}
        </p>
      )}
      <div aria-label="Graph" className="overflow-x-auto rounded-lg border bg-muted">
        <div className="relative" style={{ width: graph.width, height: graph.height }}>
          <svg
            width={graph.width}
            height={graph.height}
            aria-hidden="true"
            className="pointer-events-none absolute inset-0"
          >
            {graph.edges.map((edge) => (
              <path
                key={edgeId(edge)}
                data-from={edge.from}
                data-to={edge.to}
                data-hot={hot.has(edgeId(edge)) ? 'true' : undefined}
                d={edge.d}
                className={LINE}
              />
            ))}
          </svg>
          {steps.map((_, index) => (
            <span
              key={index}
              className="absolute top-2 text-xs font-semibold text-muted-foreground"
              style={{ left: left + index * W }}
            >
              {index === 0 ? 'Step 1 · can start now' : `Step ${index + 1}`}
            </span>
          ))}
          {graph.nodes.map((node) => {
            const id = cardIdOf(node.task)
            const dim = lit !== null && !near.has(node.task.key)
            return (
              <button
                key={id}
                type="button"
                data-card={id}
                data-dim={dim ? 'true' : undefined}
                data-lit={picked.has(id) ? 'true' : undefined}
                aria-current={id === selected}
                title={node.task.title}
                onClick={() => onSelect(id)}
                onMouseEnter={() => setLit(node.task.key)}
                onMouseLeave={() => setLit(null)}
                onFocus={() => setLit(node.task.key)}
                onBlur={() => setLit(null)}
                style={{ left: node.x, top: node.y, width: nw, height: nh }}
                className={cn(
                  NODE,
                  'data-[dim=true]:opacity-45',
                  'data-[lit=true]:border-primary/60 data-[lit=true]:bg-primary/10',
                  node.task.state.category === 'active' && 'border-c-active ring-1 ring-c-active',
                  id === selected && 'outline-2 outline-offset-1 outline-ring',
                )}
              >
                <span className="truncate text-[13.5px] leading-snug font-medium">{node.task.title}</span>
                <span className="flex items-center gap-1.5 text-xs text-muted-foreground">
                  <span className={cn('size-2 shrink-0 rounded-full', DOT[node.task.state.category])} />
                  {`${shortIdOf(node.task, data.sources)} · ${node.task.state.status}`}
                </span>
              </button>
            )
          })}
        </div>
      </div>
    </>
  )
}
