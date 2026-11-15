import { describe, expect, test } from 'vitest'
import {
  type ActionResult,
  type ActionSource,
  type ActionType,
  type UiAction,
  type UiState,
  applyAction,
  canUndo,
  initialState,
  reducer,
} from '../src/ui/index.ts'

// The reducer is plain functions over plain data, so these tests need no browser and no React (0.15: reducer tests
// need no browser). Agent tool calls arrive as untrusted input (0.9), so an agent action's `input` is written here as
// the raw object an agent might send, not as a typed value.

const agent: ActionSource = { kind: 'agent', agent: 'claude', model: 'haiku' }
const at = '2026-11-15T10:00:00Z'
const corpus = {
  tasks: ['T-01', 'T-02', 'T-03', 'T-04'],
  runs: ['R-1', 'R-2', 'R-3'],
  phases: ['build', 'review'],
}

function call(
  type: 'agent/query' | 'agent/set_view' | 'agent/show_groups' | 'agent/highlight' | 'agent/open',
  input: unknown,
): UiAction {
  return { type, source: agent, at, input }
}

/** A state that knows the corpus, as it is once the window has loaded it. */
function loaded(): UiState {
  return applyAction(initialState(), { type: 'user/corpus_loaded', corpus }).state
}

function applied(state: UiState, action: UiAction): UiState {
  const result = applyAction(state, action)
  expect(result.ok, JSON.stringify(result)).toBe(true)
  return result.state
}

const setView = { surface: 'field', title: 'Open tasks', filter: { status: ['open'] }, sort: { by: 'id', dir: 'asc' } }
const showGroups = {
  surface: 'field',
  title: 'By area',
  groups: [
    { name: 'Core', ids: ['T-01', 'T-02'], reason: 'both touch the engine' },
    { name: 'UI', ids: ['T-03'], reason: 'window only' },
  ],
  rest: 'dim',
}
const highlight = { surface: 'tracks', ids: ['R-1', 'R-3'], note: 'slowest builds' }

describe('driving the reducer without a browser', () => {
  test('a user action changes the screen', () => {
    const next = applied(loaded(), { type: 'user/surface_shown', surface: 'tracks' })
    expect(next.screen.surface).toBe('tracks')
  })

  test('an agent tool call changes the screen and leaves a chip', () => {
    const next = applied(loaded(), call('agent/set_view', setView))
    expect(next.screen.field.filter).toEqual({ status: ['open'] })
    expect(next.screen.field.sort).toEqual({ by: 'id', dir: 'asc' })
    expect(next.chips).toHaveLength(1)
    expect(next.chips[0]).toMatchObject({ tool: 'set_view', source: agent, at, undone: false })
  })

  test('the reducer function the React tree will use returns the next state', () => {
    const before = loaded()
    expect(reducer(before, call('agent/open', { surface: 'tracks' })).screen.surface).toBe('tracks')
    expect(reducer(before, call('agent/open', { surface: 'x' }))).toBe(before)
  })
})

