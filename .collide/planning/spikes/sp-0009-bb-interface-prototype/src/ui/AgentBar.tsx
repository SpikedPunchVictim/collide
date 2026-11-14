// The ask box, the live line of an ask, and the chips for what the agent changed.
import { forwardRef, useRef, useState } from 'react'
import { runAsk } from '../agent/page'
import { useApp } from './store'
import type { AgentChange, AskState } from '../types'

const ms = (a: number | null, b: number): string => (a === null ? 'none' : `${((a - b) / 1000).toFixed(1)} s`)

export const AskBox = forwardRef<HTMLInputElement>(function AskBox(_, ref) {
  const { s, d } = useApp()
  const [text, setText] = useState('')
  const ac = useRef<AbortController | null>(null)
  const st = s.agentSettings.settings
  const agent = st?.agents.find((a) => a.id === st.askAgent)
  const working = s.agent.ask.status === 'working'
  const submit = () => {
    const request = text.trim()
    if (!request || working) return
    d({ type: 'askStart', request, agent: agent?.name ?? 'the configured agent' })
    ac.current = new AbortController()
    void runAsk(request, st?.askAgent ?? null, d, ac.current.signal)
    setText('')
  }
  return (
    <form className="ask" onSubmit={(e) => { e.preventDefault(); submit() }}>
      <input
        ref={ref} aria-label="Ask the agent" placeholder="Ask the agent to change this view" value={text} disabled={working}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => { if (e.key === 'Escape') (e.target as HTMLInputElement).blur() }}
      />
      {working
        ? <button type="button" className="btn sm" onClick={() => ac.current?.abort()}>Cancel</button>
        : <button type="submit" className="btn sm primary" disabled={!text.trim()}>Ask</button>}
      <span className="note askwho" title="Change in Agents (key 5)">{agent ? agent.name : 'no agent set'}</span>
    </form>
  )
})

function AskStatus({ ask }: { ask: AskState }) {
  const { d } = useApp()
  const label = {
    working: 'Working', done: 'Done', failed: 'Failed', nochange: 'No change made', idle: '',
  }[ask.status]
  const last = ask.lines.slice(-4)
  return (
    <section className={`askstatus a-${ask.status}`} role="status" aria-label="Agent request">
      <div className="asktop">
        <b>{label}</b>
        <span className="note ell2">{ask.agent}: &ldquo;{ask.request}&rdquo;</span>
        <span className="spacer" />
        {ask.status !== 'working' && <button type="button" className="btn sm" onClick={() => d({ type: 'askDismiss' })}>Dismiss</button>}
      </div>
      {ask.status === 'failed' && <p className="bad askerr">{ask.error}{ask.text ? ` The agent said: ${ask.text}` : ''}</p>}
      {ask.status === 'nochange' && (
        <p className="askerr">
          The agent finished without changing the screen{ask.calls ? ` (${ask.calls} tool call${ask.calls === 1 ? '' : 's'}, none changed anything)` : ' (no tool calls)'}.
          {ask.text ? ` It said: ${ask.text}` : ''}
        </p>
      )}
      {ask.status === 'done' && ask.text && <p className="asktext">{ask.text}</p>}
      <ul className="asklines mono">
        {last.map((l, i) => <li key={`${l.t}-${i}`}>{l.text}</li>)}
        {ask.status === 'working' && !last.length && <li>starting the agent...</li>}
      </ul>
      {ask.status !== 'working' && (
        <p className="note asktimes">
          first tool call {ms(ask.firstToolAt, ask.startedAt)}, first change {ms(ask.chipAt, ask.startedAt)}, finished {ms(ask.doneAt, ask.startedAt)}
          {ask.usage?.costUsd != null && `, $${ask.usage.costUsd.toFixed(3)}`}
          {ask.usage?.outputTokens != null && `, ${ask.usage.inputTokens ?? '?'} in / ${ask.usage.outputTokens} out tokens`}
        </p>
      )}
    </section>
  )
}

function Chip({ c, last }: { c: AgentChange; last: boolean }) {
  const { s, d } = useApp()
  const [naming, setNaming] = useState(false)
  const [name, setName] = useState('')
  const lens = c.tool === 'show_groups'
  const canSave = lens ? s.lens?.id === c.lensId : !!c.view
  const save = () => {
    d(lens ? { type: 'agentSaveSnapshot', changeId: c.id, name } : { type: 'agentSaveView', changeId: c.id, name })
    setNaming(false)
    setName('')
  }
  return (
    <li className="agentchip" data-change={c.id}>
      <div className="chiphead">
        <span className="chiptitle">Agent: {c.title}</span>
        <span className="note">{c.source}, {new Date(c.at).toLocaleTimeString()}</span>
        <span className="spacer" />
        {last && <button type="button" className="btn sm" onClick={() => d({ type: 'agentUndo' })}>Undo</button>}
        {c.saved
          ? <span className="note">{lens ? 'snapshot' : 'view'} saved as &ldquo;{c.saved}&rdquo;</span>
          : naming
            ? (
              <form className="saveform" onSubmit={(e) => { e.preventDefault(); save() }}>
                <input aria-label={lens ? 'Snapshot name' : 'View name'} autoFocus placeholder={lens ? 'Snapshot name' : 'View name'} value={name} onChange={(e) => setName(e.target.value)} />
                <button type="submit" className="btn sm primary" disabled={!name.trim()}>Save</button>
                <button type="button" className="btn sm" onClick={() => setNaming(false)}>Cancel</button>
              </form>
            )
            : canSave && c.tool !== 'open' && c.tool !== 'highlight' && (
              <button type="button" className="btn sm" onClick={() => setNaming(true)}>{lens ? 'Save snapshot' : 'Save as view'}</button>
            )}
      </div>
      {c.expr && <code className="chipexpr">{c.expr}</code>}
      {c.note && <p className="chipnote">{c.note}</p>}
      {c.reasons.length > 0 && (
        <ul className="chipreasons">
          {c.reasons.map((r, i) => <li key={i}>{r.name && <b>{r.name} ({r.n}): </b>}{r.reason}</li>)}
        </ul>
      )}
    </li>
  )
}

export function AgentBar() {
  const { s } = useApp()
  const ask = s.agent.ask
  const changes = s.agent.changes
  if (ask.status === 'idle' && !changes.length) return null
  return (
    <div className="agentbar">
      {ask.status !== 'idle' && <AskStatus ask={ask} />}
      {changes.length > 0 && (
        <ul className="chips" aria-label="Agent changes">
          {changes.slice(-4).map((c) => <Chip key={c.id} c={c} last={c.id === changes[changes.length - 1].id} />)}
        </ul>
      )}
    </div>
  )
}
