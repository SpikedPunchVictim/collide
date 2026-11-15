// The window's shell, wired to the reducer of 0.15 (Pass 1): what it shows is a function of `UiState`, and what a
// person does that the reducer knows (show a surface, select a task, clear a lens, Undo) is a `UiAction` sent to
// `dispatch`. The rest is a stub from `actions` until its stage (see `stub-actions.ts`). It takes `state` and `dispatch`
// rather than owning them, so a test renders it for any state without a browser.
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { ExperimentsSurface } from './components/experiments/ExperimentsSurface.tsx'
import { FieldSurface } from './components/field/FieldSurface.tsx'
import type { ShellData } from './components/placeholders-detail.ts'
import { QueueSurface } from './components/queue/QueueSurface.tsx'
import { RunDetail } from './components/run/RunDetail.tsx'
import type { Actions } from './components/stub-actions.ts'
import { TracksSurface } from './components/tracks/TracksSurface.tsx'
import { SURFACES, type SurfaceId, type UiAction, type UiState, canUndo } from './ui/index.ts'

export interface AppShellProps {
  readonly state: UiState
  readonly dispatch: (action: UiAction) => void
  readonly data: ShellData
  readonly actions: Actions
}

/** The surfaces' names (0.11). Plainer names are the owner's call, asked with the approachable mockup pass. */
const SURFACE_WORDS: Record<SurfaceId, string> = {
  field: 'Field',
  tracks: 'Tracks',
  queue: 'Queue',
  experiments: 'Experiments',
}

function isSurface(value: string): value is SurfaceId {
  return (SURFACES as readonly string[]).includes(value)
}

export function AppShell(props: AppShellProps) {
  const { state, dispatch } = props
  return (
    <div className="mx-auto flex min-h-screen max-w-6xl flex-col gap-4 p-4">
      <header className="flex flex-wrap items-center justify-between gap-2">
        <h1 className="text-2xl font-semibold">Collide</h1>
        <Tabs
          value={state.screen.surface}
          onValueChange={(surface) => {
            if (isSurface(surface)) dispatch({ type: 'user/surface_shown', surface })
          }}
        >
          <TabsList aria-label="Surfaces">
            {SURFACES.map((surface) => (
              <TabsTrigger key={surface} value={surface}>
                {SURFACE_WORDS[surface]}
              </TabsTrigger>
            ))}
          </TabsList>
        </Tabs>
      </header>
      <Chips state={state} dispatch={dispatch} />
      <main className="flex flex-col gap-4">
        <Surface {...props} />
      </main>
    </div>
  )
}

function Surface({ state, dispatch, data, actions }: AppShellProps) {
  const { screen } = state
  switch (screen.surface) {
    case 'field':
      return (
        <FieldSurface
          tasks={data.tasks}
          selected={state.selectedTasks}
          lens={screen.lens.field}
          highlight={screen.highlight.field}
          onToggleTask={(id) => dispatch({ type: 'user/task_toggled', id })}
          onLaunch={actions.launch}
          onClearLens={() => dispatch({ type: 'user/lens_cleared', surface: 'field' })}
        />
      )
    case 'tracks': {
      const open = data.runs.find((r) => r.id === screen.selection.tracks)
      const finished = data.runs.filter((r) => r.state.state === 'ended')
      return (
        <>
          <TracksSurface
            runs={data.runs}
            lens={screen.lens.tracks}
            highlight={screen.highlight.tracks}
            onOpenRun={(id) => actions.openItem('tracks', id)}
            onCompare={actions.compareRuns}
            onClearLens={() => dispatch({ type: 'user/lens_cleared', surface: 'tracks' })}
            compare={finished[0] && finished[1] ? [finished[0], finished[1]] : null}
          />
          {open ? (
            <RunDetail run={open} entries={data.output} breakdown={data.breakdown} review={data.review} onDecide={actions.decideReview} />
          ) : null}
        </>
      )
    }
    case 'queue':
      return (
        <QueueSurface
          items={data.queue}
          selected={screen.selection.queue}
          context={data.context}
          nowMs={data.nowMs}
          onSelect={(id) => actions.openItem('queue', id)}
          onAnswer={actions.answerQueueItem}
        />
      )
    case 'experiments':
      return <ExperimentsSurface experiment={data.experiment} onStart={actions.startExperiment} />
  }
}

/** The agent's changes to the screen, each as a chip in Collide's own words, with Undo while it can still be undone. */
function Chips({ state, dispatch }: Pick<AppShellProps, 'state' | 'dispatch'>) {
  if (state.chips.length === 0) return null
  return (
    <ul aria-label="Changes the agent made" className="flex flex-col gap-1">
      {state.chips.map((chip) => (
        <li key={chip.id} className="flex flex-wrap items-center gap-2 rounded-lg border bg-muted/40 px-3 py-2 text-sm">
          <span>{chip.statement}</span>
          {chip.undone ? <Badge variant="outline">Undone</Badge> : null}
          {chip.title ? <span className="text-muted-foreground">{chip.title}</span> : null}
          {canUndo(state, chip.id) ? (
            <Button variant="ghost" size="sm" onClick={() => dispatch({ type: 'user/undo' })}>
              Undo
            </Button>
          ) : null}
        </li>
      ))}
    </ul>
  )
}
