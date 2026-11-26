// The window's frame (Stage 6a; ts-0179-ke): the top bar with the trail (the board menu, then a place for the view
// picker) and the Settings button, the five tabs with their subtitles and keys, and the page of the tab, or the
// Settings page the button opens. It is a function of its props, so a test renders it for any state with no browser;
// `App.tsx` holds the state. The loop page is not part of it: `onOpenLoop` asks `App` to show that page until Stage
// 9a replaces it.
import { type ReactNode, useCallback, useEffect, useState } from 'react'
import { Button } from '@/components/ui/button'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import type { BoardSummary } from '../loop/types.ts'
import { BoardMenu } from './BoardMenu.tsx'
import { LaterPage } from './LaterPage.tsx'
import { TracksPreview } from './TracksPreview.tsx'
import { actionForKey, keyOf, tabOfAction } from './actions.ts'
import { TABS, type TabId, isTab, isTyping } from './tabs.ts'

export interface FrameProps {
  readonly tab: TabId
  readonly onTab: (tab: TabId) => void
  readonly boards: readonly BoardSummary[]
  /** The board the surfaces read. */
  readonly boardId: string | null
  /** Why the last folder was not opened. */
  readonly folderProblems: readonly string[]
  readonly onChooseBoard: (id: string) => void
  readonly onOpenFolder: (path: string) => Promise<void>
  /** The system's folder picker, which only the shell has (`ts-0191-nv`). */
  readonly pickFolder?: (() => Promise<string | null>) | null
  /** The board menu was opened (`ts-0191-nv`): the boards are read again. */
  readonly onMenuOpen?: () => void
  readonly onOpenLoop: () => void
  /** The view picker, in its place in the trail after the board menu (`ts-0181-ft`). Nothing shows until given. */
  readonly viewPicker?: ReactNode
  /** The page of the Field tab. */
  readonly field: ReactNode
  /** The page of the Planning tab (`ts-0195-ju`). */
  readonly planning: ReactNode
  /** The Settings page (`ts-0193-gk`), given the way back to the board. */
  readonly settings: (back: () => void) => ReactNode
}

export function Frame(props: FrameProps) {
  const { tab, onTab, onOpenLoop } = props
  const [settings, setSettings] = useState(false)
  // Showing a tab, by a click or a key, also leaves Settings, as the mock does.
  const show = useCallback(
    (next: TabId) => {
      setSettings(false)
      onTab(next)
    },
    [onTab],
  )
  useTabKeys(show)
  return (
    <Tabs
      value={settings ? '' : tab}
      onValueChange={(next) => isTab(next) && show(next)}
      className="min-h-screen gap-0"
    >
      <header className="flex flex-wrap items-center gap-4 border-b bg-card px-4 py-2.5">
        <span className="flex items-center gap-2 text-base font-bold">
          <Logo />
          Collide
        </span>
        <nav aria-label="Board, line and view" className="flex items-center gap-1">
          <BoardMenu
            boards={props.boards}
            boardId={props.boardId}
            problems={props.folderProblems}
            onChoose={props.onChooseBoard}
            onOpenFolder={props.onOpenFolder}
            pickFolder={props.pickFolder}
            onMenuOpen={props.onMenuOpen}
          />
          {props.viewPicker == null ? null : (
            <>
              <span className="text-muted-foreground">/</span>
              {props.viewPicker}
            </>
          )}
        </nav>
        <TabsList variant="line" aria-label="Surfaces" className="mx-auto h-auto group-data-horizontal/tabs:h-auto">
          {TABS.map((entry) => (
            <TabsTrigger
              key={entry.id}
              value={entry.id}
              title={`${entry.name}: ${entry.subtitle} (key ${keyOf(`show-${entry.id}`)})`}
              aria-keyshortcuts={keyOf(`show-${entry.id}`)}
              className={
                'h-auto flex-none flex-col items-start justify-start gap-0 px-3 py-1.5 leading-tight ' +
                (entry.id === 'queue' ? 'opacity-60 data-active:opacity-100' : '')
              }
            >
              <span className="font-semibold">{entry.name}</span>
              <small className="text-xs font-normal text-muted-foreground">{entry.subtitle}</small>
            </TabsTrigger>
          ))}
        </TabsList>
        <div className="flex items-center gap-2">
          <Button variant="ghost" size="sm" aria-pressed={settings} onClick={() => setSettings((open) => !open)}>
            <Gear />
            Settings
          </Button>
        </div>
      </header>
      <main className="flex w-full flex-col gap-3 p-4">
        {settings ? (
          props.settings(() => setSettings(false))
        ) : (
          <TabsContent value={tab} className="flex flex-col gap-3">
            <Page tab={tab} field={props.field} planning={props.planning} onOpenLoop={onOpenLoop} />
          </TabsContent>
        )}
      </main>
    </Tabs>
  )
}

/** The keys of the tabs (`actions.ts`) show them, except while a text box has the focus. */
function useTabKeys(show: (tab: TabId) => void) {
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.metaKey || event.ctrlKey || event.altKey || isTyping(event.target as HTMLElement | null)) return
      const next = tabOfAction(actionForKey(event.key, 'window'))
      if (next !== null) show(next)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [show])
}

interface PageProps {
  readonly tab: TabId
  readonly field: ReactNode
  readonly planning: ReactNode
  readonly onOpenLoop: () => void
}

function Page({ tab, field, planning, onOpenLoop }: PageProps) {
  switch (tab) {
    case 'field':
      return field
    case 'tracks':
      return <TracksPreview onOpenLoop={onOpenLoop} />
    case 'queue':
      return (
        <LaterPage
          title="Queue"
          what="Every decision waiting on you, longest wait first."
          arrives={
            'It arrives in Stage 9a, with the run UI. ' +
            'Until then, the Loop page shows a run and what it needs from you.'
          }
          onOpenLoop={onOpenLoop}
        />
      )
    case 'planning':
      return planning
    case 'comms':
      return (
        <LaterPage
          title="Comms"
          what="Sessions with an agent that are not a task or a plan."
          arrives="It arrives in Stage 9b, with the run UI."
        />
      )
  }
}

function Logo() {
  return (
    <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="size-5">
      <circle cx="9" cy="12" r="5" />
      <circle cx="15" cy="12" r="5" />
    </svg>
  )
}

function Gear() {
  return (
    <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="size-4">
      <circle cx="12" cy="12" r="3" />
      <path d="M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9 7 7M17 17l2.1 2.1M4.9 19.1 7 17M17 7l2.1-2.1" />
    </svg>
  )
}
