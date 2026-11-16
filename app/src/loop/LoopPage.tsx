// The window's one page for the thin loop (Stage 3c; ts-0081-wm): the repository, the prompt box, the two `claude`
// version chips, the run and the one-item Queue. It is a function of `LoopState`, and what a person does is a call on
// `LoopActions`, so a test renders it for any state with no browser. Stage 6a brings the tabs, and Stage 9a replaces
// it.
import { TooltipProvider } from '@/components/ui/tooltip'
import { Chips } from './Chips.tsx'
import { PromptBox } from './PromptBox.tsx'
import { Queue } from './Queue.tsx'
import { Repository } from './Repository.tsx'
import { RunCard } from './RunCard.tsx'
import type { Loop } from './loop.ts'
import type { LoopState } from './state.ts'
import { versionChips } from './words.ts'

/** What the page calls. A `Loop` has all of it. */
export type LoopActions = Pick<
  Loop,
  | 'chooseBoard'
  | 'openFolder'
  | 'startRun'
  | 'allowOnce'
  | 'deny'
  | 'accept'
  | 'reprompt'
  | 'discard'
  | 'keep'
  | 'pickFile'
  | 'openItem'
>

export interface LoopPageProps {
  readonly state: LoopState
  readonly actions: LoopActions
  /** The wall clock "now", passed in so a render is the same every time. */
  readonly nowMs: number
}

export function LoopPage({ state, actions, nowMs }: LoopPageProps) {
  const chips = versionChips(state.versions, nowMs)
  const open = state.run === null ? null : (state.items.find((i) => i.run === state.run?.id) ?? null)
  const others = state.items.filter((i) => i !== open)
  return (
    <TooltipProvider>
      <div className="mx-auto flex min-h-screen max-w-3xl flex-col gap-6 p-4">
        <header className="flex flex-wrap items-center justify-between gap-2">
          <h1 className="text-2xl font-semibold">Collide</h1>
          {chips === null ? null : <Chips inUse={chips.inUse} published={chips.published} />}
        </header>
        <div className="flex flex-col gap-2 empty:hidden">
          {state.connection === 'lost' ? (
            <p role="status" className="rounded-md border p-3 text-sm">
              Lost contact with Collide. This page may be behind, and it will catch up when contact comes back.
            </p>
          ) : null}
          {state.problem === null ? null : (
            <p role="alert" className="rounded-md border border-destructive/40 p-3 text-sm text-destructive">
              {state.problem}
            </p>
          )}
          {state.refusal === null ? null : (
            <p role="alert" className="rounded-md border border-destructive/40 p-3 text-sm text-destructive">
              {state.refusal}
            </p>
          )}
          {state.notice === null ? null : (
            <p role="status" className="rounded-md border p-3 text-sm">
              {state.notice}
            </p>
          )}
        </div>
        {state.loading ? <p className="text-sm text-muted-foreground">Loading…</p> : null}
        <Repository
          boards={state.boards}
          boardId={state.boardId}
          problems={state.folderProblems}
          onChoose={actions.chooseBoard}
          onOpenFolder={actions.openFolder}
        />
        <PromptBox ready={state.boardId !== null} sending={state.sending} onStart={actions.startRun} />
        {state.run === null || state.followed === null ? null : <RunCard run={state.run} followed={state.followed} />}
        <Queue
          open={open}
          others={others}
          diff={open?.kind === 'review' ? state.diff : null}
          picked={state.picked}
          patch={state.patch}
          sending={state.sending}
          confirmingDiscard={state.confirmingDiscard}
          actions={actions}
        />
      </div>
    </TooltipProvider>
  )
}
