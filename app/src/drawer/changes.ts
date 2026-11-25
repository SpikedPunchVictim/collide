// What the drawer's Changes part says (ts-0189-fg; 0.11, Task drawer, Changes; note 17 of the board mock), as plain
// functions over what `task.changes` gave. Nothing here reads the owner or the page, so each rule is tested alone.
import { runStateWords } from '../components/words.ts'
import type { TaskChangesResultOutput } from '../owner/generated.ts'

export type Changes = TaskChangesResultOutput
export type RunEntry = Changes['runs'][number]
export type CommitEntry = Changes['commits'][number]
export type ChangedFile = RunEntry['files'][number]
export type Collapsed = RunEntry['collapsed'][number]

/** What a diff is of: one run's changes, or one commit's. */
export type DiffSource = { readonly run: string } | { readonly commit: string }

/** The file a person picked, and where it came from: what the diff window opens. */
export interface DiffTarget {
  readonly source: DiffSource
  /** The words above the path: which run or commit. */
  readonly heading: string
  readonly file: ChangedFile
}

export const NOTHING_YET = 'No changes yet.'
export const LIVE_WORDS = 'Live, as the agent edits'

export const isEmpty = (changes: Changes): boolean => changes.runs.length === 0 && changes.commits.length === 0

/** A run whose agent is working, so its files are still changing. */
export const isLive = (run: RunEntry): boolean => run.state.state === 'running'

export const hasLiveRun = (changes: Changes): boolean => changes.runs.some(isLive)

/** "Run r-12 · Working": the run and its state in words. */
export const runHeading = (run: RunEntry): string => `Run ${run.run} · ${runStateWords(run.state)}`

/** Where a list of changes comes from and how far it reaches, as the daemon says them, in one sentence. */
export function whereWords(words: { readonly place: string; readonly how: string }): string {
  const line = `${words.place}, ${words.how}`
  return line.endsWith('.') ? line : `${line}.`
}

const plural = (n: number, one: string, many: string) => (n === 1 ? `${n} ${one}` : `${n} ${many}`)

/** "3 files, +42 −4": the files of a list and the lines they add and remove. A binary file has no counts. */
export function filesWords(files: readonly ChangedFile[], more: number): string {
  const total = files.length + more
  if (total === 0) return 'No files'
  const added = files.reduce((sum, file) => sum + (file.additions ?? 0), 0)
  const removed = files.reduce((sum, file) => sum + (file.deletions ?? 0), 0)
  const counts = `${plural(total, 'file', 'files')}, +${added} −${removed}`
  return more === 0 ? counts : `${counts} in the ${files.length} listed`
}

/** A size in the largest unit that keeps it under 1024: `999 B`, `2.0 KB`, `4.3 MB`. */
export function sizeWords(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`
}

/** A commit by its first seven characters, as git shortens it. */
export const shortCommit = (id: string): string => id.slice(0, 7)
