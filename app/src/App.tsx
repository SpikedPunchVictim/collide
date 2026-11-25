import { useState } from 'react'
import type { SourceDecl } from './field/types.ts'
import { Button } from '@/components/ui/button'
import { FieldPage } from './field/FieldPage.tsx'
import { useField } from './field/useField.ts'
import { ViewPicker } from './field/ViewPicker.tsx'
import { boardName, defaultLine, isMissing } from './frame/boards.ts'
import { Frame } from './frame/Frame.tsx'
import { LocateWindow } from './frame/LocateWindow.tsx'
import { MissingBoard } from './frame/MissingBoard.tsx'
import { TABS, type TabId } from './frame/tabs.ts'
import { LoopPage } from './loop/LoopPage.tsx'
import { ExternalLinks } from './markdown/ExternalLinks.tsx'
import { useLoop } from './loop/useLoop.ts'
import { type OwnerClient, createTauriOwner } from './owner/index.ts'
import { Peek } from './peek/Peek.tsx'

// The window is the board's frame (Stage 6a): the top bar and the five tabs. The thin loop's page (Stage 3c) stays
// reachable from Queue and Tracks, with a way back, until Stage 9a replaces it. One loop serves both, so the board
// chosen in the frame's menu is the board the loop page prompts, and nothing about a run resets when the page changes.
const shellOwner = createTauriOwner()
const NO_SOURCES: readonly SourceDecl[] = []

export interface AppProps {
  /** The way to the daemon. The shell's, unless a test gives another. */
  readonly owner?: OwnerClient
  /**
   * Whether the window runs in the shell, which has the system's folder picker. The default asks the page: Tauri puts
   * `__TAURI_INTERNALS__` on `window`. A browser page (the bridge's, the fake owner's) has no picker and keeps the
   * typed path. A test says which.
   */
  readonly shell?: boolean
}

const inShell = () => typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

// A link in a task's text asks before it opens, wherever the text is shown, so the window that asks is provided once.
export function App({ owner = shellOwner, shell = inShell() }: AppProps) {
  return (
    <ExternalLinks owner={owner}>
      <Window owner={owner} shell={shell} />
    </ExternalLinks>
  )
}

function Window({ owner, shell }: { readonly owner: OwnerClient; readonly shell: boolean }) {
  const { state, loop } = useLoop(owner)
  const board = state.boards.find((each) => each.id === state.boardId)
  const missing = board !== undefined && isMissing(board)
  // A board whose folder is gone has nothing to read: its page is the missing page, and no view or task is asked for.
  const field = useField(owner, missing ? null : state.boardId)
  const [tab, setTab] = useState<TabId>('field')
  const [onLoopPage, setOnLoopPage] = useState(false)
  const [locating, setLocating] = useState(false)
  const pickFolder = shell ? async () => (await owner.rpc('shell.pick_folder')).path : null
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
    <Peek
      owner={owner}
      boardId={missing ? null : state.boardId}
      sources={field.data?.sources ?? NO_SOURCES}
      stamp={field.updatedAt}
    >
      <Frame
        tab={tab}
        onTab={setTab}
        boards={state.boards}
        boardId={state.boardId}
        folderProblems={state.folderProblems}
        onChooseBoard={loop.chooseBoard}
        onOpenFolder={loop.openFolder}
        pickFolder={pickFolder}
        onMenuOpen={loop.refreshBoards}
        onOpenLoop={() => setOnLoopPage(true)}
        viewPicker={
          board === undefined || missing ? null : (
            <ViewPicker
              boardName={boardName(board)}
              views={field.views}
              view={field.view}
              onChoose={field.select}
              shared={field.shared}
              onShare={field.shareView}
              onOpen={field.reloadViews}
            />
          )
        }
        field={
          board !== undefined && missing ? (
            <MissingBoard name={boardName(board)} folder={board.root} onLocate={() => setLocating(true)} />
          ) : (
            <FieldPage
              owner={owner}
              boardId={state.boardId}
              board={board === undefined ? null : { name: boardName(board), line: defaultLine(board) }}
              field={field}
            />
          )
        }
      />
      {locating && board !== undefined ? (
        <LocateWindow
          boardName={boardName(board)}
          {...(pickFolder === null ? {} : { pick: pickFolder })}
          onLocate={(path) => loop.locateBoard(board.id, path)}
          onClosed={() => setLocating(false)}
        />
      ) : null}
    </Peek>
  )
}
