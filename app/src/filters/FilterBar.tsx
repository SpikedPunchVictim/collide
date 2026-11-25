// The filter bar (ts-0190-tz; 0.11, Filter bar; notes 9, 10, 38 and 62 of the board mock): chips for the epic, where a
// task stands, where it comes from and its labels, a search, and what the choices come to as an expression. The bar
// only chooses; the board is the view read through the expression (`../field/load.ts`). Its header folds like the
// drawer's part headers, and the fold is kept for each board.
import { type ReactNode, useEffect, useMemo, useRef, useState } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { cn } from '@/lib/utils'
import { Chevron } from '../drawer/DrawerView.tsx'
import { DOT } from '../field/colours.ts'
import { type FoldStore } from '../field/folds.ts'
import type { FieldData, ViewEntry } from '../field/types.ts'
import type { ViewInput } from '../owner/generated.ts'
import { readBarFolded, setBarFolded } from './fold.ts'
import { EpicBox } from './EpicBox.tsx'
import { isSearchKey } from './keys.ts'
import {
  type Filters,
  NO_FILTERS,
  cannotSave,
  celOf,
  groupsOf,
  isFiltering,
  saveOf,
  summaryOf,
  toggle,
} from './model.ts'

export interface FilterBarProps {
  readonly boardId: string
  readonly data: FieldData
  /** The selected view as the board lists it, which Save as a view starts from. Undefined for the whole board. */
  readonly entry: ViewEntry | undefined
  readonly filters: Filters
  readonly onFilters: (next: Filters) => void
  /** Writes a view of the person's own, and finishes when it has. A refusal is the words it throws. */
  readonly onSave: (view: ViewInput) => Promise<void>
  readonly store: FoldStore | null
}

const taskCount = (n: number) => `${n} task${n === 1 ? '' : 's'}`

export function FilterBar(props: FilterBarProps) {
  const { boardId, data, entry, filters, onFilters, onSave, store } = props
  const [folded, setFolded] = useState(() => readBarFolded(store, boardId))
  const search = useRef<HTMLInputElement>(null)
  const wantsFocus = useRef(false)
  const groups = useMemo(() => groupsOf(data.inView, data.collections), [data.inView, data.collections])
  const shown = new Set(data.tasks.map((task) => task.key)).size
  const inView = new Set(data.inView.map((task) => task.key)).size
  const filtering = isFiltering(filters)
  const flip = (next: boolean) => {
    setBarFolded(store, boardId, next)
    setFolded(next)
  }
  const set = (over: Partial<Filters>) => onFilters({ ...filters, ...over })

  // `/` goes to the search, and opens the bar if it is folded.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (!isSearchKey(event)) return
      event.preventDefault()
      if (folded) {
        wantsFocus.current = true
        flip(false)
      } else search.current?.focus()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
    // `flip` only reads the board and the store, which are what the effect is made again for.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [folded, boardId, store])
  useEffect(() => {
    if (!folded && wantsFocus.current) {
      wantsFocus.current = false
      search.current?.focus()
    }
  }, [folded])

  const lines = summaryOf(filters, data.collections)
  const words = filtering
    ? `${lines.join(' · ')} · showing ${shown} of ${inView}`
    : `No filters · ${taskCount(inView)}`
  const expression = celOf(filters)
  return (
    <section aria-label="Filters" className="rounded-lg border bg-card text-card-foreground">
      <h3 className="flex items-center gap-2 px-3 py-1.5 text-sm font-semibold">
        <button
          type="button"
          aria-expanded={!folded}
          title={folded ? 'Show the filters (key /)' : 'Fold the filters'}
          onClick={() => flip(!folded)}
          className={cn(
            'flex min-w-0 flex-1 cursor-pointer items-center gap-1.5 py-1 text-left',
            'focus-visible:outline-2 focus-visible:outline-ring',
          )}
        >
          <Chevron folded={folded} />
          <span>Filters</span>
          {folded ? <span className="truncate font-normal text-muted-foreground">{words}</span> : null}
        </button>
        {folded && filtering ? (
          <Button variant="outline" size="sm" onClick={() => onFilters(NO_FILTERS)}>
            Clear filters
          </Button>
        ) : null}
      </h3>
      {folded ? null : (
        <div className="flex flex-col gap-2.5 border-t px-3 py-2.5">
          <div className="flex flex-wrap items-center gap-x-5 gap-y-2">
            {groups.epics.length === 0 ? null : (
              <EpicBox epics={groups.epics} filters={filters} collections={data.collections} onFilters={onFilters} />
            )}
            <Group label="Where it stands">
              {groups.categories.map((each) => (
                <Chip
                  key={each.id}
                  label={each.word}
                  count={each.count}
                  on={filters.categories.includes(each.id)}
                  dot={DOT[each.id]}
                  onClick={() => set({ categories: toggle(filters.categories, each.id) })}
                />
              ))}
            </Group>
            {groups.sources.length === 0 ? null : (
              <Group label="Comes from">
                {groups.sources.map((each) => (
                  <Chip
                    key={each.name}
                    label={each.name}
                    count={each.count}
                    on={filters.sources.includes(each.name)}
                    onClick={() => set({ sources: toggle(filters.sources, each.name) })}
                  />
                ))}
              </Group>
            )}
            {groups.labels.length === 0 ? null : (
              <Group label="Labels">
                {groups.labels.map((each) => (
                  <Chip
                    key={each.label}
                    label={each.label}
                    count={each.count}
                    on={filters.labels.includes(each.label)}
                    onClick={() => set({ labels: toggle(filters.labels, each.label) })}
                  />
                ))}
              </Group>
            )}
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <label htmlFor="filter-search" className="sr-only">
              Search tasks
            </label>
            <Input
              ref={search}
              id="filter-search"
              type="search"
              autoComplete="off"
              placeholder="Search by name or id"
              value={filters.text}
              onChange={(event) => set({ text: event.target.value })}
              className="w-64"
            />
            {filtering ? (
              <span className="text-sm text-muted-foreground">{`Showing ${shown} of ${inView}`}</span>
            ) : null}
            <span className="flex-1" />
            <Button variant="outline" size="sm" disabled={!filtering} onClick={() => onFilters(NO_FILTERS)}>
              Clear filters
            </Button>
            <SaveView entry={entry} filters={filters} onSave={onSave} />
          </div>
          {data.filterProblem === null ? null : (
            <p role="alert" className="text-sm text-destructive">
              {`The filter could not be applied: ${data.filterProblem}`}
            </p>
          )}
          <details className="text-sm">
            <summary className="cursor-pointer text-muted-foreground">
              Advanced: this view as an expression (CEL, read only)
            </summary>
            <code className="mt-1.5 block rounded-md bg-muted px-2.5 py-1.5 whitespace-pre-wrap">
              {expression ?? 'No filter: every task of the view shows.'}
            </code>
          </details>
        </div>
      )}
    </section>
  )
}

