import type { ReactNode } from 'react'
import { fmtDur, livePos } from '../world'
import type { Run, RunStatus, State, TaskStatus } from '../types'

export const STATE_LABEL: Record<string, string> = {
  working: 'working', waiting: 'waiting on you', failed: 'held at gate', paused: 'paused', done: 'done', stopped: 'stopped',
  ready: 'ready', blocked: 'blocked', backlog: 'backlog', running: 'running', review: 'in review',
}

/** Colour is reserved for state. The one exception is the `colour` identity variant, kept as a control. */
export const RUN_COLOURS = ['#00808c', '#b3286e', '#3d41a3', '#5f7a10', '#8a4a2c', '#7a5a00']

export function Chip({ st, children }: { st: RunStatus | TaskStatus; children?: ReactNode }) {
  return (
    <span className={`chip s-${st}`}>
      <i className="dot" />
      {children ?? STATE_LABEL[st]}
    </span>
  )
}

export function Kbd({ children }: { children: ReactNode }) {
  return <kbd>{children}</kbd>
}

/** How a run is told apart from another: the S6 open question on run identity. */
export function RunTag({ s, run }: { s: State; run: Run }) {
  const v = s.variants.identity
  const title = `${run.id}, lane ${run.lane}`
  if (v === 'id') return <span className="rtag mono" title={title}>{run.id}</span>
  if (v === 'lane') return <span className="rtag lane mono" title={title}>lane {run.lane}</span>
  if (v === 'position') {
    const p = livePos(s, run)
    return <span className="rtag mono" title={title}>{p > 0 ? `#${p}` : 'done'}</span>
  }
  const c = RUN_COLOURS[run.seq % RUN_COLOURS.length]
  return (
    <span className="rtag mono" title={title}>
      <i className="swatch" style={{ background: c }} />
      {run.id}
    </span>
  )
}

export function runColour(s: State, run: Run): string | undefined {
  return s.variants.identity === 'colour' ? RUN_COLOURS[run.seq % RUN_COLOURS.length] : undefined
}

export function Age({ sec }: { sec: number }) {
  return <span className="mono">{fmtDur(Math.max(0, sec))}</span>
}

export const usd = (n: number): string => `$${n.toFixed(2)}`