describe('Undo restores exactly the state a tool call replaced', () => {
  const cases: Array<[string, UiAction]> = [
    ['set_view', call('agent/set_view', setView)],
    ['show_groups', call('agent/show_groups', showGroups)],
    ['highlight', call('agent/highlight', highlight)],
    ['open', call('agent/open', { surface: 'tracks', id: 'R-2' })],
  ]

  test.each(cases)('after %s, Undo restores the screen as it was', (_name, action) => {
    const before = loaded()
    const snapshot = structuredClone(before.screen)
    const changed = applied(before, action)
    expect(changed.screen).not.toEqual(snapshot)
    const restored = applied(changed, { type: 'user/undo' })
    expect(restored.screen).toEqual(snapshot)
    expect(restored.undo).toBeNull()
  })

  test.each(cases)('after %s, a second Undo does nothing more', (_name, action) => {
    const changed = applied(loaded(), action)
    const once = applied(changed, { type: 'user/undo' })
    const twice = applyAction(once, { type: 'user/undo' })
    expect(twice.state).toBe(once)
    expect(twice.ok).toBe(true)
  })

  test('Undo undoes the last change only', () => {
    const first = applied(loaded(), call('agent/set_view', setView))
    const second = applied(first, call('agent/highlight', { surface: 'field', ids: ['T-01'] }))
    const undone = applied(second, { type: 'user/undo' })
    expect(undone.screen).toEqual(first.screen)
    expect(undone.screen.field.filter).toEqual({ status: ['open'] })
    expect(applied(undone, { type: 'user/undo' }).screen).toEqual(first.screen)
  })

  test('Undo marks the chip as undone and keeps it', () => {
    const undone = applied(applied(loaded(), call('agent/highlight', highlight)), { type: 'user/undo' })
    expect(undone.chips).toHaveLength(1)
    expect(undone.chips[0]?.undone).toBe(true)
  })

  test('a call that changes nothing keeps the Undo of the call before it', () => {
    const first = applied(loaded(), call('agent/set_view', setView))
    const again = applyAction(first, call('agent/set_view', setView))
    expect(again).toMatchObject({ ok: true, changed: false })
    expect(again.state).toBe(first)
    expect(applied(again.state, { type: 'user/undo' }).screen.field.filter).toEqual({})
  })

  describe("a person's own edit of the view ends the Undo of an agent change", () => {
    const edits: Array<[string, UiAction]> = [
      ['user/surface_shown', { type: 'user/surface_shown', surface: 'tracks' }],
      ['user/filter_set', { type: 'user/filter_set', surface: 'tracks', filter: { status: ['failed'] } }],
    ]

    test.each(edits)('after %s, Undo leaves the edit in place and changes nothing', (_name, edit) => {
      const changed = applied(loaded(), call('agent/set_view', setView))
      const edited = applied(changed, edit)
      expect(edited.screen).not.toEqual(changed.screen)
      const result = applyAction(edited, { type: 'user/undo' })
      expect(result).toMatchObject({ ok: true, changed: false })
      expect(result.state).toBe(edited)
    })

    test('after user/lens_cleared, Undo leaves the edit in place and changes nothing', () => {
      const changed = applied(loaded(), call('agent/show_groups', showGroups))
      const edited = applied(changed, { type: 'user/lens_cleared', surface: 'field' })
      expect(edited.screen.lens.field).toBeNull()
      const result = applyAction(edited, { type: 'user/undo' })
      expect(result).toMatchObject({ ok: true, changed: false })
      expect(result.state).toBe(edited)
      expect(result.state.screen.lens.field).toBeNull()
    })

    test('a screen edit drops the snapshot', () => {
      const changed = applied(loaded(), call('agent/set_view', setView))
      expect(applied(changed, { type: 'user/surface_shown', surface: 'tracks' }).undo).toBeNull()
    })

    test('the chip of the expired change is no longer undoable, and is not marked undone', () => {
      const changed = applied(loaded(), call('agent/set_view', setView))
      const chipId = changed.chips[0]?.id ?? ''
      expect(canUndo(changed, chipId)).toBe(true)
      const edited = applied(changed, { type: 'user/surface_shown', surface: 'tracks' })
      expect(canUndo(edited, chipId)).toBe(false)
      expect(edited.chips[0]?.undone).toBe(false)
    })

    test('a user action that leaves the screen equal keeps the Undo', () => {
      const changed = applied(loaded(), call('agent/set_view', setView))
      const noop = applyAction(changed, { type: 'user/surface_shown', surface: changed.screen.surface })
      expect(noop).toMatchObject({ ok: true, changed: false })
      expect(canUndo(noop.state, changed.chips[0]?.id ?? '')).toBe(true)
      expect(applied(noop.state, { type: 'user/undo' }).screen.field.filter).toEqual({})
    })

    test('a no-op filter_set and lens_cleared keep the Undo', () => {
      const changed = applied(loaded(), call('agent/set_view', setView))
      const sameFilter = applyAction(changed, { type: 'user/filter_set', surface: 'field', filter: { status: ['open'] } })
      expect(sameFilter).toMatchObject({ ok: true, changed: false })
      const noLens = applyAction(sameFilter.state, { type: 'user/lens_cleared', surface: 'field' })
      expect(noLens).toMatchObject({ ok: true, changed: false })
      expect(applied(noLens.state, { type: 'user/undo' }).screen.field.filter).toEqual({})
    })

    test('selecting a task keeps the Undo', () => {
      const changed = applied(loaded(), call('agent/set_view', setView))
      const selected = applied(changed, { type: 'user/task_toggled', id: 'T-01' })
      expect(canUndo(selected, changed.chips[0]?.id ?? '')).toBe(true)
      expect(applied(selected, { type: 'user/undo' }).screen.field.filter).toEqual({})
    })

    test('loading the corpus keeps the Undo', () => {
      const changed = applied(loaded(), call('agent/set_view', setView))
      const reloaded = applied(changed, { type: 'user/corpus_loaded', corpus })
      expect(canUndo(reloaded, changed.chips[0]?.id ?? '')).toBe(true)
      expect(applied(reloaded, { type: 'user/undo' }).screen.field.filter).toEqual({})
    })

    test('a new agent change after the expiry is undoable again', () => {
      const first = applied(loaded(), call('agent/set_view', setView))
      const edited = applied(first, { type: 'user/surface_shown', surface: 'tracks' })
      const second = applied(edited, call('agent/highlight', highlight))
      const secondChip = second.chips[1]?.id ?? ''
      expect(canUndo(second, secondChip)).toBe(true)
      expect(canUndo(second, first.chips[0]?.id ?? '')).toBe(false)
      const undone = applied(second, { type: 'user/undo' })
      expect(undone.screen).toEqual(edited.screen)
      expect(undone.chips[1]?.undone).toBe(true)
    })
  })

  test('Undo does not reach the corpus or the tasks a person selected', () => {
    const selected = applied(loaded(), { type: 'user/task_toggled', id: 'T-02' })
    const undone = applied(applied(selected, call('agent/set_view', setView)), { type: 'user/undo' })
    expect(undone.corpus).toEqual(corpus)
    expect(undone.selectedTasks).toEqual(['T-02'])
  })
})

