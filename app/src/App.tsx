import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { FieldPage } from './field/FieldPage.tsx'
import { useField } from './field/useField.ts'
import { ViewPicker } from './field/ViewPicker.tsx'
import { boardName, defaultLine } from './frame/boards.ts'
import { Frame } from './frame/Frame.tsx'
import { TABS, type TabId } from './frame/tabs.ts'
import { LoopPage } from './loop/LoopPage.tsx'
import { ExternalLinks } from './markdown/ExternalLinks.tsx'
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

// A link in a task's text asks before it opens, wherever the text is shown, so the window that asks is provided once.
export function App({ owner = shellOwner }: AppProps) {
  return (
    <ExternalLinks owner={owner}>
      <Window owner={owner} />
    </ExternalLinks>
  )
}

function Window({ owner }: { readonly owner: OwnerClient }) {
  const { state, loop } = useLoop(owner)
  const field = useField(owner, state.boardId)
  const [tab, setTab] = useState<TabId>('field')
  const [onLoopPage, setOnLoopPage] = useState(false)
  const board = state.boards.find((each) => each.id === state.boardId)
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
      viewPicker={
        board === undefined ? null : (
          <ViewPicker boardName={boardName(board)} views={field.views} view={field.view} onChoose={field.select} />
        )
      }
      field={
        <FieldPage
          owner={owner}
          boardId={state.boardId}
          board={board === undefined ? null : { name: boardName(board), line: defaultLine(board) }}
          field={field}
        />
      }
    />
  )
}
