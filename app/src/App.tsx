import { useCallback, useState } from 'react'
import type { SourceDecl } from './field/types.ts'
import { Button } from '@/components/ui/button'
import { EmptyBoard } from './firstrun/EmptyBoard.tsx'
import { FieldPage } from './field/FieldPage.tsx'
import { FirstRunPage } from './firstrun/FirstRunPage.tsx'
import { GitInitWindow } from './firstrun/GitInitWindow.tsx'
import type { Check } from './firstrun/model.ts'
import { makeScratch } from './firstrun/scratch.ts'
import { ShareBanner, ShareWindow, shareNotice } from './firstrun/Share.tsx'
import { TrustBanner, TrustWindow } from './firstrun/TrustWindow.tsx'
import { useTrust } from './firstrun/useTrust.ts'
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
import { type OwnerClient, createTauriOwner, toOwnerError } from './owner/index.ts'
import { Peek } from './peek/Peek.tsx'
import { AddPackWindow } from './settings/AddPackWindow.tsx'
import { SettingsPage } from './settings/SettingsPage.tsx'

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

/** The folder the first-run page is looking at (`ts-0194-qp`): its checks while they are read, and what went wrong. */
interface FirstRun {
  readonly folder: string | null
  readonly checks: readonly Check[] | null
  readonly problem: string | null
  readonly busy: boolean
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
  const [first, setFirst] = useState<FirstRun | null>(null)
  const [makingRepo, setMakingRepo] = useState(false)
  const [sharing, setSharing] = useState(false)
  const [addingPack, setAddingPack] = useState(false)
  // The boards whose `.collide/` this window wrote and the person has not committed yet.
  const [written, setWritten] = useState<readonly string[]>([])
  const trust = useTrust(
    owner,
    missing ? undefined : board,
    useCallback(() => {
      void loop.refreshBoards()
      field.reload()
    }, [loop, field.reload]),
  )
  // A folder opened from the menu, and a folder typed or chosen on the page, are looked at before they are opened.
  const look = async (path: string) => {
    const folder = path.trim()
    if (folder === '') return
    setTab('field')
    setOnLoopPage(false)
    setFirst({ folder, checks: null, problem: null, busy: false })
    const mine = (now: FirstRun | null) => now !== null && now.folder === folder
    try {
      const { checks } = await owner.rpc('board.checks', { root: folder })
      setFirst((now) => (mine(now) ? { ...now!, checks } : now))
    } catch (failure) {
      const problem = toOwnerError(failure).message
      setFirst((now) => (mine(now) ? { ...now!, checks: [], problem } : now))
    }
  }
  const openFirst = async () => {
    if (first?.folder == null) return
    setFirst({ ...first, problem: null, busy: true })
    const outcome = await loop.openChecked(first.folder)
    setFirst(outcome.opened ? null : { ...first, problem: outcome.problems.join('\n'), busy: false })
  }
  const makeRepo = async (): Promise<string | null> => {
    if (first?.folder == null) return 'There is no folder to make a repository of.'
    const outcome = await loop.openChecked(first.folder, true)
    if (!outcome.opened) return outcome.problems.join('\n') || 'Collide could not make this folder a repository.'
    setFirst(null)
    return null
  }
  const noBoards = !state.loading && state.boards.length === 0
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
        onChooseBoard={(id) => {
          setFirst(null)
          loop.chooseBoard(id)
        }}
        onOpenFolder={look}
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
        settings={(back) => (
          <SettingsPage
            key={state.boardId ?? ''}
            owner={owner}
            boardId={state.boardId}
            boardName={board === undefined ? null : boardName(board)}
            sources={field.data?.sources ?? NO_SOURCES}
            onBack={back}
          />
        )}
        field={
          first !== null || noBoards ? (
            <FirstRunPage
              folder={first?.folder ?? null}
              checks={first?.checks ?? null}
              problem={first?.problem ?? null}
              busy={first?.busy ?? false}
              onFolder={(path) => void look(path)}
              pickFolder={pickFolder}
              onOpen={() => void openFirst()}
              onCheckAgain={() => first?.folder != null && void look(first.folder)}
              onMakeRepo={() => setMakingRepo(true)}
            />
          ) : state.loading ? (
            <p role="status" aria-label="Field" className="text-muted-foreground">
              Reading the boards…
            </p>
          ) : board !== undefined && missing ? (
            <MissingBoard name={boardName(board)} folder={board.root} onLocate={() => setLocating(true)} />
          ) : (
            <FieldPage
              owner={owner}
              boardId={state.boardId}
              board={board === undefined ? null : { name: boardName(board), line: defaultLine(board) }}
              field={field}
              notices={
                <>
                  <ShareBanner
                    notice={shareNotice({
                      sharedRead: field.sharedRead,
                      shared: field.shared,
                      written: state.boardId !== null && written.includes(state.boardId),
                    })}
                    onShare={() => setSharing(true)}
                  />
                  {trust.prompt === null ? null : <TrustBanner brought={trust.prompt.brought} onReview={trust.show} />}
                </>
              }
              empty={
                board === undefined ? undefined : (
                  <EmptyBoard
                    folder={board.root}
                    sources={field.data?.sources ?? NO_SOURCES}
                    onScratch={async (prompt) => {
                      const failed = await makeScratch(owner, board.id, prompt)
                      if (failed === null) field.reload()
                      return failed
                    }}
                    onAddPack={() => setAddingPack(true)}
                  />
                )
              }
            />
          )
        }
      />
      {makingRepo && first?.folder != null ? (
        <GitInitWindow folder={first.folder} onRun={makeRepo} onClosed={() => setMakingRepo(false)} />
      ) : null}
      {sharing && state.boardId !== null ? (
        <ShareWindow
          owner={owner}
          boardId={state.boardId}
          onWritten={() => {
            const id = state.boardId
            if (id !== null) setWritten((now) => [...now, id])
            field.reloadViews()
          }}
          onClosed={() => setSharing(false)}
        />
      ) : null}
      {addingPack ? (
        <AddPackWindow owner={owner} onAdded={field.reload} onClosed={() => setAddingPack(false)} />
      ) : null}
      {trust.open && trust.prompt !== null && board !== undefined ? (
        <TrustWindow
          boardName={boardName(board)}
          review={trust.prompt.review}
          brought={trust.prompt.brought}
          onTrust={trust.trust}
          onWithout={trust.without}
        />
      ) : null}
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
