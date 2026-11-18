import { createHash } from 'node:crypto'
import { readdir, readFile } from 'node:fs/promises'
import { join, relative, sep } from 'node:path'
import { inBatches } from './files.ts'

/**
 * The sha256 of every regular file under the checkout's `arch/`, by its path from the root with `/` separators. A link
 * is not followed. A checkout with no `arch/` has no files.
 */
export async function hashArch(root: string): Promise<Map<string, string>> {
  let entries
  try {
    entries = await readdir(join(root, 'arch'), {
      recursive: true,
      withFileTypes: true,
    })
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') return new Map()
    throw error
  }
  const paths = entries.filter((entry) => entry.isFile()).map((entry) => join(entry.parentPath, entry.name))
  const hashes = await inBatches(paths, async (path) => {
    const hash = createHash('sha256')
      .update(await readFile(path))
      .digest('hex')
    return [relative(root, path).split(sep).join('/'), hash] as const
  })
  return new Map(hashes)
}

/** The paths whose content differs between two hashes: changed, added or removed. Sorted. */
export function changedPaths(before: Map<string, string>, after: Map<string, string>): string[] {
  const paths = new Set([...before.keys(), ...after.keys()])
  return [...paths].filter((path) => before.get(path) !== after.get(path)).sort()
}
