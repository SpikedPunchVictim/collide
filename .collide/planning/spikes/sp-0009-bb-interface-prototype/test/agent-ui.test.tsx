// The in-app agent through rendered components: chips with Undo and save, the lens board with reasons, the Agents page.
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { agentDemoState, PANTRY_GROUPS } from '../src/agentDemo'
import { reduce } from '../src/reducer'
import { defaultSettings } from '../server/settings'
import { App } from '../src/ui/App'
import { Provider, useApp } from '../src/ui/store'
import type { Action } from '../src/reducer'
import type { State } from '../src/types'

afterEach(cleanup)

let spy: { s: State; d: (a: Action) => void }
function Spy() {
  spy = useApp()
  return null
}

function mount() {
  const s = reduce(agentDemoState(), { type: 'settingsLoaded', settings: defaultSettings() })
  render(
    <Provider initial={s} clock={false}>
      <App />
      <Spy />
    </Provider>,
  )
}

const dispatch = (a: Action) => act(() => spy.d(a))
const chips = () => screen.queryAllByRole('listitem').filter((li) => li.classList.contains('agentchip'))

describe('agent chips', () => {
  it('a sort of the runs shows a chip with its expression, and Undo takes both away', () => {
    mount()
    dispatch({ type: 'agentSetView', surface: 'tracks', title: 'Longest build phase', source: 'test', sort: { by: 'phase:build', dir: 'desc' } })
    expect(screen.getByText('Agent: Longest build phase')).toBeTruthy()
    expect(screen.getAllByText('true | sort phase_duration("build") desc').length).toBeGreaterThan(0)
    fireEvent.click(screen.getByRole('button', { name: 'Undo' }))
    expect(chips()).toHaveLength(0)
    expect(spy.s.tracksView.sort).toBeNull()
  })

  it('Save as view asks for a name and the chip then says it was saved', () => {
    mount()
    dispatch({ type: 'agentSetView', surface: 'tracks', title: 'Longest build phase', source: 'test', sort: { by: 'phase:build', dir: 'desc' } })
    fireEvent.click(screen.getByRole('button', { name: 'Save as view' }))
    fireEvent.change(screen.getByLabelText('View name'), { target: { value: 'Slow builds' } })
    fireEvent.click(screen.getByRole('button', { name: 'Save' }))
    expect(screen.getByText(/view saved as/).textContent).toContain('Slow builds')
    expect(spy.s.views.some((v) => v.name === 'Slow builds')).toBe(true)
  })
})

describe('lens on the Field', () => {
  const groups = Object.entries(PANTRY_GROUPS).map(([name, ids]) => ({ name, ids, reason: `these change the same ${name} code` }))
  const show = () => dispatch({ type: 'agentShowGroups', surface: 'field', title: 'Same-file groups', source: 'test', groups, rest: 'dim' })

  it('names every group and shows its reason on the board and in the chip', () => {
    mount()
    show()
    for (const g of groups) expect(screen.getAllByText(new RegExp(g.reason)).length).toBeGreaterThanOrEqual(2)
    expect(screen.getByText('Agent: Same-file groups')).toBeTruthy()
  })

  it('Save snapshot keeps the groups under a name, Undo of the lens removes the board', () => {
    mount()
    show()
    fireEvent.click(screen.getByRole('button', { name: 'Save snapshot' }))
    fireEvent.change(screen.getByLabelText('Snapshot name'), { target: { value: 'Pantry groups' } })
    fireEvent.click(screen.getByRole('button', { name: 'Save' }))
    expect(screen.getByText(/snapshot saved as/).textContent).toContain('Pantry groups')
    expect(spy.s.lens).not.toBeNull()
    fireEvent.click(screen.getByRole('button', { name: 'Undo' }))
    expect(spy.s.lens).toBeNull()
  })
})

describe('Agents page', () => {
  it('shows the endpoint and the copy-paste snippets for an external agent, as text', () => {
    mount()
    fireEvent.click(screen.getByRole('button', { name: /Agents/ }))
    const card = screen.getByRole('article', { name: 'My own agent' })
    expect(within(card).getByText(/claude mcp add --transport http collide http:\/\/.*\/mcp/)).toBeTruthy()
    expect(within(card).getByText(/"mcpServers"/)).toBeTruthy()
    expect(within(card).getByText(/"remote"/)).toBeTruthy()
  })

  it('an edit marks the settings unsaved and the ask box names the chosen agent', () => {
    mount()
    expect(screen.getByText('Claude Code (haiku)')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: /Agents/ }))
    const card = screen.getByRole('article', { name: 'Claude Code (haiku)' })
    fireEvent.change(within(card).getByLabelText('Model'), { target: { value: 'sonnet' } })
    expect(spy.s.agentSettings.dirty).toBe(true)
    expect(spy.s.agentSettings.settings?.agents[0].model).toBe('sonnet')
  })
})
