// The epics strip (ts-0216-ve; Stage 6a, Field, Epics; notes 58, 61 and 63 of the board mock): a card for each epic
// with a task in the view, in one row that scrolls sideways, under a header that folds it as the filter bar's and the
// drawer's headers fold. A click on a card picks the epic (the Field lights its cards and opens it in the drawer),
// and the funnel on a card adds it to the filter bar's Epic box. The row keeps its place while a pick is made; it
// moves only to keep the picked card in sight (`keepInSight`). Widths are a React `style` prop, so no style string is
// made (the content security policy, `style-src 'self'`).
import { useEffect, useRef, useState } from 'react'
import { Input } from '@/components/ui/input'
import { cn } from '@/lib/utils'
import { Chevron } from '../drawer/DrawerView.tsx'
import { DOT } from '../field/colours.ts'
import type { FoldStore } from '../field/folds.ts'
import { FIND_OVER } from '../filters/EpicBox.tsx'
import { type Filters, toggle } from '../filters/model.ts'
import { readStripFolded, setStripFolded } from './fold.ts'
import { BAR_ORDER, type EpicCard, findEpics, progressWords } from './model.ts'

export interface EpicStripProps {
  readonly boardId: string
  /** The selected view's name, or null for the whole board: the fold is kept for each. */
  readonly view: string | null
  /** The epics to list, in their order. */
  readonly cards: readonly EpicCard[]
  /** The collection that is picked, or null. */
  readonly picked: string | null
  /** The drawer shows the picked epic, so its card is kept in sight. */
  readonly drawerOpen: boolean
  readonly onPick: (id: string) => void
  readonly filters: Filters
  readonly onFilters: (next: Filters) => void
  readonly store: FoldStore | null
}

const LIT_PILL = 'border-primary/40 bg-primary/10 text-primary'

/** Moves the row sideways, by as little as it takes, so the card is inside it. A card already inside moves nothing. */
export function keepInSight(row: HTMLElement, card: HTMLElement): void {
  const edge = row.getBoundingClientRect()
  const at = card.getBoundingClientRect()
  if (at.right > edge.right) row.scrollLeft += at.right - edge.right + 8
  else if (at.left < edge.left) row.scrollLeft -= edge.left - at.left + 8
}

export function EpicStrip(props: EpicStripProps) {
  const { boardId, view, cards, picked, drawerOpen, onPick, filters, onFilters, store } = props
  const [folded, setFolded] = useState(() => readStripFolded(store, boardId, view))
  const [find, setFind] = useState('')
  const row = useRef<HTMLDivElement>(null)
  const flip = () => {
    setStripFolded(store, boardId, view, !folded)
    setFolded(!folded)
  }
  const listed = findEpics(cards, find)
  const pickedCard = picked === null ? undefined : cards.find((each) => each.id === picked)

  // The pick opens the epic in the drawer: the card is then moved into sight if it is not in it.
  useEffect(() => {
    if (!drawerOpen || picked === null || row.current === null) return
    const cards = [...row.current.querySelectorAll<HTMLElement>('[data-epic]')]
    const card = cards.find((each) => each.dataset.epic === picked)
    if (card !== undefined) keepInSight(row.current, card)
  }, [drawerOpen, picked, folded])

  return (
    <section aria-label="Epics" className="rounded-lg border bg-card text-card-foreground">
      <h3 className="flex items-center gap-2 px-3 py-1.5 text-sm font-semibold">
        <button
          type="button"
          aria-expanded={!folded}
          title={folded ? 'Show the epics' : 'Fold the epics'}
          onClick={flip}
          className={cn(
            'flex min-w-0 flex-1 cursor-pointer items-center gap-1.5 py-1 text-left',
            'focus-visible:outline-2 focus-visible:outline-ring',
          )}
        >
          <Chevron folded={folded} />
          <span>Epics</span>
          <span className="font-normal text-muted-foreground">{cards.length}</span>
          {folded && pickedCard !== undefined ? (
            <span className={cn('rounded-full border px-2 py-0.5 text-xs font-normal', LIT_PILL)}>
              {`${pickedCard.short} is lit`}
            </span>
          ) : null}
        </button>
      </h3>
      {folded ? null : (
        <div className="flex flex-col gap-2.5 border-t px-3.5 py-3">
          {cards.length > FIND_OVER ? (
            <div className="flex flex-wrap items-center gap-2.5 text-sm">
              <label htmlFor="epic-strip-find" className="sr-only">
                Find an epic by name or id
              </label>
              <Input
                id="epic-strip-find"
                type="search"
                autoComplete="off"
                placeholder="Find an epic by name or id"
                value={find}
                onChange={(event) => setFind(event.target.value)}
                className="w-60 max-w-full"
              />
              <span className="text-muted-foreground">
                {find.trim() === '' ? 'Open work first' : `${listed.length} of ${cards.length}`}
              </span>
            </div>
          ) : null}
          <div ref={row} role="group" aria-label="Epic cards" className="flex gap-2.5 overflow-x-auto px-0.5 pb-2">
            {listed.length === 0 ? <span className="text-muted-foreground">No epic matches.</span> : null}
            {listed.map((card) => (
              <EpicCardView
                key={card.id}
                card={card}
                on={card.id === picked}
                only={filters.epics.includes(card.id)}
                onPick={() => onPick(card.id)}
                onOnly={() => onFilters({ ...filters, epics: toggle(filters.epics, card.id) })}
              />
            ))}
          </div>
        </div>
      )}
    </section>
  )
}