describe('query', () => {
  test('leaves the state equal to before, with no chip and no snapshot', () => {
    const before = applied(loaded(), call('agent/set_view', setView))
    const result = applyAction(before, call('agent/query', { subject: 'tasks', filter: { ids: ['T-01'] }, limit: 10 }))
    expect(result).toMatchObject({ ok: true, changed: false })
    expect(result.state).toBe(before)
  })

  test('is refused for a bad subject or a limit out of range, and changes nothing', () => {
    const before = loaded()
    for (const input of [{ subject: 'files' }, { subject: 'tasks', limit: 0 }, { subject: 'runs', limit: 501 }]) {
      const result = applyAction(before, call('agent/query', input))
      expect(result.ok).toBe(false)
      expect(result.state).toBe(before)
    }
  })
})

describe('every action type has a reducer branch', () => {
  // `satisfies Record<ActionType, ...>` makes the compiler demand one sample per type in the union, so a new action
  // type fails `pnpm typecheck` here too. Running each shows that its branch exists.
  const samples = {
    'user/corpus_loaded': { type: 'user/corpus_loaded', corpus },
    'user/surface_shown': { type: 'user/surface_shown', surface: 'queue' },
    'user/task_toggled': { type: 'user/task_toggled', id: 'T-01' },
    'user/filter_set': { type: 'user/filter_set', surface: 'field', filter: {} },
    'user/lens_cleared': { type: 'user/lens_cleared', surface: 'field' },
    'user/undo': { type: 'user/undo' },
    'agent/query': call('agent/query', { subject: 'tasks' }),
    'agent/set_view': call('agent/set_view', setView),
    'agent/show_groups': call('agent/show_groups', showGroups),
    'agent/highlight': call('agent/highlight', highlight),
    'agent/open': call('agent/open', { surface: 'tracks' }),
  } satisfies Record<ActionType, UiAction>

  test.each(Object.entries(samples))('%s', (_type, action) => {
    const result: ActionResult = applyAction(loaded(), action)
    expect(result.ok, JSON.stringify(result)).toBe(true)
  })

  test('an action type the union does not have is refused as a value, not thrown', () => {
    const before = loaded()
    const result = applyAction(before, { type: 'agent/launch' } as never)
    expect(result).toMatchObject({ ok: false, error: { code: 'unknown_action' } })
    expect(result.state).toBe(before)
  })
})

