// The shapes the Field holds, named from the generated protocol types (0.6). A nested wire type is read off its
// parent here, once, as `components/types.ts` does.
import type { TaskListResultOutput, TaskOutput, TaskViewsResultOutput } from '../owner/generated.ts'

export type Task = TaskOutput
export type Category = Task['state']['category']
export type ListResult = TaskListResultOutput
export type SourceDecl = ListResult['sources'][number]
export type Divergence = ListResult['divergence'][number]
export type DivergentCopy = Divergence['copies'][number]
export type Collision = ListResult['collisions'][number]
export type BranchOnly = ListResult['branch_only'][number]
export type ViewReport = NonNullable<ListResult['view']>
export type ViewEntry = TaskViewsResultOutput['views'][number]
export type Collection = ListResult['collections'][number]

/** What one load of the Field read: the shown tasks, every task of the board, and the board-wide lists. */
export interface FieldData {
  /** The tasks of the selected view, one entry for each place a key is at. */
  readonly tasks: readonly Task[]
  /**
   * The tasks of the selected view before the filter bar's choices, which its groups are counted over. The same as
   * `tasks` when nothing is chosen.
   */
  readonly inView: readonly Task[]
  /** Every task of the board, which a card's blockers are looked up in. */
  readonly corpus: readonly Task[]
  readonly sources: readonly SourceDecl[]
  readonly collisions: readonly Collision[]
  readonly divergence: readonly Divergence[]
  /** The tasks whose only copy is in a worktree, with the branch it is on. */
  readonly branchOnly: readonly BranchOnly[]
  /** What the selected view said of itself. Null for the whole board. */
  readonly report: ViewReport | null
  /** What the sources list as collections (an epic is one), from the first page of the listing. */
  readonly collections: readonly Collection[]
  /**
   * What the daemon said of the view with a goal's closure added (the run order of a collection), or null when no
   * goal was asked for or the walk failed.
   */
  readonly walked: ViewReport | null
  /** The words of a walk the daemon refused, such as a goal that is no longer there. Null when there is none. */
  readonly walkProblem: string | null
  /** The words of a filter the daemon refused, with no task shown. Null when there is none. */
  readonly filterProblem: string | null
  /**
   * The daemon was still listing, or a source had not started, so `sources` may be short. An event for a source not in
   * it may be this board's source coming up (`live.ts`).
   */
  readonly settling: boolean
}
