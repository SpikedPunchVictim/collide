// The view picker of the trail (ts-0181-ft; note 1; 0.11, the Field): the whole board first, then the board's views,
// the built-in ones before the rest, each named. Choosing one only changes which view the Field shows.
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import type { ViewInput } from '../owner/generated.ts'
import type { ViewEntry } from './types.ts'

/** What the whole board is called: no view, every task. */
export const ALL_TASKS = 'All tasks'

export interface ViewPickerProps {
  readonly boardName: string
  readonly views: readonly ViewEntry[]
  /** The selected view's name, or null for the whole board. */
  readonly view: string | null
  readonly onChoose: (view: string | null) => void
  /** The board's line has a Collide folder, so a personal view has Share with team. */
  readonly shared?: boolean
  /** Writes the view to the repo's Collide folder and gives the file. A refusal is the words it throws. */
  readonly onShare?: (view: ViewInput) => Promise<string>
  /** The menu was opened: the views are read again, as a shared view is listed once it is committed. */
  readonly onOpen?: () => void
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

/**
 * The rows the picker lists. After a personal view is shared, the daemon still uses the personal one and lists the repo
 * one of the same name with an error that says so. When the two say the same, they are one row: the repo view, with
 * the counts and error of the one the daemon uses. When they differ both stay, since they are not the same view.
 */
export function rowsOf(views: readonly ViewEntry[]): ViewEntry[] {
  const same = (a: ViewEntry, b: ViewEntry) =>
    a.view.name === b.view.name && JSON.stringify(a.view) === JSON.stringify(b.view)
  const shared = (personal: ViewEntry) =>
    views.find((each) => each.source.type === 'repo' && same(each, personal))
  const merged = new Set<ViewEntry>()
  const rows: ViewEntry[] = []
  for (const entry of views) {
    if (merged.has(entry)) continue
    const repo = entry.source.type === 'personal' ? shared(entry) : undefined
    if (repo === undefined) {
      rows.push(entry)
      continue
    }
    merged.add(repo)
    rows.push({ ...entry, source: repo.source })
  }
  return rows
}

export function ViewPicker(props: ViewPickerProps) {
  const [open, setOpen] = useState(false)
  return (
    <Popover
      open={open}
      onOpenChange={(next) => {
        setOpen(next)
        if (next) props.onOpen?.()
      }}
    >
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
export function ViewPickerBody({ boardName, views, view, onChoose, shared = false, onShare }: ViewPickerProps) {
  // What sharing a view came to, by view name: the file it was written to, or the words of why it was not.
  const [shares, setShares] = useState<ReadonlyMap<string, { readonly ok: boolean; readonly words: string }>>(new Map())
  const share = async (entry: ViewEntry) => {
    if (onShare === undefined) return
    const note = (ok: boolean, words: string) => setShares((had) => new Map(had).set(entry.view.name, { ok, words }))
    try {
      const path = await onShare(entry.view)
      note(true, `Shared as ${path}. Commit it to share it.`)
    } catch (failure) {
      note(false, failure instanceof Error ? failure.message : String(failure))
    }
  }
  const row = (name: string | null, label: string, sub: string, key: string, entry?: ViewEntry) => {
    const said = entry === undefined ? undefined : shares.get(entry.view.name)
    return (
      <div key={key} className="flex flex-col">
        <div className="flex items-center rounded-md hover:bg-muted has-[[aria-current=true]]:bg-muted">
          <button
            type="button"
            aria-current={name === view}
            onClick={() => onChoose(name)}
            className="flex min-w-0 flex-1 items-center rounded-md px-2 py-1.5 text-left"
          >
            <span className="min-w-0 flex-1">
              <span className="block truncate font-medium">{label}</span>
              <span className="block truncate text-xs text-muted-foreground">{sub}</span>
            </span>
          </button>
          {shared && entry?.source.type === 'personal' && onShare !== undefined ? (
            <Button
              variant="ghost"
              size="sm"
              aria-label={`Share ${label} with team`}
              title="Write this view to the repo's Collide folder, for you to commit"
              onClick={() => void share(entry)}
            >
              Share with team
            </Button>
          ) : null}
        </div>
        {said === undefined ? null : (
          <p role={said.ok ? 'status' : 'alert'} className={cn('px-2 pb-1 text-xs', !said.ok && 'text-destructive')}>
            {said.words}
          </p>
        )}
      </div>
    )
  }
  const rows = rowsOf(views)
  return (
    <div className="flex flex-col">
      <h4 className="mx-2 mt-1.5 mb-1 text-xs font-semibold tracking-wide text-muted-foreground uppercase">
        {`Views of ${boardName}`}
      </h4>
      {row(null, ALL_TASKS, 'every task of the board', 'all')}
      {rows.map((entry, index) => {
        const sub = entry.error == null ? viewSource(entry) : `${viewSource(entry)} · has an error`
        return row(entry.view.name, entry.view.name, sub, String(index), entry)
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
