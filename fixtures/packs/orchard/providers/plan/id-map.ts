import { readFile, stat } from 'node:fs/promises'
import { join } from 'node:path'
import { taskRef } from './ids.ts'

/** The id map is read up to this size. The Orchard one is about 96 KB. A bigger file is not read. */
const MAX_BYTES = 4 * 1024 * 1024

/** One row of the id map: a task that was renumbered, as short ids. */
export interface IdMapRow {
  from: string
  to: string
}

/**
 * The renumbers that the checkout's `arch/ID-MAP.md` records. A row is a table line whose first two cells name task
 * ids, each bare or with its slug: `| old id | new id | day | detail |`. Any other line is skipped, and a file that is
 * missing or too big has no rows.
 */
export async function readIdMap(root: string, log: (message: string) => void): Promise<IdMapRow[]> {
  const path = join(root, 'arch', 'ID-MAP.md')
  let text: string
  try {
    if ((await stat(path)).size > MAX_BYTES) {
      log(`${path} is over ${MAX_BYTES} bytes, so it is not read`)
      return []
    }
    text = await readFile(path, 'utf8')
  } catch (error) {
    const code = (error as NodeJS.ErrnoException).code
    if (code !== 'ENOENT' && code !== 'ENOTDIR') log(`${path} could not be read: ${(error as Error).message}`)
    return []
  }
  const rows: IdMapRow[] = []
  for (const line of text.split('\n')) {
    if (!line.startsWith('|')) continue
    const [from, to] = line
      .split('|')
      .slice(1)
      .map((cell) => taskRef(cell.trim()))
    if (from !== null && from !== undefined && to !== null && to !== undefined) rows.push({ from, to })
  }
  return rows
}

/**
 * The id that `shortId` became, following a chain of renumbers to its end, or null when no row moves it. A loop in the
 * rows ends where it began.
 */
export function renamedTo(rows: IdMapRow[], shortId: string): string | null {
  let current = shortId
  const seen = new Set([shortId])
  for (;;) {
    const row = rows.findLast((candidate) => candidate.from === current)
    if (row === undefined || seen.has(row.to)) break
    current = row.to
    seen.add(current)
  }
  return current === shortId ? null : current
}
