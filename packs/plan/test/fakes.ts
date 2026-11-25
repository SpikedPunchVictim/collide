import type { FsHandle, Timers, WatchFs } from '../src/watch.ts'

/** A clock that runs nothing until the test says so. */
export function fakeClock() {
  let next = 1
  const timeouts = new Map<number, { run: () => void; ms: number }>()
  const intervals = new Map<number, { run: () => void; ms: number }>()
  const timers: Timers = {
    setTimeout: (run, ms) => {
      timeouts.set(next, { run, ms })
      return next++
    },
    clearTimeout: (handle) => void timeouts.delete(handle as number),
    setInterval: (run, ms) => {
      intervals.set(next, { run, ms })
      return next++
    },
    clearInterval: (handle) => void intervals.delete(handle as number),
  }
  return {
    timers,
    /** The waits that are set and not yet run, by their length. */
    waits: (): number[] => [...timeouts.values()].map((entry) => entry.ms),
    /** The repeating timers that are set, by their period. */
    periods: (): number[] => [...intervals.values()].map((entry) => entry.ms),
    /** Runs every wait that is set now, as the debounce ending would. */
    runWaits(): void {
      const due = [...timeouts.entries()]
      timeouts.clear()
      due.forEach(([, entry]) => entry.run())
    },
    /** Runs each repeating timer once, as the recheck coming round would. */
    runIntervals(): void {
      ;[...intervals.values()].forEach((entry) => entry.run())
    },
  }
}

export interface Opened {
  target: string
  recursive: boolean
  closed: boolean
  emit(name: string | null): void
  fail(error: Error): void
}

/** A file watcher that watches nothing: it records what was opened and closed, and a test makes the events. */
export function fakeFs() {
  const opened: Opened[] = []
  const watchFs: WatchFs = (target, options, onEvent, onError) => {
    const entry: Opened = {
      target,
      recursive: options.recursive,
      closed: false,
      emit: (name) => {
        if (!entry.closed) onEvent(name)
      },
      fail: (error) => onError(error),
    }
    opened.push(entry)
    const handle: FsHandle = { close: () => void (entry.closed = true) }
    return handle
  }
  return {
    watchFs,
    opened,
    open: (): Opened[] => opened.filter((entry) => !entry.closed),
    /** The watcher of a folder, by its path. */
    at: (target: string): Opened => opened.filter((entry) => entry.target === target && !entry.closed).at(-1) as Opened,
  }
}