describe('a rejected payload leaves the state unchanged', () => {
  const rejected: Array<[string, UiAction, string, string]> = [
    ['an unknown surface', call('agent/set_view', { ...setView, surface: 'x' }), 'invalid_input', 'unknown surface "x" for set_view; supported: field, tracks'],
    ['a surface a tool does not take', call('agent/show_groups', { ...showGroups, surface: 'queue' }), 'invalid_input', 'supported: field, tracks'],
    ['an unknown key', call('agent/highlight', { ...highlight, path: '/etc' }), 'invalid_input', 'unknown key "path"'],
    ['input that is not an object', call('agent/open', 'tracks'), 'invalid_input', 'object'],
    ['an invented task id', call('agent/highlight', { surface: 'field', ids: ['T-01', 'T-20'] }), 'unknown_id', 'unknown task id T-20; nothing was shown. Call query for real ids.'],
    ['an invented run id', call('agent/open', { surface: 'tracks', id: 'R-9' }), 'unknown_id', 'R-9'],
    ['a task id on Tracks', call('agent/highlight', { surface: 'tracks', ids: ['T-01'] }), 'unknown_id', 'T-01'],
    ['an invented id in a filter', call('agent/set_view', { ...setView, filter: { ids: ['T-20'] } }), 'unknown_id', 'T-20'],
    ['an id in two groups', call('agent/show_groups', { ...showGroups, groups: [{ name: 'A', ids: ['T-01'], reason: 'r' }, { name: 'B', ids: ['T-01'], reason: 'r' }] }), 'duplicate_id', 'T-01'],
    ['no groups', call('agent/show_groups', { ...showGroups, groups: [] }), 'invalid_input', '1 to 12'],
    ['thirteen groups', call('agent/show_groups', { ...showGroups, groups: Array.from({ length: 13 }, (_, i) => ({ name: `g${i}`, ids: [], reason: 'r' })) }), 'invalid_input', '1 to 12'],
    ['a group with no ids', call('agent/show_groups', { ...showGroups, groups: [{ name: 'A', ids: [], reason: 'r' }] }), 'invalid_input', 'at least one id'],
    ['a group with an empty reason', call('agent/show_groups', { ...showGroups, groups: [{ name: 'A', ids: ['T-01'], reason: ' ' }] }), 'invalid_input', 'reason'],
    ['a rest that is not dim or hide', call('agent/show_groups', { ...showGroups, rest: 'delete' }), 'invalid_input', 'dim, hide'],
    ['a Field sort key Tracks has', call('agent/set_view', { ...setView, sort: { by: 'spend', dir: 'asc' } }), 'invalid_input', 'id, title, epic, labels, status'],
    ['an unknown phase in a sort', call('agent/set_view', { surface: 'tracks', title: 't', sort: { by: 'phase:deploy', dir: 'desc' } }), 'invalid_input', 'phase "deploy"; known: build, review'],
    ['a sort direction that is not asc or desc', call('agent/set_view', { ...setView, sort: { by: 'id', dir: 'up' } }), 'invalid_input', 'asc, desc'],
    ['group_by on Tracks', call('agent/set_view', { surface: 'tracks', title: 't', group_by: 'epic' }), 'invalid_input', 'group_by'],
    ['an unknown group_by', call('agent/set_view', { ...setView, group_by: 'colour' }), 'invalid_input', 'status, epic, label, source'],
    ['a window on the Field', call('agent/set_view', { ...setView, window: { from: '09:15', to: '10:00' } }), 'invalid_input', 'window'],
    ['a malformed clock window', call('agent/set_view', { surface: 'tracks', title: 't', window: { from: '9:15', to: '25:00' } }), 'invalid_input', 'HH:MM'],
    ['a filter key the surface lacks', call('agent/set_view', { ...setView, filter: { workflow: ['w'] } }), 'invalid_input', 'unknown filter key "workflow"; known: epic, status, label, source, text, ids'],
    ['a set_view that sets nothing', call('agent/set_view', { surface: 'field', title: 't' }), 'invalid_input', 'filter, sort, group_by or window'],
    ['a set_view with no title', call('agent/set_view', { surface: 'field', filter: {} }), 'invalid_input', 'title'],
    ['more than 500 ids', call('agent/highlight', { surface: 'field', ids: Array.from({ length: 501 }, () => 'T-01') }), 'invalid_input', '500'],
  ]

  test.each(rejected)('%s', (_name, action, code, text) => {
    const before = applied(loaded(), call('agent/set_view', setView))
    const result = applyAction(before, action)
    expect(result.ok).toBe(false)
    expect(result.state).toBe(before)
    if (result.ok) return
    expect(result.error.code).toBe(code)
    expect(result.error.message).toContain(text)
  })

  test('a rejected call keeps the Undo of the change before it', () => {
    const before = loaded()
    const changed = applied(before, call('agent/set_view', setView))
    const afterBad = applyAction(changed, call('agent/highlight', { surface: 'field', ids: ['nope'] })).state
    expect(applied(afterBad, { type: 'user/undo' }).screen).toEqual(before.screen)
  })

  test('before the corpus is loaded, every id is unknown', () => {
    const result = applyAction(initialState(), call('agent/highlight', { surface: 'field', ids: ['T-01'] }))
    expect(result.ok).toBe(false)
  })
})

