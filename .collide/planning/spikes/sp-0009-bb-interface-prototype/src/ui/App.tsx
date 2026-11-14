import { useEffect, useRef } from 'react'
import { loadAgentSettings } from '../agent/page'
import { AgentBar, AskBox } from './AgentBar'
import { Settings } from './Settings'
import { queueOrder } from '../reducer'
import { scenarioById } from '../scenarios'
import { fmtClock } from '../world'
import { Compare } from './Compare'
import { Experiments } from './Experiments'
import { Facilitator } from './Facilitator'
import { Field, LineSwitcher } from './Field'
import { Queue } from './Queue'
import { RunDetail } from './RunDetail'
import { Tracks } from './Tracks'
import { Kbd } from './common'
import { useApp } from './store'
import type { Surface } from '../types'

const TABS: { id: Surface; key: string; label: string }[] = [
  { id: 'field', key: '1', label: 'Field' },
  { id: 'tracks', key: '2', label: 'Tracks' },
  { id: 'queue', key: '3', label: 'Queue' },
  { id: 'experiments', key: '4', label: 'Experiments' },
]

function useKeys(askRef: React.RefObject<HTMLInputElement | null>) {
  const { s, d } = useApp()
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const ss = s.session
      if (e.ctrlKey && e.shiftKey && e.code === 'KeyF') {
        e.preventDefault()
        d({ type: 'setFacilitator', on: !ss.facilitator })
        return
      }
      if (ss.facilitator && e.altKey && e.code === 'KeyE') {
        e.preventDefault()
        d({ type: 'addError' })
        return
      }
      if (ss.facilitator && e.altKey && e.code === 'KeyS') {
        e.preventDefault()
        d(ss.phase === 'running' ? { type: 'stopTimer' } : { type: 'startTimer' })
        return
      }
      const el = e.target as HTMLElement
      const typing = /^(INPUT|TEXTAREA|SELECT)$/.test(el.tagName) || el.isContentEditable
      if (e.key === 'Escape') {
        if (typing) el.blur()
        else if (s.armed) d({ type: 'disarm' })
        else if (s.surface === 'queue' && s.ctxOpen) d({ type: 'toggleCtx' })
        else d({ type: 'back' })
        return
      }
      if (typing || e.metaKey || e.ctrlKey || e.altKey) return
      if (e.key === '/') {
        e.preventDefault()
        askRef.current?.focus()
        return
      }
      if (e.key === '5') return d({ type: 'nav', surface: 'settings' })
      const tab = TABS.find((t) => t.key === e.key)
      if (tab) return d({ type: 'nav', surface: tab.id })
      if (s.surface === 'tracks') {
        if (e.key === '[') d({ type: 'pan', dir: -1 })
        else if (e.key === ']') d({ type: 'pan', dir: 1 })
        else if (e.key === '=' || e.key === '+') d({ type: 'zoom', dir: 1 })
        else if (e.key === '-') d({ type: 'zoom', dir: -1 })
        return
      }
      if (s.surface !== 'queue') return
      if (e.key === 'j') d({ type: 'queueMove', d: 1 })
      else if (e.key === 'k') d({ type: 'queueMove', d: -1 })
      else if (e.key === 'a' || e.key === 'd' || e.key === 'r') d({ type: 'answer', key: e.key })
      else if (e.key === 'Enter' && s.armed) d({ type: 'answer', key: s.armed.key })
      else if (e.key === 'o') d({ type: 'openTrack' })
      else if (e.key === 'i') d({ type: 'toggleCtx' })
      else if (e.key === 'u') d({ type: 'undo' })
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [s, d])
}

function TopBar({ askRef }: { askRef: React.RefObject<HTMLInputElement | null> }) {
  const { s, d } = useApp()
  const n = queueOrder(s).length
  return (
    <header className="top">
      <span className="brand">Collide</span>
      <span className="crumb mono">{s.repo === 'empty' ? 'new-repo' : 'orchard'}</span>
      {s.variants.line === 'top' && <LineSwitcher where="top" />}
      <nav className="tabs" aria-label="Surfaces">
        {TABS.map((t) => (
          <button key={t.id} type="button" aria-pressed={s.surface === t.id} onClick={() => d({ type: 'nav', surface: t.id })}>
            <Kbd>{t.key}</Kbd> {t.label}
            {t.id === 'queue' && <span className="badge" aria-label={`${n} waiting`}>{n}</span>}
          </button>
        ))}
      </nav>
      <AskBox ref={askRef} />
      <button type="button" className="btn sm" aria-pressed={s.surface === 'settings'} onClick={() => d({ type: 'nav', surface: 'settings' })}>
        <Kbd>5</Kbd> Agents
      </button>
      <span className="spacer" />
      <span className="clock mono" aria-label="Fake clock">{fmtClock(s.simNow)}</span>
      <label className="speed">Speed
        <select aria-label="Sim speed" value={s.speed} onChange={(e) => d({ type: 'setSpeed', speed: Number(e.target.value) })}>
          {[1, 5, 10, 30, 60].map((x) => <option key={x} value={x}>x{x}</option>)}
        </select>
      </label>
    </header>
  )
}

function CardStrip() {
  const { s } = useApp()
  const ss = s.session
  const sc = ss.scenarioId ? scenarioById(ss.scenarioId) : undefined
  if (!sc || ss.phase === 'idle' || ss.phase === 'done') return null
  return (
    <div className="cardstrip" role="note" aria-label="Task card">
      <b>Task</b> {sc.card(s)}
    </div>
  )
}

export function App() {
  const { s, d } = useApp()
  const askRef = useRef<HTMLInputElement | null>(null)
  useKeys(askRef)
  // the ask box needs to know which agent it uses
  useEffect(() => {
    if (!s.agentSettings.loaded) void loadAgentSettings(d)
  }, [s.agentSettings.loaded, d])
  return (
    <div className="shell">
    <div className="app" data-surface={s.surface}>
      <TopBar askRef={askRef} />
      <CardStrip />
      <AgentBar />
      <main className={`main${s.detail ? ' with-detail' : ''}`}>
        <section className="surface" aria-label={s.surface}>
          {s.surface === 'field' && <Field />}
          {s.surface === 'tracks' && <Tracks />}
          {s.surface === 'queue' && <Queue />}
          {s.surface === 'experiments' && <Experiments />}
          {s.surface === 'compare' && <Compare />}
          {s.surface === 'settings' && <Settings />}
        </section>
        <RunDetail />
      </main>
    </div>
    {s.session.facilitator && <Facilitator />}
    </div>
  )
}
