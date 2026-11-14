// UI-level pilots: the same scenarios driven through rendered components, with keyboard and clicks.
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { exportSession } from '../src/export'
import { reduce } from '../src/reducer'
import { s8Target, S8_RUN } from '../src/scenarios'
import { App } from '../src/ui/App'
import { Provider, useApp } from '../src/ui/store'
import { initialState } from '../src/world'
import type { Action } from '../src/reducer'
import type { State } from '../src/types'

afterEach(cleanup)

let spy: { s: State; d: (a: Action) => void }
function Spy() {
  spy = useApp()
  return null
}

function mount(id: string) {
  let s = initialState('orchard', true)
  s = reduce(s, { type: 'loadScenario', id, t: 1000 })
  s = reduce(s, { type: 'startTimer', t: 2000 })
  render(
    <Provider initial={s} clock={false}>
      <App />
      <Spy />
    </Provider>,
  )
}

const key = (k: string, init: KeyboardEventInit = {}) => fireEvent.keyDown(window, { key: k, ...init })
const done = () => spy.s.session.attempts[spy.s.session.attempts.length - 1]?.detected

describe('UI pilots', () => {
  it('shows the task card without naming any control', () => {
    mount('2')
    expect(screen.getByRole('note', { name: 'Task card' }).textContent).toMatch(/Slow spots/)
  })

  it('S2: chips, then Save as view with a name', () => {
    mount('2')
    const chips = (group: string, name: string) => fireEvent.click(within(screen.getByRole('group', { name: group })).getByRole('button', { name }))
    chips('Collection', 'E-07')
    chips('Status', 'ready')
    chips('Label', 'perf')
    expect(screen.getByText('epic == "E-07" && status == "ready" && "perf" in labels')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Save as view' }))
    fireEvent.change(screen.getByLabelText('View name'), { target: { value: 'Slow spots' } })
    fireEvent.click(screen.getByRole('button', { name: 'Save' }))
    expect(done()).toBe(true)
  })

  it('S3: open the view, tick three, choose one group, launch', () => {
    mount('3')
    fireEvent.click(within(screen.getByRole('group', { name: 'Views' })).getByRole('button', { name: 'Ready in E-03' }))
    for (const id of ['T-03-011', 'T-03-014', 'T-03-019']) fireEvent.click(screen.getByLabelText(`Select ${id}`))
    fireEvent.click(screen.getByRole('button', { name: 'Launch 3' }))
    fireEvent.click(screen.getByLabelText('One group run'))
    fireEvent.click(within(screen.getByRole('region', { name: 'Launch preview' })).getByRole('button', { name: 'Launch' }))
    expect(done()).toBe(true)
  })

  it('S4: five decisions cleared with the keyboard', () => {
    mount('4')
    key('3')
    for (let i = 0; i < 5; i++) key('a')
    expect(done()).toBe(true)
    expect(screen.getByText('Nothing is waiting on you.')).toBeTruthy()
  })

  it('S9: context view by i, decide by key, say what was missing', () => {
    mount('9')
    key('3')
    key('i')
    expect(screen.getByRole('heading', { name: 'Why is this asked?' })).toBeTruthy()
    key('j')
    key('j')
    // the failed gate's context view cannot say what retry does or what happened before
    expect(screen.getAllByText(/Not available in this view/).length).toBeGreaterThan(0)
    key('k')
    key('k')
    key('d')
    key('a')
    key('r')
    expect(spy.s.queue.filter((q) => q.status === 'pending')).toHaveLength(0)
    for (const b of screen.getAllByRole('button', { name: 'Nothing missing' })) fireEvent.click(b)
    expect(done()).toBe(true)
  })

  it('S5: accept from the Review tab opened on the Field card', () => {
    mount('5')
    const card = document.querySelector('[data-task="T-05-009"]') as HTMLElement
    fireEvent.click(within(card).getByRole('button', { name: 'Review' }))
    fireEvent.click(screen.getByRole('button', { name: 'Accept' }))
    expect(done()).toBe(true)
  })

  it('S1: empty repo prompt, launch, run to the end, review in the Queue', () => {
    let s = initialState('empty', true)
    s = reduce(s, { type: 'loadScenario', id: '1', t: 1000 })
    s = reduce(s, { type: 'startTimer', t: 2000 })
    render(<Provider initial={s} clock={false}><App /><Spy /></Provider>)
    fireEvent.change(screen.getByLabelText('Describe a task'), { target: { value: 'Add a --quiet flag' } })
    fireEvent.click(screen.getByRole('button', { name: 'Launch' }))
    spy.d({ type: 'advance', seconds: 900 })
    key('3')
    key('a')
    expect(done()).toBe(true)
  })

  it('S8: breakdown filters set in the UI, then click the slowest bar', () => {
    mount('8')
    const t = s8Target(spy.s)!
    const card = document.querySelector('[data-task="T-07-003"]') as HTMLElement
    fireEvent.click(within(card).getByRole('button', { name: 'Output' }))
    expect(screen.getByLabelText('Agent output')).toBeTruthy()
    fireEvent.click(screen.getByRole('tab', { name: 'Time breakdown' }))
    fireEvent.change(screen.getByLabelText('Minimum duration in seconds'), { target: { value: '2' } })
    const clock = (v: number) => new Date(v * 1000).toISOString().slice(11, 19)
    const from = screen.getByLabelText('Window from')
    fireEvent.change(from, { target: { value: clock(t.win[0]) } })
    fireEvent.blur(from)
    const to = screen.getByLabelText('Window to')
    fireEvent.change(to, { target: { value: clock(t.win[1]) } })
    fireEvent.blur(to)
    expect(spy.s.detail?.bd.minDur).toBe(2)
    expect(spy.s.detail?.bd.win).toEqual(t.win)
    fireEvent.click(screen.getByRole('button', { name: /cargo test -p importer 7\.3 s/ }))
    expect(spy.s.detail?.runId).toBe(S8_RUN)
    expect(done()).toBe(true)
  })
})

describe('facilitator mode', () => {
  it('load, start, stop and export', () => {
    render(<Provider initial={initialState('orchard', true)} clock={false}><App /><Spy /></Provider>)
    fireEvent.change(screen.getByLabelText('Scenario'), { target: { value: '5' } })
    expect(screen.getByLabelText('Facilitator task card').textContent).toMatch(/T-05-009/)
    fireEvent.click(screen.getByRole('button', { name: /^Start/ }))
    fireEvent.keyDown(window, { code: 'KeyE', altKey: true })
    fireEvent.keyDown(window, { code: 'KeyS', altKey: true })
    expect(spy.s.session.attempts).toHaveLength(1)
    const out = exportSession(spy.s) as { attempts: { scenario: string; errors: number; stop: string; variants: object }[] }
    expect(out.attempts[0]).toMatchObject({ scenario: '5', errors: 1, stop: 'manual' })
    expect(out.attempts[0].variants).toMatchObject({ identity: 'id', keys: 'literal', open: 'moment', line: 'top' })
  })

  it('the variant switches change the interface', () => {
    render(<Provider initial={initialState('orchard', true)} clock={false}><App /><Spy /></Provider>)
    expect(screen.getByLabelText('Line')).toBeTruthy()
    fireEvent.change(screen.getByLabelText('Q4 line switcher'), { target: { value: 'filter' } })
    expect(screen.getByRole('group', { name: 'Line' })).toBeTruthy()
  })
})
