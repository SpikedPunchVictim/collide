import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Frame } from './frame/Frame.tsx'
import { TABS, type TabId } from './frame/tabs.ts'
import { LoopPage } from './loop/LoopPage.tsx'
import { useLoop } from './loop/useLoop.ts'
import { type OwnerClient, createTauriOwner } from './owner/index.ts'

// The window is the board's frame (Stage 6a): the top bar and the five tabs. The thin loop's page (Stage 3c) stays
// reachable from Queue and Tracks, with a way back, until Stage 9a replaces it. One loop serves both, so the board
// chosen in the frame's menu is the board the loop page prompts, and nothing about a run resets when the page changes.
const shellOwner = createTauriOwner()

export interface AppProps {
  /** The way to the daemon. The shell's, unless a test gives another. */
  readonly owner?: OwnerClient
}

export function App({ owner = shellOwner }: AppProps) {
  const { state, loop } = useLoop(owner)
  const [tab, setTab] = useState<TabId>('field')
  const [onLoopPage, setOnLoopPage] = useState(false)
  if (onLoopPage) {
    const from = TABS.find((entry) => entry.id === tab)?.name ?? 'the tabs'
    return (
      <>
        <div className="mx-auto max-w-3xl px-4 pt-4">
          <Button variant="ghost" size="sm" onClick={() => setOnLoopPage(false)}>
            {`Back to ${from}`}
          </Button>
        </div>
        <LoopPage state={state} actions={loop} nowMs={Date.now()} />
      </>
    )
  }
  return (
    <Frame
      tab={tab}
      onTab={setTab}
      boards={state.boards}
      boardId={state.boardId}
      folderProblems={state.folderProblems}
      onChooseBoard={loop.chooseBoard}
      onOpenFolder={loop.openFolder}
      onOpenLoop={() => setOnLoopPage(true)}
    />
  )
}
