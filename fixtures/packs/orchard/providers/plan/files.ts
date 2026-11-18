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
