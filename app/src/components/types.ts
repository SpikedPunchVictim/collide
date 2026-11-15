// The shapes the components take as props, named from the generated protocol types (0.6). The generated index exports
// each type under an `Output` name and exports a nested type only through its parent, so the nested ones are read off
// the parent here, once, and no component spells out a wire shape by hand.
import type {
  ContextRecordOutput,
  QueueListResultOutput,
  RunGetResultOutput,
  RunOutputResultOutput,
  TaskOutput,
} from '../owner/generated.ts'

export type Task = TaskOutput
export type QueueItem = QueueListResultOutput['items'][number]
export type RunSummary = RunGetResultOutput['summary']
export type PhaseSummary = RunSummary['phases'][number]
export type OutputEntry = RunOutputResultOutput['entries'][number]
export type ContextRecord = ContextRecordOutput
export type RunState = RunSummary['state']
export type PauseCause = Extract<RunState, { state: 'paused' }>['cause']
export type QueueKind = QueueItem['kind']
export type AnswerKind = QueueItem['answers'][number]
export type ToolKind = NonNullable<QueueItem['permission']>['kind']
