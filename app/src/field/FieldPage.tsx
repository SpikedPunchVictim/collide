// The Field (ts-0181-ft; 0.11; notes 3 to 7 of the board mock): the board's tasks as columns over the selected view,
// a line for what the view says of itself, and a place for the drawer, which `ts-0184-hv` fills. The columns come from
// the sources' declared statuses (`model.ts`); a fold is kept per board and view (`folds.ts`).
import { type KeyboardEvent, useEffect, useMemo, useState } from 'react'
import { Button } from '@/components/ui/button'
import { Card as Panel } from '@/components/ui/card'
import { cn } from '@/lib/utils'
import { Drawer } from '../drawer/Drawer.tsx'
import type { BoardNote } from '../drawer/types.ts'
import { FilterBar } from '../filters/FilterBar.tsx'
import { actionForKey } from '../frame/actions.ts'
import type { OwnerClient } from '../owner/client.ts'
import { CardButton } from './CardButton.tsx'
import { DOT } from './colours.ts'
import { agoWords } from './live.ts'
import { type FoldStore, browserStore, isShut, memoryStore, readFolds, toggleFold } from './folds.ts'
import { type LayoutKind, layoutOf, writeLayout } from './layout.ts'
import { LayoutSwitch, WalkView } from './Layouts.tsx'
import { type Column, type Lane, cardIdOf, lanesOf, reportLines } from './model.ts'
import { type Direction, neighbour } from './navigation.ts'
import type { FieldData, ViewEntry } from './types.ts'
import type { FieldState } from './useField.ts'
import { ALL_TASKS } from './ViewPicker.tsx'
import { goalTitle, goalsOf, stepsOf } from './walk.ts'

export interface FieldPageProps {
  /** The way to the daemon, which the drawer reads its task by. */
  readonly owner: OwnerClient
  /** The board the Field reads, or null when none is open. */
  readonly boardId: string | null
  /** How the drawer's details name the board. */
  readonly board?: BoardNote | null
  readonly field: FieldState
  /** Where folds are kept. The webview's own, unless a test gives another. */
  readonly store?: FoldStore | null
}

const ARROWS: Record<string, Direction> = {
  'card-down': 'down',
  'card-up': 'up',
  'card-right': 'right',
  'card-left': 'left',
}

export function FieldPage({ owner, boardId, board = null, field, store }: FieldPageProps) {
  if (boardId === null) {
    return (
      <Panel aria-label="Field" className="mx-auto w-full max-w-xl items-center px-6 text-center">
        <h1 className="text-xl font-semibold">No board is open</h1>
        <p className="text-muted-foreground">Open a folder from the board menu to see its tasks.</p>
      </Panel>
    )
  }
  if (field.problem !== null) {
    return (
      <p role="alert" aria-label="Field" className="text-destructive">
        {`The board could not be read: ${field.problem}`}
      </p>
    )
  }
  if (field.data === null) {
    return (
      <p role="status" aria-label="Field" className="text-muted-foreground">
        Reading the board…
      </p>
    )
  }
  return (
    <Board
      key={boardId}
      owner={owner}
      boardId={boardId}
      board={board}
      view={field.view}
      entry={field.views.find((each) => each.view.name === field.view)}
      goal={field.goal}
      onGoal={field.chooseGoal}
      filters={field.filters}
      onFilters={field.setFilters}
      onSave={field.saveView}
      data={field.data}
      groupBy={groupByOf(field)}
      flashed={field.flashed}
      updatedAt={field.updatedAt}
      store={store}
    />
  )
}

/** The `layout.groupBy` of the selected view, which overrides how the sources would group. */
function groupByOf(field: FieldState): string | null {
  const entry = field.views.find((each) => each.view.name === field.view)
  return entry?.view.layout?.groupBy ?? null
}

interface BoardProps {
  readonly owner: OwnerClient
  readonly boardId: string
  readonly board: BoardNote | null
  readonly view: string | null
  /** The selected view as the board lists it, which says its layout and its goal. Undefined for the whole board. */
  readonly entry: ViewEntry | undefined
  /** The goal picked for the run order and graph of a view with none of its own. */
  readonly goal: string | null
  readonly onGoal: (goal: string | null) => void
  readonly filters: FieldState['filters']
  readonly onFilters: FieldState['setFilters']
  readonly onSave: FieldState['saveView']
  readonly data: FieldData
  readonly groupBy: string | null
  readonly flashed: ReadonlySet<string>
  readonly updatedAt: number | null
  readonly store?: FoldStore | null
}

