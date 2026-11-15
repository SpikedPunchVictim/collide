// Run detail (0.11): one run's Output, Time breakdown and Review, as three tabs. Plain words first; the engineer's
// detail (the output, the table) is one tab away and never removed. Which tab is open is this component's own state,
// started from `tab`; it is not in the reducer yet (see the item's decisions).
import { Badge } from '@/components/ui/badge'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import type { OutputEntry, RunSummary } from '../types.ts'
import { runStateWords } from '../words.ts'
import { OutputPane } from './OutputPane.tsx'
import { type ReviewData, type ReviewDecision, ReviewPane } from './ReviewPane.tsx'
import { type BreakdownRow, TimeBreakdown } from './TimeBreakdown.tsx'

export type RunTab = 'output' | 'time' | 'review'

export interface RunDetailProps {
  readonly run: RunSummary
  readonly entries: readonly OutputEntry[]
  readonly breakdown: readonly BreakdownRow[]
  readonly review: ReviewData
  /** The tab open first. */
  readonly tab?: RunTab
  readonly onDecide: (runId: string, decision: ReviewDecision) => void
}

export function RunDetail({ run, entries, breakdown, review, tab = 'output', onDecide }: RunDetailProps) {
  return (
    <section aria-label={`Run ${run.id}`} className="flex flex-col gap-3 rounded-xl border p-4">
      <header className="flex items-center gap-2">
        <h3 className="text-lg font-semibold">{run.id}</h3>
        <Badge variant="secondary">{runStateWords(run.state)}</Badge>
      </header>
      <Tabs defaultValue={tab}>
        <TabsList>
          <TabsTrigger value="output">Output</TabsTrigger>
          <TabsTrigger value="time">Time breakdown</TabsTrigger>
          <TabsTrigger value="review">Review</TabsTrigger>
        </TabsList>
        <TabsContent value="output">
          <OutputPane entries={entries} />
        </TabsContent>
        <TabsContent value="time">
          <TimeBreakdown rows={breakdown} />
        </TabsContent>
        <TabsContent value="review">
          <ReviewPane run={run} review={review} onDecide={onDecide} />
        </TabsContent>
      </Tabs>
    </section>
  )
}
