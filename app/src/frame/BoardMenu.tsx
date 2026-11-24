// The board menu of the top bar (note 1; 0.11, Line selector): every board of `board.list` with its default line, and
// a typed folder to open, as `loop/Repository.tsx` has it. Choosing a board only changes which board the window reads;
// it stops nothing. The folder picker, and the row for a board whose folder is gone, are `ts-0191-nv`'s.
import { type FormEvent, useEffect, useState } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import type { BoardSummary } from '../loop/types.ts'
import { boardName, defaultLine } from './boards.ts'

export interface BoardMenuProps {
  readonly boards: readonly BoardSummary[]
  /** The board the window reads. */
  readonly boardId: string | null
  /** Why the last folder was not opened. */
  readonly problems: readonly string[]
  readonly onChoose: (id: string) => void
  readonly onOpenFolder: (path: string) => Promise<void>
}

export function BoardMenu(props: BoardMenuProps) {
  const [open, setOpen] = useState(false)
  const current = props.boards.find((board) => board.id === props.boardId) ?? null
  const line = current === null ? null : defaultLine(current)
  // A folder that opens as a board makes it the board in use, and the menu has done its work.
  useEffect(() => setOpen(false), [props.boardId])
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button variant="ghost" size="sm" title="Board and code line">
          {current === null ? 'Boards' : boardName(current)}
          {line === null ? null : <small className="font-normal text-muted-foreground">{`· ${line}`}</small>}
          <ChevronDown />
        </Button>
      </PopoverTrigger>
      <PopoverContent aria-label="Open boards">
        <BoardMenuBody
          {...props}
          onChoose={(id) => {
            props.onChoose(id)
            setOpen(false)
          }}
        />
      </PopoverContent>
    </Popover>
  )
}

/** What the menu holds, apart from the popover that shows it, so a test renders it for any boards. */
export function BoardMenuBody({ boards, boardId, problems, onChoose, onOpenFolder }: BoardMenuProps) {
  const [path, setPath] = useState('')
  const submit = (event: FormEvent) => {
    event.preventDefault()
    // The typed path stays when the folder is refused, so a typo is fixed in place. A folder that opens closes the
    // menu.
    void onOpenFolder(path)
  }
  return (
    <div className="flex flex-col">
      <h4 className="mx-2 mt-1.5 mb-1 text-xs font-semibold tracking-wide text-muted-foreground uppercase">
        Open boards
      </h4>
      {boards.map((board) => {
        const line = defaultLine(board)
        return (
          <button
            key={board.id}
            type="button"
            aria-current={board.id === boardId}
            onClick={() => onChoose(board.id)}
            className={
              'flex w-full items-center rounded-md px-2 py-1.5 text-left hover:bg-muted ' +
              'aria-[current=true]:bg-muted'
            }
          >
            <span className="min-w-0 flex-1">
              <span className="block truncate font-medium">{boardName(board)}</span>
              <span className="block truncate text-xs text-muted-foreground">
                {line === null ? null : <>line <b>{line}</b> · </>}
                {board.root}
              </span>
            </span>
          </button>
        )
      })}
      <hr className="my-1.5 border-t" />
      <form onSubmit={submit} className="flex flex-col gap-1.5 px-2 py-1">
        <p className="font-medium">Open a folder…</p>
        <p className="text-xs text-muted-foreground">Any git repo opens as a board, with no setup</p>
        <div className="flex gap-2">
          <Input
            aria-label="Folder path"
            placeholder="/path/to/a/git/checkout"
            value={path}
            onChange={(event) => setPath(event.target.value)}
          />
          <Button type="submit" variant="outline" disabled={path.trim() === ''}>
            Open folder
          </Button>
        </div>
        {problems.map((problem) => (
          <p key={problem} role="alert" className="text-sm text-destructive">
            {problem}
          </p>
        ))}
      </form>
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
