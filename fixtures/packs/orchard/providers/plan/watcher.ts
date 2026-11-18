import { watch as fsWatch } from 'node:fs'
import { join, resolve } from 'node:path'
import type { Notify } from '@collide/pack-sdk'
import { collisionsOf, compare, emptySnapshot, idsChanged } from './changes.ts'
import type { Notification, Snapshot } from './changes.ts'
import { isFolder } from './files.ts'
import { gitDir, gitHead, gitRenames } from './git.ts'
import { readIdMap } from './id-map.ts'
import { scan } from './scan.ts'

/** Changes that come close together are read once, this long after the last of them. */
export const DEBOUNCE_MS = 300

/** Every checkout is read again this often, whatever the watchers saw: a watcher can miss an event. */
export const RECHECK_MS = 10_000

/** The clock. Tests give their own, so that nothing waits for real time. */
export interface Timers {
  setTimeout(run: () => void, ms: number): unknown
  clearTimeout(handle: unknown): void
  setInterval(run: () => void, ms: number): unknown
  clearInterval(handle: unknown): void
}

export interface FsHandle {
  close(): void
}

/** Opens a file watcher. `onEvent` gets the name of the file that changed, or null when the platform does not say. */
export type WatchFs = (
  target: string,
  options: { recursive: boolean },
  onEvent: (name: string | null) => void,
  onError: (error: Error) => void,
) => FsHandle

export interface WatchOptions {
  debounceMs?: number
  recheckMs?: number
  timers?: Timers
  watchFs?: WatchFs
}

const nodeTimers: Timers = {
  setTimeout: (run, ms) => setTimeout(run, ms),
  clearTimeout: (handle) => clearTimeout(handle as NodeJS.Timeout),
  setInterval: (run, ms) => setInterval(run, ms),
  clearInterval: (handle) => clearInterval(handle as NodeJS.Timeout),
}

const nodeWatchFs: WatchFs = (target, options, onEvent, onError) => {
  const watcher = fsWatch(target, { recursive: options.recursive }, (_event, name) =>
    onEvent(name === null ? null : String(name)),
  )
  watcher.on('error', onError)
  return watcher
}

const message = (error: unknown): string => (error instanceof Error ? error.message : String(error))

/** What the watcher holds for one checkout: its last snapshot and the two watchers it opened. */
interface Watched {
  snapshot: Snapshot
  /** Whether the last `stale` was sent and no read has worked since. */
  stale: boolean
  arch: FsHandle | null
  head: FsHandle | null
}

export interface Watcher {
  /** Makes these the checkouts that are watched: the others are dropped, new ones read once and given watchers. */
  set(checkouts: string[]): Promise<void>
  /** Closes every watcher and clock. Nothing is sent after it. */
  stop(): void
}

/**
 * Watches checkouts of one repository and sends what changes in them as notifications.
 * - Each checkout's `arch/` and the folder of its HEAD file are watched. An event, or the recheck, marks the checkout;
 *   a quiet spell of `debounceMs` reads every marked checkout once and compares it with its last snapshot.
 * - `set` reads each new checkout before it returns, so a change made after the reply is a change. It sends no event
 *   for what is already there: core lists at start.
 * - A checkout that cannot be read is `source.stale`, once, until it can be read again. Its last snapshot stays.
 */