function Board(props: BoardProps) {
  const { owner, boardId, board, view, entry, goal, onGoal, data, groupBy, flashed, updatedAt, store } = props
  const { filters, onFilters, onSave } = props
  const kept = useMemo(() => store ?? browserStore() ?? memoryStore(), [store])
  // The folds are read each time, so a change to them is seen with no state of their own to keep in step.
  const [, remember] = useState(0)
  const folds = readFolds(kept, boardId, view)
  const layout = layoutOf(kept, boardId, view, entry)
  const ownGoal = entry?.view.closure?.goal ?? null
  const goals = goalsOf(data)
  // A view with no goal of its own is walked toward a collection: the one picked, else the first that has open tasks.
  const walking = layout !== 'board' && ownGoal === null
  const goalIds = goals.map((each) => each.id).join('\n')
  const chosenGoal = goals.some((each) => each.id === goal) ? goal : null
  const fallback = goals[0]?.id ?? null
  useEffect(() => {
    if (walking && chosenGoal === null && goal !== fallback) onGoal(fallback)
    // The goals are compared by what they name, since they are made again at each draw.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [walking, chosenGoal, goal, fallback, goalIds])
  const report = ownGoal === null ? data.walked : data.report
  const [selected, select] = useState<string | null>(null)
  const { lanes } = lanesOf(data, groupBy)
  const lines = reportLines(data.report)
  const count = data.tasks.length
  // The drawer's task is on the board, or else one a link in the drawer named that the view leaves out.
  const everyTask = data.corpus.length > 0 ? data.corpus : data.tasks
  const chosen =
    selected === null
      ? null
      : (data.tasks.find((task) => cardIdOf(task) === selected) ??
        everyTask.find((task) => cardIdOf(task) === selected) ??
        null)
  const openTask = (key: string) => {
    const task = data.tasks.find((each) => each.key === key) ?? everyTask.find((each) => each.key === key)
    if (task !== undefined) select(cardIdOf(task))
  }
  // Closing hands the focus back to the card, as the drawer's own button would otherwise lose it.
  const closeDrawer = () => {
    document.querySelectorAll<HTMLElement>('[data-card]').forEach((card) => {
      if (card.dataset.card === selected) card.focus()
    })
    select(null)
  }
  const choose = (next: LayoutKind) => {
    writeLayout(kept, boardId, view, next)
    remember((count) => count + 1)
  }
  const flip = (column: Column) => {
    toggleFold(kept, boardId, view, column)
    remember((count) => count + 1)
  }
  return (
    <section aria-label="Field" className="flex flex-col gap-3">
      {lines.length === 0 ? null : (
        <div
          role="status"
          aria-label="About this view"
          className="rounded-lg border border-waiting/35 bg-waiting/10 px-3.5 py-2.5"
        >
          {lines.map((line) => (
            <p key={line}>{line}</p>
          ))}
        </div>
      )}
      <div className="flex flex-wrap items-baseline gap-3">
        <h1 className="text-xl font-semibold">{view ?? ALL_TASKS}</h1>
        <span className="text-muted-foreground">{`${count} task${count === 1 ? '' : 's'}`}</span>
        {updatedAt === null ? null : <LiveStamp updatedAt={updatedAt} />}
        <LayoutSwitch layout={layout} onChoose={choose} />
      </div>
      <FilterBar
        boardId={boardId}
        data={data}
        entry={entry}
        filters={filters}
        onFilters={onFilters}
        onSave={onSave}
        store={kept}
      />
      <div className="flex items-start gap-3">
        {layout === 'board' ? (
          <Columns
            lanes={lanes}
            folds={folds}
            selected={selected}
            flashed={flashed}
            onSelect={select}
            onFlip={flip}
          />
        ) : (
          <WalkView
            layout={layout}
            data={data}
            steps={stepsOf(report, data)}
            cycle={report?.cycle ?? []}
            own={ownGoal === null ? null : goalTitle(data, ownGoal)}
            goals={goals}
            goal={chosenGoal}
            onGoal={onGoal}
            problem={ownGoal === null ? data.walkProblem : null}
            read={report !== null}
            selected={selected}
            onSelect={select}
          />
        )}
        {chosen === null ? null : (
          <Drawer
            owner={owner}
            boardId={boardId}
            shown={chosen}
            data={data}
            board={board}
            stamp={updatedAt}
            store={store}
            onOpenTask={openTask}
            onClose={closeDrawer}
          />
        )}
      </div>
    </section>
  )
}

/** How often the stamp's words are looked at again. */
const STAMP_TICK_MS = 5000

/** The Live stamp (note 14): the board follows the files, and when it last changed. It is a status, not an alert. */
function LiveStamp({ updatedAt }: { updatedAt: number }) {
  const [, tick] = useState(0)
  useEffect(() => {
    const timer = setInterval(() => tick((count) => count + 1), STAMP_TICK_MS)
    return () => clearInterval(timer)
  }, [])
  return (
    <span
      className="inline-flex items-center gap-1.5 text-sm text-muted-foreground"
      title="The board follows the files as they change"
    >
      <span aria-hidden="true" className="text-done motion-safe:animate-pulse">
        ●
      </span>
      {`Live · updated ${agoWords(updatedAt, Date.now())}`}
    </span>
  )
}

interface ColumnsProps {
  readonly lanes: readonly Lane[]
  readonly folds: ReturnType<typeof readFolds>
  readonly selected: string | null
  readonly flashed: ReadonlySet<string>
  readonly onSelect: (id: string) => void
  readonly onFlip: (column: Column) => void
}

function Columns({ lanes, folds, selected, flashed, onSelect, onFlip }: ColumnsProps) {
  /** Arrow keys between cards, and Enter on one, are the Field's named actions (`../frame/actions.ts`). */
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.metaKey || event.ctrlKey || event.altKey) return
    const card = (event.target as HTMLElement).closest<HTMLElement>('[data-card]')
    const action = actionForKey(event.key, 'field')
    if (card === null || action === null) return
    event.preventDefault()
    if (action === 'card-open') {
      onSelect(card.dataset.card!)
      return
    }
    const columns = [...event.currentTarget.querySelectorAll<HTMLElement>('[data-column]')]
    const cards = columns.map((column) => [...column.querySelectorAll<HTMLElement>('[data-card]')])
    const column = columns.findIndex((each) => each.contains(card))
    const row = cards[column]?.indexOf(card) ?? 0
    const to = neighbour(
      cards.map((each) => each.length),
      { column, row },
      ARROWS[action]!,
    )
    cards[to.column]?.[to.row]?.focus()
  }
  return (
    // The keys are heard on the board as a whole, from the card that has the focus.
    // eslint-disable-next-line jsx-a11y/no-static-element-interactions
    <div
      aria-label="Columns"
      className="flex min-w-0 flex-1 items-start gap-3 overflow-x-auto pb-2"
      onKeyDown={onKeyDown}
    >
      {lanes.map((lane) =>
        isShut(lane.column, folds) ? (
          <ShutColumn key={lane.column.key} lane={lane} onOpen={() => onFlip(lane.column)} />
        ) : (
          <OpenColumn
            key={lane.column.key}
            lane={lane}
            selected={selected}
            flashed={flashed}
            onSelect={onSelect}
            onFold={() => onFlip(lane.column)}
          />
        ),
      )}
    </div>
  )
}