function Group({ label, children }: { readonly label: string; readonly children: ReactNode }) {
  return (
    <div role="group" aria-label={label} className="flex flex-wrap items-center gap-1.5">
      <span className="text-xs font-semibold tracking-wide text-muted-foreground uppercase">{label}</span>
      {children}
    </div>
  )
}

interface ChipProps {
  readonly label: string
  readonly count: number
  readonly on: boolean
  readonly dot?: string
  readonly onClick: () => void
}

// The name says the count in words, so a chip is never taken for a column header that carries the same two words.
function Chip({ label, count, on, dot, onClick }: ChipProps) {
  return (
    <button
      type="button"
      aria-pressed={on}
      aria-label={`${label}, ${taskCount(count)}`}
      onClick={onClick}
      className={cn(
        'inline-flex cursor-pointer items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-sm',
        'hover:bg-muted focus-visible:outline-2 focus-visible:outline-ring',
        'aria-pressed:border-primary aria-pressed:bg-primary/10',
      )}
    >
      {dot === undefined ? null : <span aria-hidden="true" className={cn('size-2 rounded-full', dot)} />}
      {label}
      <span className="text-muted-foreground">{count}</span>
    </button>
  )
}

/** Save as a view (0.11): a name, and a view of the person's own with the choices in it. */
function SaveView({
  entry,
  filters,
  onSave,
}: Pick<FilterBarProps, 'entry' | 'filters' | 'onSave'>): ReactNode {
  const [open, setOpen] = useState(false)
  const [name, setName] = useState('My view')
  const [problem, setProblem] = useState<string | null>(null)
  const blocked = cannotSave(entry)
  const save = async () => {
    const made = saveOf(entry, filters, name)
    if ('why' in made) {
      setProblem(made.why)
      return
    }
    try {
      await onSave(made.view)
      setOpen(false)
    } catch (failure) {
      setProblem(failure instanceof Error ? failure.message : String(failure))
    }
  }
  return (
    <>
      {blocked === null ? null : (
        <span role="note" className="max-w-sm text-sm text-muted-foreground">{`Save is off: ${blocked}`}</span>
      )}
      <Popover
        open={open}
        onOpenChange={(next) => {
          setOpen(next)
          setProblem(null)
        }}
      >
        <PopoverTrigger asChild>
          <Button variant="outline" size="sm" disabled={!isFiltering(filters) || blocked !== null}>
            Save as a view…
          </Button>
        </PopoverTrigger>
        <PopoverContent aria-label="Save as a view" className="flex flex-col gap-2 p-3">
          <label htmlFor="view-name" className="text-sm font-medium">
            Name
          </label>
          <Input id="view-name" value={name} autoComplete="off" onChange={(event) => setName(event.target.value)} />
          <p className="text-xs text-muted-foreground">The view is kept for you.</p>
          {problem === null ? null : (
            <p role="alert" className="text-sm text-destructive">
              {problem}
            </p>
          )}
          <div className="flex justify-end">
            <Button size="sm" onClick={() => void save()}>
              Save view
            </Button>
          </div>
        </PopoverContent>
      </Popover>
    </>
  )
}
