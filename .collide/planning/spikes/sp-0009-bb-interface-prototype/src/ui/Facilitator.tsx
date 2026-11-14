import { useState } from 'react'
import { exportSession } from '../export'
import { SCENARIOS, scenarioById } from '../scenarios'
import { Kbd } from './common'
import { useApp } from './store'
import type { Variants } from '../types'

const QUESTIONS: { key: keyof Variants; label: string; options: { v: string; text: string }[] }[] = [
  {
    key: 'identity', label: 'Q1 run identity',
    options: [
      { v: 'id', text: 'by id (R-118)' }, { v: 'lane', text: 'by lane (lane B)' },
      { v: 'position', text: 'by position (#2)' }, { v: 'colour', text: 'by colour (control; breaks "colour is state")' },
    ],
  },
  {
    key: 'keys', label: 'Q2 a and d per kind',
    options: [
      { v: 'literal', text: 'literal: a yes, d no or destroy' }, { v: 'safe', text: 'safe: a retry or accept, d hold or send back' },
      { v: 'confirm', text: 'confirm: literal, consequential keys need a second press' },
    ],
  },
  {
    key: 'open', label: 'Q3 o from a Queue item',
    options: [
      { v: 'moment', text: 'track, zoomed on the moment of the decision' }, { v: 'track', text: 'track at the usual zoom' },
      { v: 'output', text: 'output pane at the last call before the ask' },
    ],
  },
  { key: 'line', label: 'Q4 line switcher', options: [{ v: 'top', text: 'top bar menu' }, { v: 'filter', text: 'filter bar chips' }] },
]

function fmtTimer(ms: number): string {
  const t = Math.max(0, Math.floor(ms / 100)) / 10
  return `${Math.floor(t / 60)}:${(t % 60).toFixed(1).padStart(4, '0')}`
}

