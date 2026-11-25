// The Field (ts-0181-ft; 0.11; notes 3 to 7 of the board mock): the board's tasks as columns over the selected view,
// a line for what the view says of itself, and a place for the drawer, which `ts-0184-hv` fills. The columns come from
// the sources' declared statuses (`model.ts`); a fold is kept per board and view (`folds.ts`).
import { type KeyboardEvent, useEffect, useMemo, useState } from 'react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card as Panel } from '@/components/ui/card'
import { cn } from '@/lib/utils'
import { actionForKey } from '../frame/actions.ts'
import { agoWords } from './live.ts'
import { type FoldStore, browserStore, isShut, memoryStore, readFolds, toggleFold } from './folds.ts'
import { type Card, type Chip, type Column, type Lane, lanesOf, reportLines } from './model.ts'
import { type Direction, neighbour } from './navigation.ts'
import type { Category, FieldData } from './types.ts'
import type { FieldState } from './useField.ts'
import { ALL_TASKS } from './ViewPicker.tsx'

export interface FieldPageProps {
  /** The board the Field reads, or null when none is open. */
  readonly boardId: string | null
  readonly field: FieldState
  /** Where folds are kept. The webview's own, unless a test gives another. */
  readonly store?: FoldStore | null
}

const DOT: Record<Category, string> = {
  backlog: 'bg-c-backlog',
  ready: 'bg-c-ready',
  active: 'bg-c-active',
  review: 'bg-c-review',
  done: 'bg-c-done',
  dropped: 'bg-c-dropped',
}

const TEXT: Record<Category, string> = {
  backlog: 'text-c-backlog',
  ready: 'text-c-ready',
  active: 'text-c-active',
  review: 'text-c-review',
  done: 'text-c-done',
  dropped: 'text-c-dropped',
}

const ARROWS: Record<string, Direction> = {
  'card-down': 'down',
  'card-up': 'up',
  'card-right': 'right',
  'card-left': 'left',
}

export function FieldPage({ boardId, field, store }: FieldPageProps) {
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
      boardId={boardId}
      view={field.view}
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
  readonly boardId: string
  readonly view: string | null
  readonly data: FieldData
  readonly groupBy: string | null
  readonly flashed: ReadonlySet<string>
  readonly updatedAt: number | null
  readonly store?: FoldStore | null
}

function Board({ boardId, view, data, groupBy, flashed, updatedAt, store }: BoardProps) {
  const kept = useMemo(() => store ?? browserStore() ?? memoryStore(), [store])
  // The folds are read each time, so a change to them is seen with no state of their own to keep in step.
  const [, remember] = useState(0)
  const folds = readFolds(kept, boardId, view)
  const [selected, select] = useState<string | null>(null)
  const { lanes } = lanesOf(data, groupBy)
  const lines = reportLines(data.report)
  const count = data.tasks.length
  const chosen = lanes.flatMap((lane) => lane.cards).find((card) => card.id === selected) ?? null
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
      </div>
      <div className="flex items-start gap-3">
        <Columns
          lanes={lanes}
          folds={folds}
          selected={chosen?.id ?? null}
          flashed={flashed}
          onSelect={select}
          onFlip={flip}
        />
        {chosen === null ? null : <DrawerPlace card={chosen} onClose={() => select(null)} />}
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

interface CardButtonProps {
  readonly card: Card
  readonly selected: boolean
  /** The card changed in a read just now: it flashes once (`field-flash` in `index.css`), and says nothing. */
  readonly flash: boolean
  readonly onSelect: (id: string) => void
}

function CardButton({ card, selected, flash, onSelect }: CardButtonProps) {
  return (
    <button
      type="button"
      data-card={card.id}
      data-flash={flash ? 'true' : undefined}
      aria-current={selected}
      onClick={() => onSelect(card.id)}
      className={cn(
        'flex w-full cursor-pointer flex-col gap-1.5 rounded-[10px] border bg-card px-3 py-2.5 text-left shadow-sm',
        'hover:border-input focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring',
        // A card being worked on has the lit border (note 5).
        card.category === 'active' && 'border-c-active ring-1 ring-c-active',
        selected && 'outline-2 outline-offset-1 outline-ring',
        flash && 'field-flash',
      )}
    >
      <span
        className={cn('leading-snug font-medium', card.category === 'dropped' && 'text-muted-foreground line-through')}
      >
        {card.title}
      </span>
      <span className="text-xs text-muted-foreground">
        {card.shortId}
        {card.stage === null ? null : ` · ${card.stage}`}
      </span>
      {card.chips.length === 0 ? null : (
        <span className="flex flex-wrap gap-1.5">
          {card.chips.map((chip) => (
            <ChipView key={chip.id} chip={chip} />
          ))}
        </span>
      )}
    </button>
  )
}

function ChipView({ chip }: { chip: Chip }) {
  const style =
    chip.id === 'status'
      ? TEXT[chip.category!]
      : chip.id === 'divergence'
        ? 'border-waiting/40 text-waiting'
        : chip.id === 'collision'
          ? 'border-destructive/40 text-destructive'
          : 'text-muted-foreground'
  return (
    <Badge variant="outline" className={cn('font-normal', style)}>
      {chip.id === 'status' ? <span className={cn('size-2 rounded-full', DOT[chip.category!])} /> : null}
      {chip.id === 'divergence' ? <span aria-hidden="true">↗</span> : null}
      {chip.text}
    </Badge>
  )
}

/** The drawer's place: it opens beside the board for the selected card, and says what comes in `ts-0184-hv`. */
function DrawerPlace({ card, onClose }: { card: Card; onClose: () => void }) {
  return (
    <aside
      aria-label="Task drawer"
      className="flex w-80 shrink-0 flex-col gap-2 rounded-lg border bg-card p-3.5 shadow-sm"
    >
      <div className="flex items-start gap-2">
        <h2 className="min-w-0 flex-1 text-base font-semibold">{card.title}</h2>
        <Button variant="ghost" size="sm" onClick={onClose}>
          Close
        </Button>
      </div>
      <p className="text-xs text-muted-foreground">{card.key}</p>
      <p className="text-muted-foreground">The drawer comes next.</p>
    </aside>
  )
}