export function createWatcher(deps: {
  env: NodeJS.ProcessEnv
  log: (message: string) => void
  notify: Notify
  options?: WatchOptions
}): Watcher {
  const { env, log, notify } = deps
  const options = deps.options ?? {}
  const { debounceMs = DEBOUNCE_MS, recheckMs = RECHECK_MS } = options
  const { timers = nodeTimers, watchFs = nodeWatchFs } = options

  const watched = new Map<string, Watched>()
  const dirty = new Set<string>()
  let debounce: unknown = null
  let recheck: unknown = null
  let queue: Promise<void> = Promise.resolve()
  let collisions = new Map<string, string>()
  let stopped = false

  // The same complaint about the same file, every ten seconds, would fill the log.
  const logged = new Set<string>()
  const logOnce = (line: string): void => {
    if (logged.has(line)) return
    logged.add(line)
    log(line)
  }

  const send = (notifications: Notification[]): void => {
    if (stopped) return
    for (const { method, params } of notifications) (notify as (m: string, p: unknown) => void)(method, params)
  }
  const sendStale = (reason: string, checkout: string): void => {
    if (!stopped) notify('source.stale', { reason, checkout })
  }

  const enqueue = (job: () => Promise<void>): void => {
    queue = queue.then(job).catch((error) => log(`watch: ${message(error)}`))
  }

  const markDirty = (checkout: string): void => {
    if (stopped || !watched.has(checkout)) return
    dirty.add(checkout)
    if (debounce !== null) timers.clearTimeout(debounce)
    debounce = timers.setTimeout(() => {
      debounce = null
      enqueue(flush)
    }, debounceMs)
  }

  const closeHandles = (entry: Watched): void => {
    entry.arch?.close()
    entry.head?.close()
    entry.arch = null
    entry.head = null
  }

  /** Opens the watchers the checkout lacks. `arch/` may not exist yet, and a repository may not be there at all. */
  const openHandles = async (checkout: string, entry: Watched): Promise<void> => {
    const open = (target: string, recursive: boolean, accept: (name: string | null) => boolean, drop: () => void) => {
      try {
        const handle = watchFs(
          target,
          { recursive },
          (name) => {
            if (accept(name)) markDirty(checkout)
          },
          (error) => {
            logOnce(`watching ${target}: ${error.message}`)
            drop()
          },
        )
        return handle
      } catch (error) {
        logOnce(`cannot watch ${target}: ${message(error)}`)
        return null
      }
    }
    const archDir = join(checkout, 'arch')
    if (entry.arch === null && (await isFolder(archDir))) {
      entry.arch = open(archDir, true, () => true, () => {
        entry.arch?.close()
        entry.arch = null
      })
    }
    if (entry.head === null) {
      const dir = await gitDir(checkout, env)
      // Git replaces HEAD by renaming a lock file over it, so the folder is watched and the name is checked.
      if (dir !== null) {
        entry.head = open(dir, false, (name) => name === null || name === 'HEAD', () => {
          entry.head?.close()
          entry.head = null
        })
      }
    }
    // The checkout may have been dropped, or the provider stopped, while git answered.
    if (stopped || watched.get(checkout) !== entry) closeHandles(entry)
  }

  /** Reads a checkout. HEAD is read on both sides of the files, so a snapshot never mixes two commits for long. */
  const takeSnapshot = async (checkout: string): Promise<Snapshot> => {
    if (!(await isFolder(checkout))) throw new Error('it is not a folder')
    for (let attempt = 0; ; attempt++) {
      const head = await gitHead(checkout, env)
      const found = await scan(checkout, logOnce)
      const idMap = await readIdMap(checkout, logOnce)
      if (attempt >= 2 || (await gitHead(checkout, env)) === head) {
        const tasks = new Map(found.tasks.map(({ task }) => [task.key.slice('plan:'.length), task]))
        const invalid = new Map(found.invalid.map((item) => [item.path, item.reason]))
        return { head, tasks, invalid, idMap }
      }
    }
  }

  const refresh = async (checkout: string): Promise<void> => {
    const entry = watched.get(checkout)
    if (entry === undefined) return
    let next: Snapshot
    try {
      next = await takeSnapshot(checkout)
    } catch (error) {
      if (!entry.stale && !stopped && watched.get(checkout) === entry) {
        entry.stale = true
        closeHandles(entry)
        sendStale(`the checkout ${checkout} could not be read: ${message(error)}`, checkout)
      }
      return
    }
    if (stopped || watched.get(checkout) !== entry) return
    entry.stale = false

    const before = entry.snapshot
    const { gone, fresh } = idsChanged(before, next)
    const headMoved = before.head !== null && next.head !== null && before.head !== next.head
    // Git is asked only when a task went, another came, and HEAD moved: the case the id map may not explain.
    const renames =
      headMoved && gone.length > 0 && fresh.length > 0
        ? await gitRenames(checkout, before.head as string, next.head as string, env)
        : []
    if (stopped || watched.get(checkout) !== entry) return
    entry.snapshot = next
    // Watchers first: whoever hears of the change may write at once, and that write must be seen.
    if (entry.arch === null || entry.head === null) await openHandles(checkout, entry)
    send(compare(checkout, before, next, renames))
  }

  const announceCollisions = (): void => {
    const found = collisionsOf(new Map([...watched].map(([checkout, entry]) => [checkout, entry.snapshot])))
    const next = new Map(found.map((one) => [one.shortId, JSON.stringify(one.locations)]))
    for (const one of found) {
      if (collisions.get(one.shortId) !== next.get(one.shortId)) {
        send([{ method: 'task.collision', params: { shortId: one.shortId, locations: one.locations } }])
      }
    }
    collisions = next
  }

  const flush = async (): Promise<void> => {
    const batch = [...dirty].filter((checkout) => watched.has(checkout))
    dirty.clear()
    await Promise.all(batch.map(refresh))
    if (!stopped && batch.length > 0) announceCollisions()
  }

  const tick = (): void => {
    if (stopped) return
    for (const checkout of watched.keys()) dirty.add(checkout)
    enqueue(flush)
  }

  const set = async (checkouts: string[]): Promise<void> => {
    const wanted = [...new Set(checkouts.map((checkout) => resolve(checkout)))]
    for (const [checkout, entry] of watched) {
      if (wanted.includes(checkout)) continue
      closeHandles(entry)
      watched.delete(checkout)
      dirty.delete(checkout)
    }

    const added = wanted.filter((checkout) => !watched.has(checkout))
    const reads = await Promise.all(
      added.map(async (checkout) => {
        try {
          return { checkout, snapshot: await takeSnapshot(checkout), reason: null }
        } catch (error) {
          return { checkout, snapshot: null, reason: `the checkout ${checkout} could not be read: ${message(error)}` }
        }
      }),
    )
    if (stopped) return
    const unreadable: { reason: string; checkout: string }[] = []
    for (const { checkout, snapshot, reason } of reads) {
      watched.set(checkout, { snapshot: snapshot ?? emptySnapshot(), stale: snapshot === null, arch: null, head: null })
      if (reason !== null) unreadable.push({ reason, checkout })
    }
    await Promise.all([...watched].map(([checkout, entry]) => openHandles(checkout, entry)))

    // What is there now is not news: core lists at start. Collisions are announced when their locations change.
    collisions = new Map(
      collisionsOf(new Map([...watched].map(([checkout, entry]) => [checkout, entry.snapshot]))).map((one) => [
        one.shortId,
        JSON.stringify(one.locations),
      ]),
    )

    if (watched.size === 0 && recheck !== null) {
      timers.clearInterval(recheck)
      recheck = null
    } else if (watched.size > 0 && recheck === null) {
      recheck = timers.setInterval(tick, recheckMs)
    }
    // After the reply, so that the answer to `watch` is the first thing the caller reads.
    for (const { reason, checkout } of unreadable) setImmediate(() => sendStale(reason, checkout))
  }

  const stop = (): void => {
    stopped = true
    if (debounce !== null) timers.clearTimeout(debounce)
    if (recheck !== null) timers.clearInterval(recheck)
    debounce = null
    recheck = null
    for (const entry of watched.values()) closeHandles(entry)
    watched.clear()
    dirty.clear()
  }

  return { set, stop }
}
