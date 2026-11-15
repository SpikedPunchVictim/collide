import { randomBytes } from 'node:crypto'
import { link, open, readdir, rename, unlink } from 'node:fs/promises'
import { basename, dirname, join } from 'node:path'
import { setTimeout as sleep } from 'node:timers/promises'

/** The lock file's name, in the shared root (`<root>/.collide/planning`). The scan never reads it, and `.gitignore` lists it. */
export const LOCK_FILE = '.write-lock'

/** A waiter gives up after this long (plan 0.10, Provider writes are safe against each other). */
export const LOCK_TIMEOUT_MS = 15_000

const POLL_MS = 10

export interface LockOptions {
  /** How long to wait for the lock. */
  timeoutMs?: number
  /** Called once for each stale lock this process removes (the tests count these). */
  onTakeover?: (stalePid: number) => void
}

/**
 * What a lock file says: the pid in it, or null when its content is not `<digits>\n`. A lock file is only ever made
 * whole (see `placeWithPid`), so a null pid means someone made or changed the file by hand, and the lock is never
 * taken over: its owner is unknown, so it is for a person to look at.
 */
interface Holder {
  pid: number | null
}

const isMissing = (error: unknown): boolean => (error as NodeJS.ErrnoException).code === 'ENOENT'

/** A process is alive unless the system says there is no such process (`EPERM` is a live process that is not ours). */
function isAlive(pid: number): boolean {
  try {
    process.kill(pid, 0)
    return true
  } catch (error) {
    return (error as NodeJS.ErrnoException).code !== 'ESRCH'
  }
}

/** Reads a lock file, or null when it is not there. */
async function readHolder(path: string): Promise<Holder | null> {
  let text: string
  try {
    const handle = await open(path, 'r')
    try {
      text = await handle.readFile('utf8')
    } finally {
      await handle.close()
    }
  } catch (error) {
    if (isMissing(error)) return null
    throw error
  }
  const match = /^([0-9]+)\n$/.exec(text)
  return { pid: match === null ? null : Number(match[1]) }
}

/** A lock is stale when its writer is gone: its pid is dead. A lock that holds no pid is not stale, it is unreadable. */
const isStale = (holder: Holder): holder is { pid: number } => holder.pid !== null && !isAlive(holder.pid)

/**
 * Places `path` holding this process's pid, failing when it exists. The pid goes into a temp file beside it (a name
 * that starts with `<name>.`, so `.gitignore` covers it), which is synced and then hard-linked to `path`: `link` is
 * atomic and fails with `EEXIST` when `path` exists, so this is the exclusive create, and the lock file never
 * exists without its whole pid, however long this process is paused. The temp file is always removed. Returns
 * false when `path` exists.
 */
async function placeWithPid(path: string): Promise<boolean> {
  const temp = `${path}.${process.pid}.${randomBytes(4).toString('hex')}.tmp`
  const handle = await open(temp, 'wx')
  try {
    try {
      await handle.writeFile(`${process.pid}\n`)
      await handle.sync()
    } finally {
      await handle.close()
    }
    await link(temp, path)
    return true
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'EEXIST') return false
    throw error
  } finally {
    await unlink(temp)
  }
}

/** `<name>.<pid>.<8 hex>.tmp` and `<name>.takeover.<pid>.<8 hex>.tmp`: the temp files of `placeWithPid`. */
const tempPid = (name: string, file: string): number | null => {
  const match = new RegExp(`^${name.replaceAll('.', '\\.')}\\.(?:takeover\\.)?([0-9]+)\\.[0-9a-f]{8}\\.tmp$`).exec(file)
  return match === null ? null : Number(match[1])
}

/**
 * Removes the temp files that killed processes left beside the lock. Only the holder calls this, and only a file
 * whose embedded pid is dead is removed: a live process may be between writing its temp file and linking it.
 */