export function Facilitator() {
  const { s, d } = useApp()
  const [open, setOpen] = useState(true)
  const [saved, setSaved] = useState('')
  const ss = s.session
  const sc = ss.scenarioId ? scenarioById(ss.scenarioId) : undefined
  const elapsed = ss.phase === 'running' && ss.startedAt !== null ? s.realNow - ss.startedAt : ss.attempts[ss.attempts.length - 1]?.ms ?? 0
  const json = () => JSON.stringify(exportSession(s), null, 2)

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(json())
      setSaved('copied to the clipboard')
    } catch {
      setSaved('copy failed, use Download')
    }
  }
  const download = () => {
    const a = document.createElement('a')
    a.href = URL.createObjectURL(new Blob([json()], { type: 'application/json' }))
    a.download = `s6-session-${ss.participant || 'p'}-${new Date().toISOString().slice(0, 19).replace(/[:T]/g, '-')}.json`
    a.click()
    setSaved('downloaded')
  }
  const post = async () => {
    try {
      const r = await fetch('/__session', { method: 'POST', body: json() })
      const j = await r.json()
      setSaved(`saved ${j.saved}`)
    } catch (e) {
      setSaved(`save failed: ${String(e)}`)
    }
  }

  if (!open) return <button type="button" className="facbtn" onClick={() => setOpen(true)}>Facilitator</button>

  return (
    <aside className="fac" aria-label="Facilitator">
      <header>
        <h2>Facilitator</h2>
        <span className="spacer" />
        <button type="button" className="btn sm" onClick={() => setOpen(false)}>Hide</button>
      </header>
      <label className="row">Participant <input value={ss.participant} onChange={(e) => d({ type: 'setParticipant', name: e.target.value })} placeholder="p1" /></label>
      <label className="row">Scenario
        <select value={ss.scenarioId ?? ''} onChange={(e) => e.target.value && d({ type: 'loadScenario', id: e.target.value })}>
          <option value="">choose</option>
          {SCENARIOS.map((x) => <option key={x.id} value={x.id}>{x.id} {x.title}</option>)}
        </select>
      </label>
      {sc && <blockquote className="card-text" aria-label="Facilitator task card">{sc.card(s)}</blockquote>}
      <div className="row">
        <button type="button" className="btn sm" disabled={!ss.scenarioId} onClick={() => ss.scenarioId && d({ type: 'loadScenario', id: ss.scenarioId })}>Reload</button>
        {ss.phase === 'running'
          ? <button type="button" className="btn sm danger" onClick={() => d({ type: 'stopTimer' })}>Stop <Kbd>Alt S</Kbd></button>
          : <button type="button" className="btn sm primary" disabled={ss.phase !== 'loaded'} onClick={() => d({ type: 'startTimer' })}>Start <Kbd>Alt S</Kbd></button>}
        <button type="button" className="btn sm" disabled={ss.phase !== 'running'} onClick={() => d({ type: 'addError' })}>Error +1 <Kbd>Alt E</Kbd></button>
        <span className="timer mono" aria-label="Timer">{fmtTimer(elapsed)}</span>
      </div>
      <p className="note" role="status">
        {ss.phase === 'idle' && 'No scenario loaded.'}
        {ss.phase === 'loaded' && 'Loaded. Read the card to the participant, then Start.'}
        {ss.phase === 'running' && `Running. Errors ${ss.errors}.`}
        {ss.phase === 'done' && (ss.detected ? 'End state detected, timer stopped.' : 'Stopped by hand.')}
      </p>
      {sc && !sc.detect && ss.phase === 'running' && (
        <div className="row">
          This probe has no detector:
          <button type="button" className="btn sm" onClick={() => d({ type: 'stopTimer', correct: true })}>Stop, correct</button>
          <button type="button" className="btn sm" onClick={() => d({ type: 'stopTimer', correct: false })}>Stop, wrong</button>
        </div>
      )}
      {ss.autoFlags.length > 0 && <p className="note">Flags: {ss.autoFlags.join('; ')}</p>}
      {ss.errorLog.length > 0 && <ul className="note">{ss.errorLog.map((e, i) => <li key={i}>{e.auto ? 'auto: ' : ''}{e.note || 'error'}</li>)}</ul>}
      <label className="row stack">Notes
        <textarea rows={2} value={ss.notes} onChange={(e) => d({ type: 'setNotes', text: e.target.value })} />
      </label>

      <h3>Variants for the open questions</h3>
      {QUESTIONS.map((q) => (
        <label className="row stack" key={q.key}>{q.label}
          <select value={s.variants[q.key]} onChange={(e) => d({ type: 'setVariant', key: q.key, value: e.target.value })}>
            {q.options.map((o) => <option key={o.v} value={o.v}>{o.text}</option>)}
          </select>
        </label>
      ))}
      <div className="row">
        <label>Speed
          <select value={s.speed} onChange={(e) => d({ type: 'setSpeed', speed: Number(e.target.value) })}>
            {[1, 5, 10, 30, 60].map((x) => <option key={x} value={x}>x{x}</option>)}
          </select>
        </label>
        <label><input type="checkbox" checked={s.paused} onChange={(e) => d({ type: 'setPaused', paused: e.target.checked })} /> pause sim</label>
      </div>

      <h3>Attempts ({ss.attempts.length})</h3>
      <table className="att">
        <thead><tr><th>Sc</th><th>Time</th><th>Err</th><th>Stop</th></tr></thead>
        <tbody>
          {ss.attempts.map((a, i) => <tr key={i}><td>{a.scenario}</td><td className="mono">{fmtTimer(a.ms)}</td><td>{a.errors}</td><td>{a.stop}</td></tr>)}
        </tbody>
      </table>
      <div className="row">
        <button type="button" className="btn sm" onClick={copy}>Copy JSON</button>
        <button type="button" className="btn sm" onClick={download}>Download</button>
        <button type="button" className="btn sm" onClick={post}>Save to out/sessions</button>
      </div>
      {saved && <p className="note" role="status">{saved}</p>}
    </aside>
  )
}