function ShutColumn({ lane, onOpen }: { lane: Lane; onOpen: () => void }) {
  const { column, cards } = lane
  return (
    <button
      type="button"
      data-column
      title={`Show ${column.label}`}
      aria-expanded={false}
      onClick={onOpen}
      className="flex w-[52px] shrink-0 cursor-pointer flex-col items-center rounded-lg border bg-muted px-1 py-2.5"
    >
      <span className="flex rotate-180 items-center gap-2 font-semibold whitespace-nowrap [writing-mode:vertical-rl]">
        <span className={cn('size-2 rounded-full', DOT[column.category])} />
        {column.label}
        <span className="font-normal text-muted-foreground">{cards.length}</span>
      </span>
    </button>
  )
}

interface OpenColumnProps {
  readonly lane: Lane
  readonly selected: string | null
  readonly flashed: ReadonlySet<string>
  readonly onSelect: (id: string) => void
  readonly onFold: () => void
}

function OpenColumn({ lane, selected, flashed, onSelect, onFold }: OpenColumnProps) {
  const { column, cards } = lane
  return (
    <section
      data-column
      aria-label={column.label}
      className="flex w-[280px] shrink-0 flex-col gap-2 rounded-lg border bg-muted p-2.5"
    >
      <div className="flex items-start gap-2 px-1 pb-1">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-1.5 font-semibold">
            <span className={cn('size-2 rounded-full', DOT[column.category])} />
            {column.label}
          </div>
          <div className="text-xs text-muted-foreground">{column.sub}</div>
        </div>
        <span className="text-sm font-medium text-muted-foreground">{cards.length}</span>
        <Button variant="ghost" size="sm" title="Fold this column" aria-label={`Fold ${column.label}`} onClick={onFold}>
          –
        </Button>
      </div>
      {cards.length === 0 ? (
        <p className="px-1 py-1.5 text-sm text-muted-foreground">Nothing here</p>
      ) : (
        cards.map((card) => (
          <CardButton
            key={card.id}
            card={card}
            selected={card.id === selected}
            flash={flashed.has(card.id)}
            onSelect={onSelect}
          />
        ))
      )}
    </section>
  )
}
