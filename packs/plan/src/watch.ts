import { watch as fsWatch } from 'node:fs'
import { stat } from 'node:fs/promises'
import { isAbsolute, join, relative, resolve, sep } from 'node:path'
import type { ListResultInput, Notify, TaskInput, TaskInvalidOutput } from '@collide/pack-sdk'
import { collectionsChanged, compare, emptySnapshot, idsChanged, snapshotOf } from './changes.ts'
import type { Notification, Snapshot } from './changes.ts'
import { gitDir, gitHead, gitRenames } from './git.ts'
import { SPIKE_RECORD } from './scan.ts'

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
  /** The environment git runs in. This process's own when it is left out. */
  env?: NodeJS.ProcessEnv
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

const isFolder = async (path: string): Promise<boolean> => (await stat(path).catch(() => null))?.isDirectory() === true

/** What the provider tells the watcher about a checkout: how to read it, and what to watch in it. */
export interface Where {
  /** The items and invalid files of a checkout, as `list` reads them. It throws when the checkout cannot be read. */
  read(checkout: string): Promise<{
    tasks: TaskInput[]
    collections: NonNullable<ListResultInput['collections']>
    invalid: TaskInvalidOutput[]
  }>
  /**
   * Where to look for a change: the folders to watch (each with everything below it), the folders that hold spike
   * folders, and the paths git is asked about when it is asked for renames. All are absolute but `pathspecs`.
   */
  places(checkout: string): Promise<{ folders: string[]; spikes: string[]; pathspecs: string[] }>
  /**
   * The snapshot the last `list` of this checkout was computed from, or undefined when there is none. It is given
   * once: asking for it takes it. A watcher that starts from it hears of what changed after that `list`.
   */
  listed?(checkout: string): Snapshot | undefined
}

/**
 * Whether a change at `path` is a change of the plan. A spike folder is read for its `record.md` and nothing else, so
 * a file anywhere else below a spike folder (`out/`, `work/`, `node_modules/`, another `record.md`) is not
 * (`ts-0126-nf`).
 * The folder itself is: it may be new, or gone.
 */
export function isPlanChange(path: string, spikes: readonly string[]): boolean {
  for (const dir of spikes) {
    const inside = relative(dir, path)
    if (inside === '' || inside === '..' || inside.startsWith(`..${sep}`) || isAbsolute(inside)) continue
    const parts = inside.split(sep)
    return parts.length === 1 || (parts.length === 2 && parts[1] === SPIKE_RECORD)
  }
  return true
}

/** What the watcher holds for one checkout: its last snapshot and the watchers it opened. */
interface Watched {
  snapshot: Snapshot
  /** Whether the last `stale` was sent and no read has worked since. */
  stale: boolean
  /** The folders being watched, by path. */
  handles: Map<string, FsHandle>
}

export interface Watcher {
  /** Makes these the checkouts that are watched: the others are dropped, new ones read once and given watchers. */
  set(checkouts: string[]): Promise<void>
  /** Closes every watcher and clock. Nothing is sent after it. */
  stop(): void
}

/**
 * Watches checkouts of one repository and sends what changes in them as notifications.
 * - Each checkout's planning folders, the private root of the main checkout and the folder of the git HEAD file are
 *   watched. An event, or the recheck, marks the checkout; a quiet spell of `debounceMs` reads every marked checkout
 *   once and compares it with its last snapshot.
 * - `set` reads each new checkout before it returns, so a change made after the reply is a change. It sends no event
 *   for what is already there: core lists at start. A checkout that was listed starts from that list instead of a
 *   read, and is read once more right after the reply, so a change made between the list and the watch is sent then.
 * - A checkout that cannot be read is `source.stale`, once, until it can be read again. Its last snapshot stays.
 */