interface EpicCardViewProps {
  readonly card: EpicCard
  readonly on: boolean
  readonly only: boolean
  readonly onPick: () => void
  readonly onOnly: () => void
}

function EpicCardView({ card, on, only, onPick, onOnly }: EpicCardViewProps) {
  const words = progressWords(card)
  return (
    <div
      data-epic={card.id}
      data-on={on ? 'true' : undefined}
      className={cn(
        'relative flex w-[230px] shrink-0 rounded-[10px] border bg-card shadow-sm hover:border-input',
        'data-[on=true]:border-primary data-[on=true]:ring-2 data-[on=true]:ring-primary',
      )}
    >
      <button
        type="button"
        aria-pressed={on}
        onClick={onPick}
        className={cn(
          'flex min-w-0 flex-1 cursor-pointer flex-col gap-1.5 rounded-[10px] px-3 py-2.5 text-left',
          'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring',
        )}
      >
        <span className="line-clamp-2 min-h-[2.6em] pr-6 leading-snug font-semibold">{card.title}</span>
        <span aria-hidden="true" className="flex h-1.5 overflow-hidden rounded-full bg-secondary">
          {BAR_ORDER.filter((category) => card.counts[category] > 0).map((category) => (
            <span
              key={category}
              className={cn('block h-full', DOT[category])}
              style={{ flex: card.counts[category] }}
            />
          ))}
        </span>
        <span className="flex gap-1.5 overflow-hidden text-xs whitespace-nowrap text-muted-foreground">
          <span className="shrink-0">{words.done}</span>
          <span aria-hidden="true">·</span>
          <span className="truncate">{words.open}</span>
        </span>
        <span className="font-mono text-xs text-muted-foreground">{card.short}</span>
      </button>
      <button
        type="button"
        aria-pressed={only}
        title={only ? "Show every epic's tasks" : "Show only this epic's tasks"}
        aria-label={`Show only the tasks of ${card.title}`}
        onClick={onOnly}
        className={cn(
          'absolute top-1.5 right-1.5 grid size-[26px] cursor-pointer place-items-center rounded-md',
          'text-muted-foreground hover:bg-muted hover:text-foreground',
          'focus-visible:outline-2 focus-visible:outline-ring',
          'aria-pressed:bg-primary/10 aria-pressed:text-primary',
        )}
      >
        <Funnel />
      </button>
    </div>
  )
}

function Funnel() {
  return (
    <svg
      aria-hidden="true"
      viewBox="0 0 24 24"
      className="size-[15px]"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinejoin="round"
    >
      <path d="M3 5h18l-7 8v6l-4 2v-8z" />
    </svg>
  )
}
