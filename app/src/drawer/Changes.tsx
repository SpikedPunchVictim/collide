// The drawer's Changes part, drawn from what `task.changes` gave (ts-0189-fg; 0.11, Task drawer, Changes; note 17 of
// the board mock): each run with where it ran and its files, then the commits made by hand. A file is a button that
// opens its diff (`DiffWindow`). It reads nothing: `changes` is what `useChanges` holds, so a test draws it from a
// scripted read. A run's entry only names the run; the link to its place in Tracks waits for Stage 9b.
import { Badge } from '@/components/ui/badge'
import { cn } from '@/lib/utils'
import { DOT, TEXT } from '../field/colours.ts'
import { fileStatusWords } from '../loop/words.ts'
import {
  type ChangedFile,
  type Changes,
  type Collapsed,
  type DiffTarget,
  LIVE_WORDS,
  NOTHING_YET,
  type RunEntry,
  filesWords,
  isEmpty,
  isLive,
  runHeading,
  shortCommit,
  sizeWords,
  whereWords,
} from './changes.ts'
import type { ChangesRead } from './changes-feed.ts'

export interface ChangesPartProps {
  /** What the feed holds, or null before it has been asked. */
  readonly changes: ChangesRead | null
  readonly onOpenFile: (target: DiffTarget) => void
}

export function ChangesPart({ changes, onOpenFile }: ChangesPartProps) {
  if (changes === null || (changes.changes === null && changes.problem === null)) {
    return (
      <p role="status" className="text-muted-foreground">
        Reading the changes…
      </p>
    )
  }
  return (
    <div className="flex flex-col gap-3">
      {changes.problem === null ? null : (
        <p role="alert" className="text-destructive">{`The changes could not be read: ${changes.problem}`}</p>
      )}
      {changes.changes === null ? null : <Listed changes={changes.changes} onOpenFile={onOpenFile} />}
    </div>
  )
}

function Listed({ changes, onOpenFile }: { changes: Changes; onOpenFile: (target: DiffTarget) => void }) {
  if (isEmpty(changes)) return <p className="text-muted-foreground">{NOTHING_YET}</p>
  return (
    <>
      {changes.runs.map((run) => (
        <Run key={run.run} run={run} onOpenFile={onOpenFile} />
      ))}
      {changes.commits.length === 0 ? null : (
        <div className="flex flex-col gap-2">
          <p className="text-xs text-muted-foreground">{whereWords(changes.commits_words)}</p>
          {changes.commits.map((commit) => (
            <div
              key={commit.id}
              data-commit={commit.id}
              className="flex flex-col gap-1 rounded-md border px-2.5 py-2"
            >
              <div>
                <span className="font-mono text-xs text-muted-foreground">{shortCommit(commit.id)}</span>{' '}
                <span>{commit.subject}</span>
              </div>
              <Files
                files={commit.files}
                collapsed={commit.collapsed}
                more={commit.more}
                onOpen={(file) =>
                  onOpenFile({
                    source: { commit: commit.id },
                    heading: `Commit ${shortCommit(commit.id)}: ${commit.subject}`,
                    file,
                  })
                }
              />
            </div>
          ))}
        </div>
      )}
    </>
  )
}

function Run({ run, onOpenFile }: { run: RunEntry; onOpenFile: (target: DiffTarget) => void }) {
  const live = isLive(run)
  return (
    <div data-run={run.run} className="flex flex-col gap-1 rounded-md border px-2.5 py-2">
      <div className="flex flex-wrap items-center gap-2">
        <b className="font-semibold">{runHeading(run)}</b>
        {live ? (
          <Badge variant="outline" className={cn('gap-1.5 font-normal', TEXT.active)}>
            <span className={cn('size-1.5 rounded-full motion-safe:animate-pulse', DOT.active)} />
            {LIVE_WORDS}
          </Badge>
        ) : null}
      </div>
      <p className="text-xs text-muted-foreground">{whereWords(run.words)}</p>
      {run.note === undefined || run.note === null ? null : <p className="text-muted-foreground">{run.note}</p>}
      <Files
        files={run.files}
        collapsed={run.collapsed}
        more={run.more}
        onOpen={(file) => onOpenFile({ source: { run: run.run }, heading: `Run ${run.run}`, file })}
      />
    </div>
  )
}

interface FilesProps {
  readonly files: readonly ChangedFile[]
  readonly collapsed: readonly Collapsed[]
  readonly more: number
  readonly onOpen: (file: ChangedFile) => void
}

function Files({ files, collapsed, more, onOpen }: FilesProps) {
  if (files.length === 0 && collapsed.length === 0 && more === 0) return null
  return (
    <div className="flex flex-col gap-0.5">
      <p className="text-xs text-muted-foreground">{filesWords(files, more)}</p>
      {files.map((file) => (
        <button
          key={file.path}
          type="button"
          data-file={file.path}
          title="Open the diff"
          onClick={() => onOpen(file)}
          className={cn(
            'flex w-full cursor-pointer items-baseline gap-2 rounded px-1.5 py-0.5 text-left',
            'hover:bg-muted focus-visible:outline-2 focus-visible:outline-ring',
          )}
        >
          <span className="min-w-0 flex-1 font-mono text-xs break-all">{file.path}</span>
          <Counts file={file} />
          {file.status === 'modified' ? null : (
            <span className="text-xs text-muted-foreground">{fileStatusWords(file.status)}</span>
          )}
        </button>
      ))}
      {collapsed.map((folder) => (
        <p key={folder.path} className="px-1.5 text-xs text-muted-foreground">
          {`${folder.path}: ${folder.files} files, ${sizeWords(folder.bytes)}, not listed`}
        </p>
      ))}
      {more === 0 ? null : (
        <p className="px-1.5 text-xs text-muted-foreground">
          {more === 1 ? '1 more file is not listed.' : `${more} more files are not listed.`}
        </p>
      )}
    </div>
  )
}

/** A file's added and removed lines. A binary file has none, and says so. */
function Counts({ file }: { file: ChangedFile }) {
  if (file.additions == null || file.deletions == null) {
    return <span className="text-xs text-muted-foreground">binary file</span>
  }
  return (
    <span className="flex gap-1.5 font-mono text-xs">
      <span className={TEXT.done}>{`+${file.additions}`}</span>
      <span className="text-destructive">{`−${file.deletions}`}</span>
    </span>
  )
}
