import { renderToString } from 'react-dom/server'
import { describe, expect, test } from 'vitest'
import { AppShell } from '../src/AppShell.tsx'
import { PLACEHOLDER_DATA } from '../src/components/placeholders-detail.ts'
import { stubActions } from '../src/components/stub-actions.ts'
import { type UiAction, type UiState, applyAction, initialState } from '../src/ui/index.ts'

// The shell is wired to the reducer: what it shows is a function of the `UiState`, and what a person does is a
// `UiAction`. Click handling needs a browser; Stage 6's ported UI tests cover it.

const noop = () => {}

function applied(state: UiState, action: UiAction): UiState {
  const result = applyAction(state, action)
  expect(result.ok).toBe(true)
  return result.state
}

const loaded = () => {
  const corpus = {
    tasks: PLACEHOLDER_DATA.tasks.map((t) => t.key),
    runs: PLACEHOLDER_DATA.runs.map((r) => r.id),
    phases: ['build', 'review'],
  }
  return applied(initialState(), { type: 'user/corpus_loaded', corpus })
}

const show = (state: UiState) =>
  renderToString(<AppShell state={state} dispatch={noop} data={PLACEHOLDER_DATA} actions={stubActions} />)

describe('the shell shows the surface the state names', () => {
  test.each([
    ['field', 'Field'],
    ['tracks', 'Tracks'],
    ['queue', 'Queue'],
    ['experiments', 'Experiments'],
  ] as const)('%s', (surface, heading) => {
    const html = show(applied(loaded(), { type: 'user/surface_shown', surface }))
    expect(html).toContain(heading)
  })
})

test('a selected task is counted on Launch', () => {
  const state = applied(loaded(), { type: 'user/task_toggled', id: 'T-03-011' })
  expect(show(state)).toContain('Launch 1 task')
})

describe('an in-app agent call', () => {
  const agent = { kind: 'agent' as const, agent: 'claude', model: 'haiku' }
  const at = '2026-11-15T10:00:00Z'

  test('shows its group title and a chip that offers Undo', () => {
    const state = applied(loaded(), {
      type: 'agent/show_groups',
      source: agent,
      at,
      input: { surface: 'field', title: 'Tasks that touch the same files', groups: [{ name: 'Cache', ids: ['T-03-011'], reason: 'Same module' }] },
    })
    const html = show(state)
    expect(html).toContain('Tasks that touch the same files')
    expect(html).toContain('Undo')
  })

  test('opening a run on Tracks shows its detail', () => {
    const state = applied(
      applied(loaded(), { type: 'user/surface_shown', surface: 'tracks' }),
      { type: 'agent/open', source: agent, at, input: { surface: 'tracks', id: 'R-118' } },
    )
    expect(show(state)).toContain('Time breakdown')
  })

  test('without a call there is no chip and no Undo', () => {
    expect(show(loaded())).not.toContain('Undo')
  })
})

test('every stub action throws its own tag', () => {
  expect(() => stubActions.launch([])).toThrow('unimplemented: ui: launch')
  expect(() => stubActions.answerQueueItem('Q-1', 'stop')).toThrow('unimplemented: ui: queue answer')
  expect(() => stubActions.decideReview('R-1', 'accept')).toThrow('unimplemented: ui: review decision')
  expect(() => stubActions.openItem('tracks', 'R-1')).toThrow('unimplemented: ui: open item')
  expect(() => stubActions.compareRuns('R-1', 'R-2')).toThrow('unimplemented: ui: compare runs')
  expect(() => stubActions.startExperiment()).toThrow('unimplemented: ui: experiment start')
})
