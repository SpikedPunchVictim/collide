// A clock a test moves by hand, so that nothing waits for real time (ts-0182-ad). It has the shape `Clock` of
// `src/field/live.ts` asks for.
import type { Clock } from '../../src/field/live.ts'

interface Timer {
  readonly at: number
  readonly run: () => void
}

export class FakeClock implements Clock {
  private now = 0
  private next = 1
  private readonly timers = new Map<number, Timer>()

  setTimeout(run: () => void, ms: number): number {
    const id = this.next++
    this.timers.set(id, { at: this.now + ms, run })
    return id
  }

  clearTimeout(handle: unknown): void {
    this.timers.delete(handle as number)
  }

  /** How many timers wait. */
  pending(): number {
    return this.timers.size
  }

  /** Moves time on by `ms`, running each timer that falls due, in order. */
  advance(ms: number): void {
    const end = this.now + ms
    for (;;) {
      const due = [...this.timers.entries()].filter(([, timer]) => timer.at <= end).sort((a, b) => a[1].at - b[1].at)[0]
      if (due === undefined) break
      this.timers.delete(due[0])
      this.now = due[1].at
      due[1].run()
    }
    this.now = end
  }
}
