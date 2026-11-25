// The Epic box (ts-0190-tz; note 62 of the board mock): the epics the view's tasks are in, each to tick with its
// count of tasks, in a box that holds fifty as well as three. The choice is `Filters.epics`, kept with the other
// choices, so the strip of `ts-0216-ve` sets the same one. An epic is a collection a source lists (`plan:<epic id>`),
// shown by its title.
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { cn } from '@/lib/utils'
import type { Collection } from '../field/types.ts'
import { type Filters, type Groups, epicWords, toggle } from './model.ts'

/** Over this many epics the box has a find box. */
export const FIND_OVER = 8

export interface EpicBoxProps {
  readonly epics: Groups['epics']
  readonly filters: Filters
  readonly collections: readonly Collection[]
  readonly onFilters: (next: Filters) => void
}

const BOX = 'flex size-4 shrink-0 items-center justify-center rounded border'

export function EpicBox({ epics, filters, collections, onFilters }: EpicBoxProps) {
  const [find, setFind] = useState('')
  const words = epicWords(filters, collections)
  const chosen = filters.epics.length
  const clear = () => onFilters({ ...filters, epics: [] })
  const wanted = find.trim().toLowerCase()
  const listed = epics.filter((each) => wanted === '' || `${each.id} ${each.title}`.toLowerCase().includes(wanted))
  return (
    <div role="group" aria-label="Epic" className="flex flex-wrap items-center gap-1.5">
      <span className="text-xs font-semibold tracking-wide text-muted-foreground uppercase">Epic</span>
      <Popover onOpenChange={() => setFind('')}>
        <PopoverTrigger asChild>
          <Button
            variant="outline"
            size="sm"
            aria-label={`Epic: ${words}`}
            className={cn(chosen > 0 && 'border-primary bg-primary/10')}
          >
            <span className="max-w-48 truncate">{words}</span>
          </Button>
        </PopoverTrigger>
        <PopoverContent aria-label="Epics to show" className="flex flex-col gap-1.5 p-2">
          {epics.length > FIND_OVER ? (
            <>
              <label htmlFor="epic-find" className="sr-only">
                Find an epic
              </label>
              <Input
                id="epic-find"
                type="search"
                autoComplete="off"
                placeholder="Find an epic"
                value={find}
                onChange={(event) => setFind(event.target.value)}
              />
            </>
          ) : null}
          <div role="group" aria-label="Epics" className="flex max-h-64 flex-col overflow-y-auto">
            {listed.length === 0 ? <p className="px-2 py-1.5 text-muted-foreground">No epic matches.</p> : null}
            {listed.map((each) => {
              const on = filters.epics.includes(each.id)
              return (
                <button
                  key={each.id}
                  type="button"
                  role="checkbox"
                  aria-checked={on}
                  onClick={() => onFilters({ ...filters, epics: toggle(filters.epics, each.id) })}
                  className="flex cursor-pointer items-center gap-2 rounded-md px-2 py-1 text-left hover:bg-muted"
                >
                  <span
                    aria-hidden="true"
                    className={cn(BOX, on && 'bg-primary')}
                  >
                    {on ? <Tick /> : null}
                  </span>
                  <span className="min-w-0 flex-1 truncate">{each.title}</span>
                  <span className="text-muted-foreground">{each.count}</span>
                </button>
              )
            })}
          </div>
          <div className="flex items-center gap-2 border-t px-1 pt-1.5">
            <span className="flex-1 text-xs text-muted-foreground">
              {chosen > 0 ? `${chosen} chosen` : 'None chosen: every epic shows'}
            </span>
            <Button variant="ghost" size="sm" disabled={chosen === 0} onClick={clear}>
              Clear
            </Button>
          </div>
        </PopoverContent>
      </Popover>
    </div>
  )
}

function Tick() {
  return (
    <svg aria-hidden="true" viewBox="0 0 16 16" fill="none" stroke="white" strokeWidth="2.5" className="size-3">
      <path d="M3 8.5l3 3 7-7" />
    </svg>
  )
}
