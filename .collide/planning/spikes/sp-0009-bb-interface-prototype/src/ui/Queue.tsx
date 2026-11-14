import { useState } from 'react'
import { ACTIONS, KIND_LABEL, KIND_ORDER, actFor } from '../items'
import { queueOrder } from '../reducer'
import { S9_RUNS } from '../scenarios'
import { fmtClock, fmtDur, runOf, taskOf } from '../world'
import { Kbd, RunTag, usd } from './common'
import { useApp } from './store'
import type { Ctx, QueueItem, Run, State } from '../types'

const WARM_SECONDS = 15 * 60

function MiniTrack({ run }: { run: Run }) {
  const total = run.script[run.script.length - 1]?.at || 1
  return (
    <div className="mini" aria-label="Mini track">
      {run.phases.map((p) => (
        <i
          key={p.key} className={`m ${p.end ? 'done' : `s-${run.status}`}`} title={p.reviewer ? `review ${p.reviewer}` : p.label}
          style={{ left: `${(((p.start ?? 0) - run.startWall) / (total + run.shift)) * 100}%`, width: `${Math.max(1, (((p.end ?? run.startWall + total) - (p.start ?? 0)) / (total + run.shift)) * 100)}%`, top: p.reviewer === 'B' ? '50%' : 0, height: p.reviewer ? '50%' : '100%' }}
        />
      ))}
    </div>
  )
}

function Section({ n, title, lines, missing, mono }: { n: number; title: string; lines?: string[]; missing?: string; mono?: boolean }) {
  return (
    <section className="ctxsec">
      <h3><span className="num">{n}</span> {title}</h3>
      {lines && lines.length > 0
        ? <ul>{lines.map((l, i) => <li key={i} className={mono ? 'mono' : ''}>{l}</li>)}</ul>
        : <p className="miss">Not available in this view: {missing ?? 'nothing recorded'}.</p>}
    </section>
  )
}

/** The six-part context view. Collide's facts come first and the agent's own words last. */
function ContextView({ it }: { it: QueueItem }) {
  const c: Ctx = it.ctx
  return (
    <div className="ctx" aria-label="Why is this asked?">
      <h2>Why is this asked?</h2>
      <Section n={1} title="Why it asks" lines={c.why} missing="the rule or check" />
      <Section n={2} title="What it would do" lines={[...(c.does ?? []), ...(c.flags ?? []).map((f) => `FLAG: ${f}`)]} missing="what the action would do" />
      {c.diff && <pre className="diff">{c.diff.join('\n')}</pre>}
      <Section n={3} title="The task" lines={c.task} missing="the task and its checklist" />
      <Section n={4} title="What led here" lines={c.led} missing="the phase's last calls and files changed" />
      <Section n={5} title="History" lines={c.history} missing="earlier answers to the same request" />
      <Section n={6} title="The agent's own words (unverified)" lines={c.words} missing="the call's description and last message" />
    </div>
  )
}

function Detail({ s, it }: { s: State; it: QueueItem }) {
  const { d } = useApp()
  const run = runOf(s, it.runId)
  const task = taskOf(s, run?.taskId ?? null)
  const age = s.simNow - it.raisedWall
  const acts = ACTIONS[s.variants.keys][it.kind]
  const armedKey = s.armed?.itemId === it.id ? s.armed.key : null
  const recent = run?.spans.filter((x) => x.start <= it.raisedWall && x.kind !== 'wait' && x.kind !== 'text').slice(-5) ?? []
  const live = it.kind !== 'review' && it.kind !== 'budget' && it.kind !== 'gate'
  return (
    <div className="qdetail">
      <header className={`dh k-${it.kind}`}>
        <span className="kind">{KIND_LABEL[it.kind]}</span>
        <h1>{it.title}</h1>
        <p className="note">
          {run && <RunTag s={s} run={run} />} {task ? `${task.id} ${task.title}` : run?.title}, phase {it.phase}.
          Blocking for {fmtDur(age)} since {fmtClock(it.raisedWall)}.
        </p>
      </header>

      {s.ctxOpen ? (
        <ContextView it={it} />
      ) : (
        <>
          <p>{it.why}</p>
          <section>
            <h3>{it.kind === 'edit' ? 'Target' : it.kind === 'gate' ? 'Gate command' : it.kind === 'review' ? 'Summary' : 'Exact command'}</h3>
            <pre className="cmd">{it.body}</pre>
            <p className="note">in <span className="mono">{it.cwd}</span></p>
            {it.ctx.flags?.map((f) => <p key={f} className="flag">Flagged: {f}</p>)}
          </section>
          {live && (
            <p className={`cache ${age < WARM_SECONDS ? 's-waiting' : 's-cold'}`}>
              {age < WARM_SECONDS
                ? `Prompt cache warm for ${fmtDur(WARM_SECONDS - age)}, then ${usd(1.3)} to resume.`
                : `Prompt cache is cold: ${usd(1.3)} to resume.`}
            </p>
          )}
          {run && <section><h3>Where this sits in {run.id}</h3><MiniTrack run={run} /><p className="note mono">{usd(run.spend)} of {usd(run.budget)}</p></section>}
          <section>
            <h3>Recent activity</h3>
            <ol className="act">{recent.map((x) => <li key={x.id}><span className="mono">{fmtClock(x.start)}</span> {x.tool} {x.input}</li>)}</ol>
          </section>
        </>
      )}

      <div className="abar" role="group" aria-label="Answer">
        {(['a', 'd', 'r'] as const).map((k) => {
          const act = acts[k]
          if (!act) return null
          return (
            <button key={k} type="button" className={`btn${k === 'a' ? ' go' : k === 'd' ? ' deny' : ''}${armedKey === k ? ' armed' : ''}`} title={act.hint} onClick={() => d({ type: 'answer', key: k })}>
              <Kbd>{k}</Kbd> {armedKey === k ? `Confirm: ${act.label}` : act.label}
            </button>
          )
        })}
        <button type="button" className="btn" onClick={() => d({ type: 'openTrack' })}><Kbd>o</Kbd> Open track</button>
        <button type="button" className="btn" aria-pressed={s.ctxOpen} onClick={() => d({ type: 'toggleCtx' })}><Kbd>i</Kbd> Why is this asked?</button>
        {it.kind === 'review' && run && <button type="button" className="btn" onClick={() => d({ type: 'selectRun', runId: run.id, tab: 'review' })}>Open review</button>}
      </div>
      {armedKey && <p className="armnote" role="status">Press <Kbd>{armedKey}</Kbd> or Enter again to confirm: {actFor(s.variants.keys, it.kind, armedKey)?.hint}</p>}
    </div>
  )
}

