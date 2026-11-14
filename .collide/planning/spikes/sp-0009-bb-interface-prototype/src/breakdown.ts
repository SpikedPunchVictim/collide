// Tool-call time breakdown for a run, a phase or a time window (plan 0.11).
import type { BreakdownFilter, Run, Span } from './types'

export interface Row {
  key: string
  name: string
  calls: number
  total: number
  share: number
  median: number
  max: number
  errors: number
  children: Row[]
}

export interface Breakdown {
  rows: Row[]
  /** clock time in the scope */
  wall: number
  /** sum of the matched top-level calls, clipped to the window */
  summed: number
  /** clock time during which at least one matched call ran */
  union: number
  matched: Span[]
}

export const NO_FILTER: BreakdownFilter = { tool: null, minDur: 0, win: null, phase: null }

const endOf = (s: Span, now: number): number => s.end ?? now
export const durOf = (s: Span, now: number): number => Math.max(0, endOf(s, now) - s.start)

/** The keys a span answers to in the tool filter: its tool, tool:exe, and mcp for any server. */
export function keysOf(s: Span): string[] {
  const keys = [s.tool]
  if (s.exe) keys.push(`${s.tool}:${s.exe}`)
  if (s.tool.startsWith('mcp:')) keys.push('mcp')
  return keys
}

export const groupOf = (s: Span): string => (s.kind === 'hook' ? 'hooks' : s.tool.startsWith('mcp:') ? 'mcp' : s.tool)

export function filterSpans(run: Run, f: BreakdownFilter, now: number): Span[] {
  return run.spans.filter((s) => {
    if (s.kind === 'text') return false
    if (f.phase && s.phase !== f.phase) return false
    if (f.tool && !keysOf(s).includes(f.tool)) return false
    if (durOf(s, now) < f.minDur) return false
    if (f.win && !(s.start < f.win[1] && endOf(s, now) > f.win[0])) return false
    return true
  })
}

function median(xs: number[]): number {
  if (!xs.length) return 0
  const a = [...xs].sort((x, y) => x - y)
  const m = Math.floor(a.length / 2)
  return a.length % 2 ? a[m] : (a[m - 1] + a[m]) / 2
}

function unionLength(iv: [number, number][]): number {
  const a = [...iv].sort((x, y) => x[0] - y[0])
  let total = 0
  let cur: [number, number] | null = null
  for (const [s, e] of a) {
    if (!cur || s > cur[1]) {
      if (cur) total += cur[1] - cur[0]
      cur = [s, e]
    } else cur[1] = Math.max(cur[1], e)
  }
  if (cur) total += cur[1] - cur[0]
  return total
}

function clip(s: Span, f: BreakdownFilter, now: number): number {
  const a = f.win ? Math.max(s.start, f.win[0]) : s.start
  const b = f.win ? Math.min(endOf(s, now), f.win[1]) : endOf(s, now)
  return Math.max(0, b - a)
}

function rowFor(key: string, name: string, spans: Span[], f: BreakdownFilter, now: number, scope: number): Row {
  const durs = spans.map((s) => durOf(s, now))
  const total = spans.reduce((a, s) => a + clip(s, f, now), 0)
  return {
    key, name, calls: spans.length, total, share: scope > 0 ? total / scope : 0, median: median(durs),
    max: durs.length ? Math.max(...durs) : 0, errors: spans.filter((s) => s.error).length, children: [],
  }
}

function scopeOf(run: Run, f: BreakdownFilter, now: number): [number, number] {
  const ph = f.phase ? run.phases.filter((p) => p.key === f.phase) : run.phases
  const a = Math.min(...ph.map((p) => p.start ?? now), now)
  const b = Math.max(...ph.map((p) => p.end ?? now), a)
  if (!f.win) return [a, b]
  return [Math.max(a, f.win[0]), Math.max(Math.max(a, f.win[0]), Math.min(b, f.win[1]))]
}

export function breakdown(run: Run, f: BreakdownFilter, now: number): Breakdown {
  const matched = filterSpans(run, f, now)
  const [a, b] = scopeOf(run, f, now)
  const wall = b - a
  const top = matched.filter((s) => !s.parent)
  const groups = new Map<string, Span[]>()
  for (const s of top) {
    const g = groupOf(s)
    groups.set(g, [...(groups.get(g) ?? []), s])
  }
  const rows: Row[] = []
  for (const [g, spans] of groups) {
    const row = rowFor(g, g, spans, f, now, wall)
    const sub = new Map<string, Span[]>()
    if (g === 'Bash') for (const s of spans) sub.set(`Bash:${s.exe}`, [...(sub.get(`Bash:${s.exe}`) ?? []), s])
    if (g === 'mcp') for (const s of spans) sub.set(s.tool, [...(sub.get(s.tool) ?? []), s])
    if (g === 'Task') {
      for (const s of matched.filter((x) => x.parent && spans.some((p) => p.id === x.parent))) {
        sub.set(s.tool, [...(sub.get(s.tool) ?? []), s])
      }
    }
    for (const [k, v] of sub) row.children.push(rowFor(k, k.replace('Bash:', ''), v, f, now, wall))
    row.children.sort((x, y) => y.total - x.total)
    rows.push(row)
  }
  rows.sort((x, y) => y.total - x.total)
  const iv = (xs: Span[]): [number, number][] =>
    xs.map((s) => [f.win ? Math.max(s.start, f.win[0]) : s.start, f.win ? Math.min(endOf(s, now), f.win[1]) : endOf(s, now)])
  const work = top.filter((s) => s.kind !== 'wait')
  const allBusy = run.spans.filter((s) => s.kind !== 'text' && !s.parent && (!f.phase || s.phase === f.phase))
  const model = Math.max(0, wall - unionLength(iv(allBusy).map(([x, y]) => [Math.max(x, a), Math.min(y, b)] as [number, number]).filter(([x, y]) => y > x)))
  if (!f.tool && f.minDur === 0) {
    rows.unshift({ key: 'model', name: 'model (estimate)', calls: 0, total: model, share: wall > 0 ? model / wall : 0, median: 0, max: 0, errors: 0, children: [] })
  }
  return {
    rows, wall, summed: work.reduce((acc, s) => acc + clip(s, f, now), 0), union: unionLength(iv(work)), matched,
  }
}

export function slowest(spans: Span[], now: number): Span | undefined {
  return [...spans].sort((x, y) => durOf(y, now) - durOf(x, now))[0]
}

/** Total time per phase kind, split into active and wait, for the compare view. */
export function phaseTimes(run: Run): { key: string; label: string; wall: number; wait: number; tool: number }[] {
  return run.phases.map((p) => {
    const end = p.end ?? run.endWall ?? p.start ?? 0
    const wall = end - (p.start ?? end)
    const mine = run.spans.filter((s) => s.phase === p.key && !s.parent)
    const wait = mine.filter((s) => s.kind === 'wait').reduce((a, s) => a + ((s.end ?? end) - s.start), 0)
    const tool = mine.filter((s) => s.kind !== 'wait' && s.kind !== 'text').reduce((a, s) => a + ((s.end ?? end) - s.start), 0)
    return { key: p.key, label: p.reviewer ? `review ${p.reviewer}` : p.label, wall, wait, tool }
  })
}
