// What a person's click does once a later stage builds it. Each body is a stub with a tag that names the work and, in
// `stub-tags.json`, the stage that fills it (0.15). A stub that is reached throws, so a placeholder button that is
// pressed in the dev window fails loudly instead of seeming to work. Selecting a task, showing a surface, clearing a
// lens and Undo are not here: they are reducer actions and work now.
import type { SurfaceId } from '../ui/index.ts'
import { unimplemented } from '../unimplemented.ts'
import type { ReviewDecision } from './run/ReviewPane.tsx'
import type { AnswerKind } from './types.ts'

export interface Actions {
  readonly launch: (taskKeys: readonly string[]) => void
  readonly answerQueueItem: (itemId: string, answer: AnswerKind) => void
  readonly decideReview: (runId: string, decision: ReviewDecision) => void
  /** Opening an item by hand has no reducer action yet: only the agent's `open` does (see the item's decisions). */
  readonly openItem: (surface: SurfaceId, id: string) => void
  readonly compareRuns: (leftId: string, rightId: string) => void
  readonly startExperiment: () => void
}

export const stubActions: Actions = {
  launch: () => unimplemented('ui: launch'),
  answerQueueItem: () => unimplemented('ui: queue answer'),
  decideReview: () => unimplemented('ui: review decision'),
  openItem: () => unimplemented('ui: open item'),
  compareRuns: () => unimplemented('ui: compare runs'),
  startExperiment: () => unimplemented('ui: experiment start'),
}
