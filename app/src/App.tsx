import { useReducer } from 'react'
import { AppShell } from './AppShell.tsx'
import { PLACEHOLDER_DATA, type ShellData } from './components/placeholders-detail.ts'
import { type Actions, stubActions } from './components/stub-actions.ts'
import { type UiState, initialState, reducer } from './ui/index.ts'

/** The state the window starts in: the corpus the placeholder data holds, so the reducer knows its ids (0.9). */
function startState(data: ShellData): UiState {
  return reducer(initialState(), {
    type: 'user/corpus_loaded',
    corpus: {
      tasks: data.tasks.map((t) => t.key),
      runs: data.runs.map((r) => r.id),
      phases: [...new Set(data.runs.flatMap((r) => r.phases.map((p) => p.name)))],
    },
  })
}

export interface AppProps {
  /** What the surfaces show. Placeholders until the owner client supplies it (Stage 3c and Stage 6). */
  readonly data?: ShellData
  readonly actions?: Actions
}

export function App({ data = PLACEHOLDER_DATA, actions = stubActions }: AppProps) {
  const [state, dispatch] = useReducer(reducer, data, startState)
  return <AppShell state={state} dispatch={dispatch} data={data} actions={actions} />
}