async function removeLeftoverTemps(path: string): Promise<void> {
  for (const file of await readdir(dirname(path))) {
    const pid = tempPid(basename(path), file)
    if (pid === null || isAlive(pid)) continue
    try {
      await unlink(join(dirname(path), file))
    } catch (error) {
      if (!isMissing(error)) throw error
    }
  }
}

/**
 * Removes a stale lock, at most one remover at a time.
 *
 * Deleting a lock that looked stale is where two waiters go wrong: A and B both read the dead pid, A deletes the
 * lock and creates its own, and B, which read before that, then deletes A's live lock. So the delete is done under
 * a claim file, `<lock>.takeover`, placed the same way as the lock (`placeWithPid`). The holder of the claim
 * reads the lock again, and deletes it only when it is still stale, so a lock that a faster waiter just created is
 * left alone. A waiter that does not get the claim goes back to waiting.
 *
 * A claim is held for the length of one read and one delete. Its writer can still be killed inside that time, so a
 * claim whose pid is dead is removed too, by a rename to a name no other waiter uses and then a delete. That step
 * has the same shape of race, but it needs a writer killed inside that one read and one delete, and two waiters then
 * clearing the dead claim at the same moment; the lock file itself is still only ever placed by an exclusive link.
 */
async function removeStale(path: string, onTakeover: (stalePid: number) => void): Promise<void> {
  const claim = `${path}.takeover`
  if (!(await placeWithPid(claim))) {
    const claimant = await readHolder(claim)
    if (claimant !== null && isStale(claimant)) {
      const aside = `${claim}.${process.pid}.${randomBytes(4).toString('hex')}.tmp`
      try {
        await rename(claim, aside)
        await unlink(aside)
      } catch (error) {
        if (!isMissing(error)) throw error
      }
    }
    return
  }
  try {
    const holder = await readHolder(path)
    if (holder !== null && isStale(holder)) {
      await unlink(path)
      onTakeover(holder.pid)
    }
  } finally {
    await unlink(claim)
  }
}

/** Who the timeout message says has the lock; a holder that left between the checks is reported as gone. */
const describeHolder = (holder: Holder | null): string =>
  holder === null ? 'which has just been released' : holder.pid === null ? 'which holds no pid, so no writer can be asked to leave: remove it by hand once no writer is running' : `held by pid ${holder.pid}`

/**
 * Runs `work` holding the lock file at `path`. The file is placed only when absent (an exclusive hard link, see
 * `placeWithPid`), holds the writer's pid, and is removed when `work` ends. A waiter retries until the timeout, and
 * takes over a lock whose writer is dead (see `removeStale`), but never one that holds no pid. Node has no `flock`
 * and no native add-on is allowed, so this is a lock file.
 */
export async function withLock<T>(path: string, work: () => Promise<T>, options: LockOptions = {}): Promise<T> {
  const { timeoutMs = LOCK_TIMEOUT_MS, onTakeover = () => {} } = options
  const deadline = Date.now() + timeoutMs
  for (;;) {
    if (await placeWithPid(path)) break
    const holder = await readHolder(path)
    if (holder !== null && isStale(holder)) {
      await removeStale(path, onTakeover)
      await sleep(1)
      continue
    }
    if (Date.now() >= deadline) throw new Error(`timed out after ${timeoutMs / 1000} s waiting for the lock file ${path}, ${describeHolder(holder)}`)
    await sleep(POLL_MS + Math.floor(Math.random() * POLL_MS))
  }
  try {
    await removeLeftoverTemps(path)
    return await work()
  } finally {
    await release(path)
  }
}

/** Removes the lock file. It must still hold this process's pid: a live writer's lock is never taken over, so anything else is a bug worth failing on. */
async function release(path: string): Promise<void> {
  const holder = await readHolder(path)
  if (holder?.pid !== process.pid) throw new Error(`the lock file ${path} was not ours at release: it holds ${describeHolder(holder)}`)
  await unlink(path)
}
