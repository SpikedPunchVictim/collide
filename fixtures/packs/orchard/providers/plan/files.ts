import { readdir } from 'node:fs/promises'

async function entriesIn(dir: string) {
  try {
    return await readdir(dir, { withFileTypes: true })
  } catch (error) {
    // A checkout with no such folder has none of what it would hold.
    const code = (error as NodeJS.ErrnoException).code
    if (code === 'ENOENT' || code === 'ENOTDIR') return []
    throw error
  }
}

/** The names of one folder's regular files, sorted. A link, a folder or a missing folder gives none. */
export async function filesIn(dir: string): Promise<string[]> {
  return (await entriesIn(dir)).filter((entry) => entry.isFile()).map((entry) => entry.name).sort()
}

/** The names of one folder's folders, sorted. A link, a file or a missing folder gives none. */
export async function foldersIn(dir: string): Promise<string[]> {
  return (await entriesIn(dir)).filter((entry) => entry.isDirectory()).map((entry) => entry.name).sort()
}

/** How many files are open at once. */
const BATCH = 32

/** Runs `work` over `items` a batch at a time, and returns the results in order. */
export async function inBatches<T, R>(items: T[], work: (item: T) => Promise<R>): Promise<R[]> {
  const results: R[] = []
  for (let at = 0; at < items.length; at += BATCH) {
    results.push(...(await Promise.all(items.slice(at, at + BATCH).map(work))))
  }
  return results
}
