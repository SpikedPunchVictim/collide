import { randomBytes } from 'node:crypto'
import { open, readdir, rename, unlink } from 'node:fs/promises'
import { basename, dirname, join } from 'node:path'

/** The ending of a temp file's name. The scan reads only `.md` files, so it never opens one, and `.gitignore` lists it. */
export const TEMP_SUFFIX = '.collide-tmp'

/**
 * Test hook: when the environment variable `COLLIDE_FAILPOINT` equals the name of a failpoint, the process kills
 * itself with SIGKILL there. `before-rename` fires after the temp file is written and synced and before it is
 * renamed over the item, so a test can check that a killed writer leaves the old file whole. `between-files` fires
 * in a write of several files, after one is replaced and before the next, so a test can check that every file is
 * whole after a kill. With the variable unset (every real run) nothing happens.
 */
export function failpoint(name: 'before-rename' | 'between-files'): void {
  if (process.env.COLLIDE_FAILPOINT !== name) return
  process.kill(process.pid, 'SIGKILL')
  // The signal can arrive a moment after `kill` returns; do not go on in the meantime.
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 10_000)
}

/** Syncs a directory so that a rename in it survives a crash. Windows cannot open a directory, so it is skipped there. */
export async function syncDirectory(path: string): Promise<void> {
  if (process.platform === 'win32') return
  const handle = await open(path, 'r')
  try {
    await handle.sync()
  } finally {
    await handle.close()
  }
}

/**
 * Replaces `path` with `content` so that a reader sees the old file or the new one and never half of either: the
 * content goes to a temp file beside it (same directory, so the rename stays on one file system), is synced, renamed
 * over the item, and the directory is synced. The file keeps its permission bits.
 */
export async function replaceFile(path: string, content: Buffer, mode: number): Promise<void> {
  // Every temp file is made under the lock, which this writer holds, so one still here is what a killed writer left.
  const mine = `.${basename(path)}.`
  for (const name of await readdir(dirname(path))) {
    if (name.startsWith(mine) && name.endsWith(TEMP_SUFFIX)) await unlink(join(dirname(path), name))
  }
  const temp = join(dirname(path), `.${basename(path)}.${process.pid}-${randomBytes(4).toString('hex')}${TEMP_SUFFIX}`)
  const handle = await open(temp, 'wx', mode)
  try {
    try {
      await handle.chmod(mode)
      await handle.writeFile(content)
      await handle.sync()
    } finally {
      await handle.close()
    }
    failpoint('before-rename')
    await rename(temp, path)
  } catch (error) {
    // Best effort: the write failed and that error is the one to report, so a failed cleanup must not replace it.
    await unlink(temp).catch(() => {})
    throw error
  }
  await syncDirectory(dirname(path))
}
