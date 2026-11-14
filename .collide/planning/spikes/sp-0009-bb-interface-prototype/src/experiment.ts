// Fake experiment results. The pair in S6 scenario 7 has a fixed answer; other pairs get stable numbers from the names.
import type { Experiment } from './types'

export interface ArmRow {
  phase: string
  a: number
  b: number
}

export interface ExpResult {
  rows: ArmRow[]
  /** critical path of each arm, minutes */
  critA: number
  critB: number
  outcomeA: string
  outcomeB: string
  winner: 'A' | 'B' | 'none'
  verdict: string
}

const hash = (s: string): number => s.split('').reduce((a, c) => (a * 31 + c.charCodeAt(0)) >>> 0, 7)

export function experimentResult(e: Experiment): ExpResult {
  const h = hash(e.armA + '|' + e.armB)
  const fixed = [e.armA, e.armB].sort().join() === 'exp/lru-cache,exp/ttl-cache'
  // fixed pair: lru wins by 14 minutes on the critical path
  const lruIsB = e.armB === 'exp/lru-cache'
  const gap = fixed ? 14 : ((h % 9) - 4) * 2
  const base = 58
  const slowArm = fixed ? (lruIsB ? 'A' : 'B') : gap >= 0 ? 'A' : 'B'
  const critSlow = base + Math.abs(gap)
  const critFast = base
  const critA = slowArm === 'A' ? critSlow : critFast
  const critB = slowArm === 'B' ? critSlow : critFast
  const rows: ArmRow[] = [
    { phase: 'provision', a: 4, b: 4 },
    { phase: 'build', a: slowArm === 'A' ? 22 + Math.abs(gap) / 2 : 22, b: slowArm === 'B' ? 22 + Math.abs(gap) / 2 : 22 },
    { phase: 'review', a: 8, b: 8 },
    { phase: 'fix', a: slowArm === 'A' ? 14 + Math.abs(gap) / 2 : 14, b: slowArm === 'B' ? 14 + Math.abs(gap) / 2 : 14 },
    { phase: 'close', a: 1, b: 1 },
  ]
  const fastName = slowArm === 'A' ? e.armB : e.armA
  const winner = gap === 0 && !fixed ? 'none' : slowArm === 'A' ? 'B' : 'A'
  return {
    rows,
    critA,
    critB,
    outcomeA: 'gates pass, 0 findings open',
    outcomeB: 'gates pass, 0 findings open',
    winner,
    verdict: winner === 'none'
      ? 'No difference on the primary metric, on this task.'
      : `${fastName} was faster on this task: its critical path was ${Math.abs(critA - critB)} min shorter (${Math.round((Math.abs(critA - critB) / critSlow) * 100)}%). Outcomes were equal. This is one task, not a general result.`,
  }
}
