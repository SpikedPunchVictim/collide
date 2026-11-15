// Compare (0.11): any two finished runs side by side, by phase, with no verdict. The phase durations come from the
// generated `PhaseSummary` times. By-tool rows, the note box and "where time went" over many runs are Stage 9c.
import type { PhaseSummary, RunSummary } from '../types.ts'
import { durationWords } from '../words.ts'

export interface ComparePaneProps {
  readonly left: RunSummary
  readonly right: RunSummary
}

function phaseMs(phase: PhaseSummary | undefined): number | null {
  if (phase?.started_ms == null || phase.ended_ms == null) return null
  return phase.ended_ms - phase.started_ms
}

const show = (ms: number | null) => (ms === null ? 'not reported' : durationWords(ms))

export function ComparePane({ left, right }: ComparePaneProps) {
  const names = [...new Set([...left.phases, ...right.phases].map((p) => p.name))]
  return (
    <section aria-labelledby="compare-heading" className="flex flex-col gap-2 rounded-xl border p-4">
      <h3 id="compare-heading" className="text-lg font-semibold">
        Compare runs
      </h3>
      <table className="w-full text-left text-sm">
        <thead>
          <tr className="border-b text-muted-foreground">
            <th className="py-1 font-medium">Phase</th>
            <th className="py-1 font-medium">{left.id}</th>
            <th className="py-1 font-medium">{right.id}</th>
            <th className="py-1 font-medium">Difference</th>
          </tr>
        </thead>
        <tbody>
          {names.map((name) => {
            const a = phaseMs(left.phases.find((p) => p.name === name))
            const b = phaseMs(right.phases.find((p) => p.name === name))
            return (
              <tr key={name} className="border-b last:border-0">
                <td className="py-1">{name}</td>
                <td className="py-1">{show(a)}</td>
                <td className="py-1">{show(b)}</td>
                <td className="py-1">{a === null || b === null ? '' : `${b >= a ? '+' : '-'}${durationWords(Math.abs(b - a))}`}</td>
              </tr>
            )
          })}
        </tbody>
      </table>
      <p className="text-sm text-muted-foreground">Two runs show a difference, not a cause.</p>
    </section>
  )
}
