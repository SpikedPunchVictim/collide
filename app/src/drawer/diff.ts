// A file's diff as lines for the diff window (ts-0189-fg; 0.11, Task drawer; note 29 of the board mock). A patch from
// `task.diff` is git's text; the whole file is lines the daemon already numbered and marked. Both become `DiffLine`s,
// so the window draws one kind of row, and the first change is found in one place.
import type { TaskDiffResultOutput } from '../owner/generated.ts'

/** `meta` is git's header and notes, `hunk` a `@@` line, and the rest are lines of the file. */
export type LineKind = 'meta' | 'hunk' | 'add' | 'del' | 'ctx'

export interface DiffLine {
  readonly kind: LineKind
  /** The line's number in the file after the changes. A removed line is not in that file, so it has none. */
  readonly n: number | null
  /** The line as the file has it, less the mark git put before it. */
  readonly text: string
}

export type WholeLines = NonNullable<TaskDiffResultOutput['whole']>['lines']

const HUNK = /^@@ -\d+(?:,\d+)? \+(\d+)(?:,\d+)? @@/

/**
 * A patch as lines. Before a file's first hunk every line is git's header, even one that begins with `+++` or `---`;
 * `diff --git` starts a new file, whose header is read the same way.
 */
export function patchLines(patch: string): DiffLine[] {
  const rows = patch.split('\n')
  if (rows[rows.length - 1] === '') rows.pop()
  const lines: DiffLine[] = []
  let inHunk = false
  let next = 0
  for (const row of rows) {
    if (row.startsWith('diff --git ')) inHunk = false
    const start = HUNK.exec(row)
    if (start !== null) {
      inHunk = true
      next = Number(start[1])
      lines.push({ kind: 'hunk', n: null, text: row })
    } else if (!inHunk || row.startsWith('\\')) {
      lines.push({ kind: 'meta', n: null, text: row })
    } else if (row.startsWith('+')) {
      lines.push({ kind: 'add', n: next++, text: row.slice(1) })
    } else if (row.startsWith('-')) {
      lines.push({ kind: 'del', n: null, text: row.slice(1) })
    } else {
      lines.push({ kind: 'ctx', n: next++, text: row.slice(1) })
    }
  }
  return lines
}

/** The whole file after the changes: a changed line is marked as added. The file has no line for what was removed. */
export function wholeLines(lines: WholeLines): DiffLine[] {
  return lines.map((line) => ({ kind: line.changed ? 'add' : 'ctx', n: line.n, text: line.text }))
}

/** The index of the first line the changes touched, or -1 when they touched none. */
export function firstChange(lines: readonly DiffLine[]): number {
  return lines.findIndex((line) => line.kind === 'add' || line.kind === 'del')
}