function MissingNotes() {
  const { s, d } = useApp()
  if (s.session.scenarioId !== '9') return null
  const items = s.queue.filter((q) => S9_RUNS.includes(q.runId))
  return (
    <section className="missing" aria-label="What was missing">
      <h3>What was missing?</h3>
      {items.map((q) => <MissingRow key={q.id} q={q} saved={s.session.missing[q.id]} save={(text) => d({ type: 'missingNote', itemId: q.id, text })} />)}
    </section>
  )
}

function MissingRow({ q, saved, save }: { q: QueueItem; saved?: string; save: (t: string) => void }) {
  const [text, setText] = useState('')
  return (
    <div className="mrow">
      <span className="mono">{q.kind}</span>
      {saved !== undefined ? <span className="note">saved: {saved}</span> : (
        <>
          <input aria-label={`Missing for ${q.title}`} value={text} placeholder="what you wanted to know" onChange={(e) => setText(e.target.value)} />
          <button type="button" className="btn sm" disabled={!text.trim()} onClick={() => save(text.trim())}>Save</button>
          <button type="button" className="btn sm" onClick={() => save('nothing missing')}>Nothing missing</button>
        </>
      )}
    </div>
  )
}

export function Queue() {
  const { s, d } = useApp()
  const order = queueOrder(s)
  const sel = order.find((q) => q.id === s.queueSel) ?? order[0]
  const last = s.lastAnswer ? s.queue.find((q) => q.id === s.lastAnswer!.itemId) : undefined
  return (
    <div className="queue">
      <nav className="qlist" aria-label="Decisions waiting on you, longest blocking first">
        {KIND_ORDER.map((k) => {
          const items = order.filter((q) => q.kind === k)
          if (!items.length) return null
          return (
            <section key={k}>
              <h3>{KIND_LABEL[k]} <span className="count">{items.length}</span></h3>
              <ul>
                {items.map((q) => {
                  const run = runOf(s, q.runId)
                  return (
                    <li key={q.id}>
                      <button
                        type="button" className={`qitem${sel?.id === q.id ? ' selected' : ''}${run && s.hover === run.id ? ' hl' : ''}`}
                        onClick={() => d({ type: 'queueSelect', id: q.id })}
                        onMouseEnter={() => d({ type: 'hover', runId: q.runId })} onMouseLeave={() => d({ type: 'hover', runId: null })}
                      >
                        <span className="t">{q.title}</span>
                        <span className="mono note">{run && <RunTag s={s} run={run} />} {fmtDur(s.simNow - q.raisedWall)}</span>
                      </button>
                    </li>
                  )
                })}
              </ul>
            </section>
          )
        })}
        {!order.length && <p className="note pad">Nothing is waiting on you.</p>}
        <MissingNotes />
      </nav>
      <div className="qpane">
        {sel ? <Detail s={s} it={sel} /> : <p className="note pad">Select an item.</p>}
        {last && last.answer && (
          <div className="toast" role="status">
            {last.title}: {last.answer.effect}.{' '}
            <button type="button" className="btn sm" onClick={() => d({ type: 'undo' })}><Kbd>u</Kbd> Undo</button>
          </div>
        )}
      </div>
    </div>
  )
}
