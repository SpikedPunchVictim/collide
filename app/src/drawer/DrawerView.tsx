// The task drawer, drawn from what was read (ts-0184-hv; 0.11, Task drawer; notes 6, 7, 8 and 18 of the board mock):
// the header with the status, the category in words, the id and Close, then the title and the read-only line, then the
// parts in order, each of which folds. It reads nothing and keeps nothing, so a test draws it from a scripted read.
// Everything it shows from a task is text: the description goes through `Markdown`, and the rest is React text.
import type { ReactNode } from 'react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { DOT, TEXT } from '../field/colours.ts'
import { tailOf } from '../field/model.ts'
import type { FieldData, Task } from '../field/types.ts'
import { Markdown } from '../markdown/Markdown.tsx'
import {
  type CopyRow,
  type DrawerInput,
  type PartId,
  blockingLinks,
  categoryWords,
  copyRows,
  detailsOf,
  fieldRows,
  linkWords,
  partOf,
  partsOf,
  readOnlyLine,
} from './model.ts'
import type { BoardNote, Got, GotLink } from './types.ts'

export interface DrawerViewProps {
  /** The task as the board lists it. */
  readonly shown: Task
  /** What `task.get` gave for it, or null before the first read. */
  readonly got: Got | null
  /** The words of a read that failed. */
  readonly problem: string | null
  readonly data: FieldData
  readonly board: BoardNote | null
  /** The parts that are folded. */
  readonly shut: readonly string[]
  readonly onFlip: (part: PartId) => void
  /** A task named in a link opens in the drawer, by its key. */
  readonly onOpenTask: (key: string) => void
  readonly onClose: () => void
}

export function DrawerView(props: DrawerViewProps) {
  const { shown, got, problem, data, shut, onFlip, onClose } = props
  const input: DrawerInput | null = got === null ? null : { shown, got, data, board: props.board }
  const { category, status } = (got?.task ?? shown).state
  return (
    <aside
      aria-label="Task drawer"
      className={cn(
        'flex h-full max-h-[calc(100vh-1.5rem)] w-full min-w-0 flex-col rounded-lg border bg-card shadow-sm',
        'max-[900px]:max-h-none max-[900px]:rounded-none max-[900px]:shadow-xl',
      )}
    >
      <header className="flex shrink-0 flex-col gap-1.5 border-b px-4 py-3">
        <div className="flex items-center gap-2">
          <Badge variant="outline" className={cn('font-normal', TEXT[category])}>
            <span className={cn('size-2 rounded-full', DOT[category])} />
            {status}
          </Badge>
          <span className="text-sm text-muted-foreground">{categoryWords(category)}</span>
          <span className="flex-1" />
          <span className="font-mono text-xs text-muted-foreground">{tailOf(shown.key)}</span>
          <Button variant="ghost" size="sm" onClick={onClose}>
            Close
          </Button>
        </div>
        <h2 className="text-base leading-snug font-semibold break-words">{shown.title}</h2>
        <p className="text-sm text-muted-foreground">{readOnlyLine(got?.task ?? shown, data)}</p>
      </header>
      <div className="min-h-0 flex-1 overflow-y-auto">
        {problem === null ? null : (
          <p role="alert" className="px-4 py-3 text-destructive">{`The task could not be read: ${problem}`}</p>
        )}
        {input === null ? (
          problem === null ? (
            <p role="status" className="px-4 py-3 text-muted-foreground">
              Reading the task…
            </p>
          ) : null
        ) : (
          partsOf(input).map((part) => {
            const folded = shut.includes(part.id)
            return (
              <section key={part.id} data-part={part.id} className="border-b px-4 py-2.5 last:border-b-0">
                <h3 className="text-sm font-semibold">
                  <button
                    type="button"
                    aria-expanded={!folded}
                    onClick={() => onFlip(part.id)}
                    className={cn(
                      'flex w-full cursor-pointer items-center gap-1.5 py-1 text-left',
                      'focus-visible:outline-2 focus-visible:outline-ring',
                    )}
                  >
                    <Chevron folded={folded} />
                    <span>{part.title}</span>
                  </button>
                </h3>
                {folded ? null : (
                  <div className="pt-1 pb-1 break-words">
                    <PartBody id={part.id} input={input} onOpenTask={props.onOpenTask} />
                  </div>
                )}
              </section>
            )
          })
        )}
      </div>
    </aside>
  )
}

function Chevron({ folded }: { folded: boolean }) {
  return (
    <svg
      aria-hidden="true"
      viewBox="0 0 16 16"
      className={cn('size-3.5 shrink-0 text-muted-foreground transition-transform', folded && '-rotate-90')}
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
    >
      <path d="M4 6l4 4 4-4" />
    </svg>
  )
}

interface PartBodyProps {
  readonly id: PartId
  readonly input: DrawerInput
  readonly onOpenTask: (key: string) => void
}

function PartBody({ id, input, onOpenTask }: PartBodyProps) {
  const { got, data } = input
  const task = got.task ?? input.shown
  const source = data.sources.find((each) => each.name === task.key.slice(0, task.key.indexOf(':')))
  switch (id) {
    case 'description':
      return <Markdown text={got.task?.body ?? ''} />
    case 'waits':
      return (
        <LinkRows
          links={blockingLinks(got, source)?.waits ?? []}
          empty="Nothing. It can start."
          onOpenTask={onOpenTask}
        />
      )
    case 'unblocks':
      return (
        <LinkRows
          links={blockingLinks(got, source)?.unblocks ?? []}
          empty="Nothing waits on it."
          onOpenTask={onOpenTask}
        />
      )
    case 'part-of':
      return <PartOfChips input={input} />
    case 'fields':
      return source === undefined ? null : <Fields task={task} source={source} />
    case 'copies':
      return <Copies rows={copyRows(input)} />
    case 'details':
      return <Details input={input} />
  }
}

