import { type ReactNode, useCallback, useEffect, useMemo, useState } from 'react'
import type { SourceDecl } from './field/types.ts'
import { Button } from '@/components/ui/button'
import { DefaultsBanner, ResetNotice, ResetWindow } from './defaults/DefaultsBanner.tsx'
import {
  StateStoreProvider,
  clearStoredState,
  defaultsFromHash,
  hashForNormalStart,
  hashWithoutReset,
  resetFolderFromHash,
} from './defaults/mode.ts'
import { EmptyBoard } from './firstrun/EmptyBoard.tsx'
import { FieldPage } from './field/FieldPage.tsx'
import { memoryStore } from './field/folds.ts'
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
import { PlanningPage } from './planning/PlanningPage.tsx'
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
  /**
   * Whether the window started with defaults (`ts-0215-gt`): the shell reloaded it with `defaults` in the fragment of
   * its address. The default asks the page. In defaults mode the window tells the shell before it asks the daemon
   * anything, reads and writes no stored state, and shows a banner.
   */
  readonly defaults?: boolean
}

const inShell = () => typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
const currentHash = () => (typeof window === 'undefined' ? '' : window.location.hash)
const startedWithDefaults = () => defaultsFromHash(currentHash())

/** Reloads the page with the fragment `hash` and no other change to its address. */
function reloadWith(hash: string) {
  window.history.replaceState(null, '', `${window.location.pathname}${window.location.search}${hash}`)
  window.location.reload()
}

// A link in a task's text asks before it opens, wherever the text is shown, so the window that asks is provided once.
export function App({ owner = shellOwner, shell = inShell(), defaults = startedWithDefaults() }: AppProps) {
  return (
    <ExternalLinks owner={owner}>
      {defaults ? (
        <DefaultsMode owner={owner}>
          <Window owner={owner} shell={shell} defaults />
        </DefaultsMode>
      ) : (
        <Window owner={owner} shell={shell} defaults={false} />
      )}
    </ExternalLinks>
  )
}

/**
 * Tells the shell the window is in defaults mode before the window asks the daemon for anything, so the shell's
 * connection is made with the flag, and gives the window a store that lives as long as the page. A shell that cannot be
 * told is said, and the window is not shown: it would read the person's files after saying it did not.
 */
function DefaultsMode({ owner, children }: { readonly owner: OwnerClient; readonly children: ReactNode }) {
  const [told, setTold] = useState<true | string | null>(null)
  const store = useMemo(() => memoryStore(), [])
  useEffect(() => {
    let live = true
    owner.rpc('shell.set_defaults', { defaults: true }).then(
      () => live && setTold(true),
      (failure: unknown) => live && setTold(toOwnerError(failure).message),
    )
    return () => {
      live = false
    }
  }, [owner])
  if (told === null) return null
  if (told !== true) {
    return (
      <div className="mx-auto flex max-w-3xl flex-col gap-3 p-4">
        <p role="alert" className="whitespace-pre-wrap text-destructive">
          Could not start with defaults: {told}
        </p>
        <div>
          <Button variant="outline" size="sm" onClick={() => reloadWith(hashForNormalStart(currentHash(), null))}>
            Start normally
          </Button>
        </div>
      </div>
    )
  }
  return <StateStoreProvider value={store}>{children}</StateStoreProvider>
}

/** The folder the first-run page is looking at (`ts-0194-qp`): its checks while they are read, and what went wrong. */
interface FirstRun {
  readonly folder: string | null
  readonly checks: readonly Check[] | null
  readonly problem: string | null
  readonly busy: boolean
}

function Window({
  owner,
  shell,
  defaults,
}: {
  readonly owner: OwnerClient
  readonly shell: boolean
  readonly defaults: boolean
}) {
  const { state, loop } = useLoop(owner, defaults)
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
  const [resetting, setResetting] = useState(false)
  const [leaveProblem, setLeaveProblem] = useState<string | null>(null)
  // The folder a Reset moved the files to, which the fragment of this start names until the person dismisses it.
  const [resetFolder, setResetFolder] = useState(() => resetFolderFromHash(currentHash()))
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
  // Leaves defaults mode: the shell is told first, so the connection the next start makes is a normal one.
  const startNormally = async (folder: string | null): Promise<string | null> => {
    try {
      await owner.rpc('shell.set_defaults', { defaults: false })
    } catch (failure) {
      return toOwnerError(failure).message
    }
    reloadWith(hashForNormalStart(currentHash(), folder))
    return null
  }
  const reset = async (): Promise<string | null> => {
    try {
      const { folder } = await owner.rpc('config.reset', state.boardId === null ? {} : { board: state.boardId })
      // The files are moved. The stored state is cleared next, then the window starts normally and names the folder.
      try {
        clearStoredState(window.localStorage)
      } catch {
        // Storage that cannot be used has no state of ours to clear.
      }
      return (await startNormally(folder)) ?? null
    } catch (failure) {
      return toOwnerError(failure).message
    }
  }
  const top: ReactNode = defaults ? (
    <div className="flex flex-col gap-2 px-4 pt-4">
      <DefaultsBanner
        onNormal={() => void startNormally(null).then(setLeaveProblem)}
        onReset={() => setResetting(true)}
        canReset={!state.loading}
      />
      {leaveProblem === null ? null : (
        <p role="alert" className="whitespace-pre-wrap text-destructive">
          {leaveProblem}
        </p>
      )}
    </div>
  ) : resetFolder === null ? null : (
    <div className="px-4 pt-4">
      <ResetNotice
        folder={resetFolder}
        onDismiss={() => {
          window.history.replaceState(
            null,
            '',
            `${window.location.pathname}${window.location.search}${hashWithoutReset(currentHash())}`,
          )
          setResetFolder(null)
        }}
      />
    </div>
  )
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
      {top}
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
        planning={
          <PlanningPage
            owner={owner}
            boardId={missing ? null : state.boardId}
            board={board === undefined ? null : { name: boardName(board), line: defaultLine(board) }}
            field={field}
          />
        }
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
      {resetting ? (
        <ResetWindow
          boardName={board === undefined ? null : boardName(board)}
          onConfirm={reset}
          onClosed={() => setResetting(false)}
        />
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
