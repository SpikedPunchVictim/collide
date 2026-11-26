// The "Open a project" page (ts-0194-qp; note 13; the mock's `welcomeHtml`): the folder, the checks of what Collide
// needs, what the board will read, and the way to open it. It shows for a folder opened from the board menu and when
// nothing is open yet. Only a folder that is not a git repository stops the board; `claude` missing or logged out is
// named with its fix and the board still opens. Nothing is written into the folder from here. It is a function of its
// props, so a test draws it for every state.
import { type FormEvent, useId, useState } from 'react'
import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { CopyButton } from './CopyButton.tsx'
import { type Check, type CheckRow, type Mark, canOpen, checkRows, needsGit, parts } from './model.ts'

export interface FirstRunPageProps {
  /** The folder being looked at, or null when none has been chosen yet. */
  readonly folder: string | null
  /** The checks for the folder, or null while they are being read. */
  readonly checks: readonly Check[] | null
  /** Why the folder could not be looked at or opened, in the daemon's words. */
  readonly problem: string | null
  readonly busy?: boolean
  /** Looks at another folder: typed in a browser, or chosen in the shell. */
  readonly onFolder: (path: string) => void
  /** The system's folder picker, in the shell. Without it the person types the folder. */
  readonly pickFolder?: (() => Promise<string | null>) | null
  readonly onOpen: () => void
  readonly onCheckAgain: () => void
  readonly onMakeRepo: () => void
  /** What the clipboard is written with, for a test. */
  readonly write?: (text: string) => Promise<void>
}

export function FirstRunPage(props: FirstRunPageProps) {
  const { folder, checks, problem, busy = false } = props
  const rows = checks === null ? [] : checkRows(checks)
  return (
    <Card aria-label="Open a folder" className="mx-auto w-full max-w-2xl px-6">
      <h1 className="text-center text-xl font-semibold">Open a project</h1>
      <p className="text-center text-muted-foreground">
        Any git repository opens as a board, with the built-in packs. Nothing is written into the folder until you
        choose Share with team.
      </p>
      <FolderRow folder={folder} pickFolder={props.pickFolder ?? null} onFolder={props.onFolder} />
      {problem === null ? null : (
        <p role="alert" className="whitespace-pre-wrap text-destructive">
          {problem}
        </p>
      )}
      {folder === null ? null : (
        <>
          <h2 className="font-semibold">Checks</h2>
          {checks === null ? (
            <p role="status" className="text-muted-foreground">
              Checking the folder…
            </p>
          ) : (
            <ul aria-label="Checks" className="flex flex-col gap-2">
              {rows.map((row) => (
                <Row key={row.id} row={row} onMakeRepo={props.onMakeRepo} write={props.write} />
              ))}
              <Row key="github" row={GITHUB} onMakeRepo={props.onMakeRepo} />
            </ul>
          )}
          <h2 className="font-semibold">Found in this folder</h2>
          <ul aria-label="Found in this folder" className="flex flex-col gap-1">
            <li>
              Scratch tasks <span className="text-muted-foreground">· built in, always on</span>
            </li>
            <li>
              Planning (collide/plan){' '}
              <span className="text-muted-foreground">
                · built in · reads <code>.collide/planning</code> when the folder has it
              </span>
            </li>
          </ul>
          <div className="flex flex-wrap items-center gap-2">
            <Button disabled={busy || checks === null || !canOpen(checks)} onClick={props.onOpen}>
              Open the board
            </Button>
            <Button variant="outline" disabled={busy} onClick={props.onCheckAgain}>
              Check again
            </Button>
            {checks !== null && needsGit(checks) ? (
              <span className="text-sm text-muted-foreground">A folder that is not a git repo cannot open.</span>
            ) : null}
          </div>
        </>
      )}
    </Card>
  )
}

/** The GitHub row is text: a GitHub source arrives in Stage 5, and nothing here needs it. */
const GITHUB: CheckRow = {
  id: 'gh_logged_in',
  mark: 'unknown',
  title: 'GitHub',
  detail: 'Checked when GitHub sources arrive, in Stage 5.',
  copy: null,
}

function FolderRow({
  folder,
  pickFolder,
  onFolder,
}: {
  readonly folder: string | null
  readonly pickFolder: (() => Promise<string | null>) | null
  readonly onFolder: (path: string) => void
}) {
  const inputId = useId()
  const [typed, setTyped] = useState('')
  const [pickProblem, setPickProblem] = useState<string | null>(null)
  const pick = async () => {
    if (pickFolder === null) return
    setPickProblem(null)
    try {
      const chosen = await pickFolder()
      if (chosen !== null) onFolder(chosen)
    } catch (failure) {
      setPickProblem(`Collide could not open the folder picker: ${failure instanceof Error ? failure.message : ''}`)
    }
  }
  const submit = (event: FormEvent) => {
    event.preventDefault()
    if (typed.trim() !== '') onFolder(typed.trim())
  }
  return (
    <div className="flex flex-col gap-2">
      {folder === null ? null : (
        <p className="flex flex-wrap items-baseline gap-2">
          <span className="text-muted-foreground">Folder</span>
          <code className="font-mono [overflow-wrap:anywhere]">{folder}</code>
        </p>
      )}
      {pickFolder === null ? (
        <form onSubmit={submit} className="flex items-center gap-2">
          <label htmlFor={inputId} className="sr-only">
            Folder to look at
          </label>
          <Input
            id={inputId}
            placeholder="/path/to/a/git/checkout"
            value={typed}
            onChange={(event) => setTyped(event.target.value)}
          />
          <Button type="submit" variant="outline" disabled={typed.trim() === ''}>
            {folder === null ? 'Check this folder' : 'Choose another…'}
          </Button>
        </form>
      ) : (
        <div className="flex items-center gap-2">
          <Button variant="outline" onClick={() => void pick()}>
            {folder === null ? 'Choose a folder…' : 'Choose another…'}
          </Button>
        </div>
      )}
      {pickProblem === null ? null : (
        <p role="alert" className="text-sm text-destructive">
          {pickProblem}
        </p>
      )}
    </div>
  )
}

const MARKS: Record<Mark, { readonly glyph: string; readonly label: string; readonly tone: string }> = {
  ok: { glyph: '✓', label: 'Passed', tone: 'text-done' },
  no: { glyph: '✗', label: 'Failed', tone: 'text-destructive' },
  unknown: { glyph: '–', label: 'Not checked', tone: 'text-muted-foreground' },
}

function Row({
  row,
  onMakeRepo,
  write,
}: {
  readonly row: CheckRow
  readonly onMakeRepo: () => void
  readonly write?: ((text: string) => Promise<void>) | undefined
}) {
  const mark = MARKS[row.mark]
  return (
    <li data-check={row.id} data-mark={row.mark} className="flex items-start gap-3">
      <span aria-label={mark.label} role="img" className={`w-4 text-center font-bold ${mark.tone}`}>
        {mark.glyph}
      </span>
      <div className="min-w-0 flex-1">
        <b>{row.title}</b>
        <p className="text-muted-foreground">
          {parts(row.detail).map((part, index) =>
            part.code ? (
              <code key={index} className="font-mono [overflow-wrap:anywhere]">
                {part.text}
              </code>
            ) : (
              part.text
            ),
          )}
        </p>
      </div>
      {row.id === 'git_repo' && row.mark === 'no' ? (
        <Button size="sm" onClick={onMakeRepo}>
          Make it one…
        </Button>
      ) : null}
      {row.copy === null ? null : <CopyButton text={row.copy} {...(write === undefined ? {} : { write })} />}
    </li>
  )
}
