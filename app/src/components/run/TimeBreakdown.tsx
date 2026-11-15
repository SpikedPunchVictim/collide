// Tool-call time breakdown (0.11): a table of where a run's time went. The generated types have no breakdown record yet
// (Stage 9c's metrics define it), so the row is typed here; the real rows, the filters and the timeline are Stage 9b.
import { durationWords } from '../words.ts'

export interface BreakdownRow {
  /** Plain name of the activity: "Model time", a tool's name, "Hooks", "Waiting for you". */
  readonly name: string
  readonly calls: number
  readonly totalMs: number
  /** Part of the sum of the rows, `0` to `1`, so the rows add to 1. */
  readonly share: number
  readonly errors: number
}

export interface TimeBreakdownProps {
  readonly rows: readonly BreakdownRow[]
}

export function TimeBreakdown({ rows }: TimeBreakdownProps) {
  return (
    <table className="w-full text-left text-sm">
      <thead>
        <tr className="border-b text-muted-foreground">
          <th className="py-1 font-medium">Time spent on</th>
          <th className="py-1 font-medium">Calls</th>
          <th className="py-1 font-medium">Total</th>
          <th className="py-1 font-medium">Share</th>
          <th className="py-1 font-medium">Errors</th>
        </tr>
      </thead>
      <tbody>
        {rows.map((row) => (
          <tr key={row.name} className="border-b last:border-0">
            <td className="py-1">{row.name}</td>
            <td className="py-1">{row.calls}</td>
            <td className="py-1">{durationWords(row.totalMs)}</td>
            <td className="py-1">{Math.round(row.share * 100)}%</td>
            <td className="py-1">{row.errors}</td>
          </tr>
        ))}
      </tbody>
    </table>
  )
}