function LinkRows({
  links,
  empty,
  onOpenTask,
}: {
  links: readonly GotLink[]
  empty: string
  onOpenTask: (key: string) => void
}) {
  if (links.length === 0) return <span className="text-muted-foreground">{empty}</span>
  return (
    <div className="flex flex-col gap-1.5">
      {links.map((link) =>
        link.missing ? (
          <div
            key={`${link.type}|${link.dir}|${link.key}`}
            className="flex items-center gap-2 rounded-md border px-2.5 py-1.5"
          >
            <span className="size-2 shrink-0 rounded-full bg-muted-foreground/40" />
            <span className="min-w-0 flex-1">{link.key}</span>
            <span className="text-xs text-muted-foreground">not a task</span>
          </div>
        ) : (
          <button
            key={`${link.type}|${link.dir}|${link.key}`}
            type="button"
            data-open={link.key}
            onClick={() => onOpenTask(link.key)}
            className={cn(
              'flex w-full cursor-pointer items-center gap-2 rounded-md border px-2.5 py-1.5 text-left',
              'hover:border-input hover:bg-muted focus-visible:outline-2 focus-visible:outline-ring',
            )}
          >
            <span className={cn('size-2 shrink-0 rounded-full', DOT[link.category])} />
            <span className="min-w-0 flex-1">{linkWords(link)}</span>
            <span className="text-xs text-muted-foreground">{`${tailOf(link.key)} · ${link.status}`}</span>
          </button>
        ),
      )}
    </div>
  )
}

function PartOfChips({ input }: { input: DrawerInput }) {
  const { epic, collections, views } = partOf(input)
  return (
    <div className="flex flex-wrap gap-1.5">
      {epic === null ? null : <Badge variant="outline">{epic}</Badge>}
      {collections.map((name) => (
        <Badge key={`collection-${name}`} variant="outline" className="font-normal text-muted-foreground">
          {name}
        </Badge>
      ))}
      {views.map((name) => (
        <Badge key={`view-${name}`} variant="outline" className="font-normal text-muted-foreground">
          {`view: ${name}`}
        </Badge>
      ))}
    </div>
  )
}

function Fields({ task, source }: { task: Task; source: FieldData['sources'][number] }) {
  return (
    <>
      <dl className="grid grid-cols-[minmax(0,9rem)_minmax(0,1fr)] gap-x-3 gap-y-1.5">
        {fieldRows(task, source).map((row) => (
          <div key={row.id} className="contents">
            <dt className="text-muted-foreground">{row.label}</dt>
            <dd>{row.value ?? <span className="text-muted-foreground">not set</span>}</dd>
          </div>
        ))}
      </dl>
      {source.pack === undefined || source.pack === null ? null : (
        <p className="pt-2 text-xs text-muted-foreground">{`Declared by ${source.pack}.`}</p>
      )}
    </>
  )
}

const NOTE_WORDS = { shown: 'shown on the board', differs: 'differs from main', same: 'same as main' } as const

function Copies({ rows }: { rows: readonly CopyRow[] }) {
  if (rows.length === 0) return <span className="text-muted-foreground">No copy of it is listed.</span>
  return (
    <table className="w-full text-sm">
      <thead>
        <tr className="text-left text-xs text-muted-foreground">
          <th className="py-1 font-normal">Copy</th>
          <th className="py-1 font-normal">Status</th>
          <th className="py-1 font-normal">
            <span className="sr-only">Compared with the board</span>
          </th>
        </tr>
      </thead>
      <tbody>
        {rows.map((row) => (
          <tr key={row.id} className={cn('border-t align-top', row.note === 'differs' && 'bg-waiting/10')}>
            <td className="py-1.5 pr-2">
              <b className="font-semibold">{row.label}</b>
              <br />
              <span className="text-xs break-all text-muted-foreground">{row.where}</span>
            </td>
            {row.other === null ? (
              <>
                <td className="py-1.5 pr-2">
                  <span className={cn('inline-flex items-center gap-1.5', TEXT[row.category])}>
                    <span className={cn('size-2 rounded-full', DOT[row.category])} />
                    {row.status}
                  </span>
                </td>
                <td className="py-1.5 text-muted-foreground">{row.note === null ? '' : NOTE_WORDS[row.note]}</td>
              </>
            ) : (
              <td colSpan={2} className="py-1.5">
                <Badge variant="outline" className="border-destructive/40 font-normal text-destructive">
                  Same id, another task
                </Badge>
                <div>{row.other.title}</div>
                <div className="font-mono text-xs break-all text-muted-foreground">{row.other.path}</div>
              </td>
            )}
          </tr>
        ))}
      </tbody>
    </table>
  )
}

function Details({ input }: { input: DrawerInput }): ReactNode {
  const { rows, raw } = detailsOf(input)
  return (
    <>
      <dl className="grid grid-cols-[minmax(0,5rem)_minmax(0,1fr)] gap-x-3 gap-y-1.5">
        {rows.map((row) => (
          <div key={row.name} className="contents">
            <dt className="text-muted-foreground">{row.name}</dt>
            <dd className={cn(row.mono === true && 'font-mono text-xs break-all')}>{row.value}</dd>
          </div>
        ))}
      </dl>
      <pre className="mt-2 overflow-x-auto rounded-md bg-muted p-2.5 text-xs">{raw}</pre>
    </>
  )
}