export function createWatcher(deps: {
  where: Where
  log: (message: string) => void
  notify: Notify
  options?: WatchOptions
}): Watcher {
  const { where, log, notify } = deps
  const options = deps.options ?? {}
  const { debounceMs = DEBOUNCE_MS, recheckMs = RECHECK_MS, env = process.env } = options
  const { timers = nodeTimers, watchFs = nodeWatchFs } = options

  const watched = new Map<string, Watched>()
  const dirty = new Set<string>()
  let debounce: unknown = null
  let recheck: unknown = null
  let queue: Promise<void> = Promise.resolve()
  let stopped = false

  // The same complaint about the same folder, every ten seconds, would fill the log.
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
    for (const handle of entry.handles.values()) handle.close()
    entry.handles.clear()
  }

  /** Opens the watchers the checkout lacks. A planning folder may not exist yet; the recheck tries again. */
  const openHandles = async (checkout: string, entry: Watched): Promise<void> => {
    const open = (target: string, recursive: boolean, accept: (path: string | null) => boolean): void => {
      try {
        const handle = watchFs(
          target,
          { recursive },
          (name) => {
            if (accept(name === null ? null : join(target, name))) markDirty(checkout)
          },
          (error) => {
            logOnce(`watching ${target}: ${error.message}`)
            entry.handles.get(target)?.close()
            entry.handles.delete(target)
          },
        )
        entry.handles.set(target, handle)
      } catch (error) {
        logOnce(`cannot watch ${target}: ${message(error)}`)
      }
    }
    const { folders, spikes } = await where.places(checkout)
    for (const folder of folders) {
      if (!entry.handles.has(folder) && (await isFolder(folder))) {
        open(folder, true, (path) => path === null || isPlanChange(path, spikes))
      }
    }
    const dir = await gitDir(checkout, env)
    // Git replaces HEAD by renaming a lock file over it, so the folder is watched and the name is checked.
    if (dir !== null && !entry.handles.has(dir)) {
      open(dir, false, (path) => path === null || path === join(dir, 'HEAD'))
    }
    // The checkout may have been dropped, or the provider stopped, while git answered.
    if (stopped || watched.get(checkout) !== entry) closeHandles(entry)
  }

  /** Reads a checkout. HEAD is read on both sides of the files, so a snapshot never mixes two commits for long. */
  const takeSnapshot = async (checkout: string): Promise<Snapshot> => {
    for (let attempt = 0; ; attempt++) {
      const head = await gitHead(checkout, env)
      const found = await where.read(checkout)
      if (attempt >= 2 || (await gitHead(checkout, env)) === head) {
        return snapshotOf(head, found)
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
        notify('source.stale', { reason: `the checkout ${checkout} could not be read: ${message(error)}`, checkout })
      }
      return
    }
    if (stopped || watched.get(checkout) !== entry) return
    entry.stale = false

    const before = entry.snapshot
    const { gone, fresh } = idsChanged(before, next)
    const headMoved = before.head !== null && next.head !== null && before.head !== next.head
    // Git is asked only when an item went, another came, and HEAD moved: the one case that may be a rename.
    const renames =
      headMoved && gone.length > 0 && fresh.length > 0
        ? await gitRenames(
            checkout,
            before.head as string,
            next.head as string,
            (await where.places(checkout)).pathspecs,
            env,
          )
        : []
    if (stopped || watched.get(checkout) !== entry) return
    entry.snapshot = next
    // Watchers first: whoever hears of the change may write at once, and that write must be seen.
    await openHandles(checkout, entry)
    send([...compare(checkout, before, next, renames), ...collectionsChanged(checkout, before, next)])
  }

  const flush = async (): Promise<void> => {
    const batch = [...dirty].filter((checkout) => watched.has(checkout))
    dirty.clear()
    await Promise.all(batch.map(refresh))
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
    const fromList = new Set<string>()
    const reads = await Promise.all(
      added.map(async (checkout) => {
        const remembered = where.listed?.(checkout)
        if (remembered !== undefined) {
          fromList.add(checkout)
          return { checkout, snapshot: remembered, reason: null }
        }
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
      watched.set(checkout, { snapshot: snapshot ?? emptySnapshot(), stale: snapshot === null, handles: new Map() })
      if (reason !== null) unreadable.push({ reason, checkout })
    }
    await Promise.all([...watched].map(([checkout, entry]) => openHandles(checkout, entry)))

    if (watched.size === 0 && recheck !== null) {
      timers.clearInterval(recheck)
      recheck = null
    } else if (watched.size > 0 && recheck === null) {
      recheck = timers.setInterval(tick, recheckMs)
    }
    // After the reply, so that the answer to `watch` is the first thing the caller reads.
    for (const { reason, checkout } of unreadable) {
      setImmediate(() => !stopped && notify('source.stale', { reason, checkout }))
    }
    // The watchers are open, so a read now misses nothing: what changed since the list is sent without waiting.
    if (fromList.size > 0) {
      setImmediate(() => {
        if (stopped) return
        for (const checkout of fromList) dirty.add(checkout)
        enqueue(flush)
      })
    }
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
