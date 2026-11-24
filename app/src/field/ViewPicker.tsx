// The view picker of the trail (ts-0181-ft; note 1; 0.11, the Field): the whole board first, then the board's views,
// the built-in ones before the rest, each named. Choosing one only changes which view the Field shows.
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import type { ViewEntry } from './types.ts'

/** What the whole board is called: no view, every task. */
export const ALL_TASKS = 'All tasks'

export interface ViewPickerProps {
  readonly boardName: string
  readonly views: readonly ViewEntry[]
  /** The selected view's name, or null for the whole board. */
  readonly view: string | null
  readonly onChoose: (view: string | null) => void
}

/** Where a view is kept, in a few words. */
export function viewSource(entry: ViewEntry): string {
  switch (entry.source.type) {
    case 'builtin':
      return 'built in'
    case 'pack':
      return `from ${entry.source.pack}`
    case 'personal':
      return 'yours'
    case 'repo':
      return 'in this repo'
  }
}

export function ViewPicker(props: ViewPickerProps) {
  const [open, setOpen] = useState(false)
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button variant="ghost" size="sm" title="The view of this board">
          {props.view ?? ALL_TASKS}
          <ChevronDown />
        </Button>
      </PopoverTrigger>
      <PopoverContent aria-label="Views">
        <ViewPickerBody
          {...props}
          onChoose={(view) => {
            props.onChoose(view)
            setOpen(false)
          }}
        />
      </PopoverContent>
    </Popover>
  )
}

/** What the menu holds, apart from the popover that shows it, so a test renders it for any views. */
export function ViewPickerBody({ boardName, views, view, onChoose }: ViewPickerProps) {
  const row = (name: string | null, label: string, sub: string, key: string) => (
    <button
      key={key}
      type="button"
      aria-current={name === view}
      onClick={() => onChoose(name)}
      className="flex w-full items-center rounded-md px-2 py-1.5 text-left hover:bg-muted aria-[current=true]:bg-muted"
    >
      <span className="min-w-0 flex-1">
        <span className="block truncate font-medium">{label}</span>
        <span className="block truncate text-xs text-muted-foreground">{sub}</span>
      </span>
    </button>
  )
  return (
    <div className="flex flex-col">
      <h4 className="mx-2 mt-1.5 mb-1 text-xs font-semibold tracking-wide text-muted-foreground uppercase">
        {`Views of ${boardName}`}
      </h4>
      {row(null, ALL_TASKS, 'every task of the board', 'all')}
      {views.map((entry, index) => {
        const sub = entry.error == null ? viewSource(entry) : `${viewSource(entry)} · has an error`
        return row(entry.view.name, entry.view.name, sub, String(index))
      })}
    </div>
  )
}

function ChevronDown() {
  return (
    <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="size-4">
      <path d="m6 9 6 6 6-6" />
    </svg>
  )
}
