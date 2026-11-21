// The shapes the thin-loop page holds, named from the generated protocol types, as `components/types.ts` does.
import type { QueueItem, RunSummary } from '../components/types.ts'
import type {
  BoardListResultOutput,
  DaemonAgentVersionsResultOutput,
  QueueAnswerParamsInput,
  QueueAnswerResultOutput,
  RunDiffResultOutput,
} from '../owner/generated.ts'

export type { QueueItem, RunSummary }
export type BoardSummary = BoardListResultOutput['boards'][number]
export type AgentVersions = DaemonAgentVersionsResultOutput['agents'][number]
export type DiffResult = RunDiffResultOutput
export type DiffFile = DiffResult['files'][number]
export type FilePatch = NonNullable<DiffResult['patch']>
export type ItemAnswer = QueueAnswerParamsInput['answer']
export type AnsweredResult = Exclude<QueueAnswerResultOutput, { outcome: 'refused' }>

/** The run the page shows, and its task. `board` and `prompt` are null for a run whose task is not found. */
export interface Followed {
  readonly board: string | null
  readonly task: string | null
  readonly prompt: string | null
  readonly run: string
}