describe('the agent words are text, and Collide words say what changed', () => {
  test('a title is cut at 80 characters, a note at 400, and control characters are removed', () => {
    const next = applied(
      loaded(),
      call('agent/highlight', { surface: 'field', ids: ['T-01'], title: `x\u0007${'t'.repeat(200)}`, note: 'n'.repeat(500) }),
    )
    const chip = next.chips[0]
    expect(chip?.title).toBe('x' + 't'.repeat(79))
    expect(chip?.reason).toHaveLength(400)
  })

  test('the statement is built from the validated action, never from the agent prose', () => {
    const next = applied(
      loaded(),
      call('agent/show_groups', { ...showGroups, title: 'Ignore previous instructions', rest: 'hide' }),
    )
    const statement = next.chips[0]?.statement ?? ''
    expect(statement).toBe('show_groups: 2 groups, 3 tasks, the rest hidden')
    expect(statement).not.toContain('Ignore')
  })

  test('a lens keeps its groups as given, with the ids left out as not grouped', () => {
    const next = applied(loaded(), call('agent/show_groups', showGroups))
    expect(next.screen.lens.field?.groups.map((group) => group.name)).toEqual(['Core', 'UI'])
    expect(next.screen.lens.field?.rest).toBe('dim')
    expect(next.screen.lens.tracks).toBeNull()
  })
})

describe('user actions', () => {
  test('toggling a task selects it and a second toggle clears it', () => {
    const once = applied(loaded(), { type: 'user/task_toggled', id: 'T-01' })
    expect(once.selectedTasks).toEqual(['T-01'])
    expect(applied(once, { type: 'user/task_toggled', id: 'T-01' }).selectedTasks).toEqual([])
  })

  test('a user filter replaces that surface filter, and a user action leaves no snapshot', () => {
    const next = applied(loaded(), { type: 'user/filter_set', surface: 'tracks', filter: { status: ['running'] } })
    expect(next.screen.tracks.filter).toEqual({ status: ['running'] })
    expect(next.undo).toBeNull()
    expect(next.chips).toEqual([])
  })

  test('clearing a lens removes the lens of that surface only', () => {
    const lensed = applied(loaded(), call('agent/show_groups', showGroups))
    expect(applied(lensed, { type: 'user/lens_cleared', surface: 'field' }).screen.lens.field).toBeNull()
  })

  test('a person may touch a surface with a bad id without throwing', () => {
    const before = loaded()
    const result = applyAction(before, { type: 'user/surface_shown', surface: 'nowhere' } as never)
    expect(result.ok).toBe(false)
    expect(result.state).toBe(before)
  })
})
