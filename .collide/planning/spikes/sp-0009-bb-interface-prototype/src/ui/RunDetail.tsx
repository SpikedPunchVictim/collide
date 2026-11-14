import { Fragment, useEffect, useRef, useState } from 'react'
import { breakdown, durOf, filterSpans, groupOf, type Row } from '../breakdown'
import { fmtClock, fmtDur, parseClock, runOf, taskOf } from '../world'
import { Chip, Kbd, RunTag } from './common'
import { useApp } from './store'
import type { Detail, DetailTab, Run, Span, State } from '../types'

function PhaseTabs({ run, d: det }: { run: Run; d: Detail }) {
  const { d } = useApp()
  const dispatch = (phase: string | null) => d({ type: 'setBd', patch: { phase, win: null } })
  return (
    <div className="ptabs" role="group" aria-label="Phase">
      <button type="button" className="fchip" aria-pressed={!det.bd.phase} onClick={() => dispatch(null)}>all phases</button>
      {run.phases.map((p) => (
        <button key={p.key} type="button" className="fchip" aria-pressed={det.bd.phase === p.key} onClick={() => dispatch(p.key)}>
          {p.reviewer ? `review ${p.reviewer}` : p.label}
        </button>
      ))}
    </div>
  )
}

function SpanRow({ s, sp, focus, depth }: { s: State; sp: Span; focus: boolean; depth: number }) {
  const { d } = useApp()
  const running = sp.end === null || sp.end > s.simNow
  if (sp.kind === 'text') {
    return (
      <li id={`call-${sp.id}`} className={`orow text${focus ? ' focus' : ''}`} data-focused={focus || undefined}>
        <span className="mono tm">{fmtClock(sp.start)}</span>
        <span className="say"><b>agent says</b> (unverified): {sp.input}</span>
      </li>
    )
  }
  if (sp.kind === 'wait') {
    const item = sp.itemId ? s.queue.find((q) => q.id === sp.itemId) : undefined
    return (
      <li id={`call-${sp.id}`} className={`orow wait${focus ? ' focus' : ''}`} data-focused={focus || undefined}>
        <span className="mono tm">{fmtClock(sp.start)}</span>
        <span className="tool">{sp.tool}</span>
        <span className="inp">{sp.input}</span>
        <span className="mono dur">{sp.end === null ? `${fmtDur(s.simNow - sp.start)}, open` : fmtDur(sp.end - sp.start)}</span>
        {item && item.status === 'pending' && (
          <button type="button" className="btn sm" onClick={() => { d({ type: 'nav', surface: 'queue' }); d({ type: 'queueSelect', id: item.id }) }}>
            Open in Queue
          </button>
        )}
      </li>
    )
  }
  return (
    <li id={`call-${sp.id}`} className={`orow${focus ? ' focus' : ''}${sp.error ? ' err' : ''}`} style={{ paddingLeft: 8 + depth * 18 }} data-focused={focus || undefined}>
      <span className="mono tm">{fmtClock(sp.start)}</span>
      <span className="tool">{sp.tool}</span>
      <span className="inp mono">{sp.input}</span>
      <span className="mono dur">{running ? 'running' : fmtDur(durOf(sp, s.simNow))}</span>
      <span className="mono res">{sp.error ? `exit ${sp.exit ?? 1}` : sp.exit === 0 ? 'exit 0' : ''}</span>
    </li>
  )
}

function Output({ s, run, det }: { s: State; run: Run; det: Detail }) {
  const { d } = useApp()
  const box = useRef<HTMLDivElement>(null)
  const spans = run.spans.filter((x) => !det.bd.phase || x.phase === det.bd.phase)
  const roots = spans.filter((x) => !x.parent)
  const kids = (id: string) => spans.filter((x) => x.parent === id)

  useEffect(() => {
    if (det.follow && box.current) box.current.scrollTop = box.current.scrollHeight
  }, [det.follow, spans.length])
  useEffect(() => {
    if (!det.focusCall) return
    document.getElementById(`call-${det.focusCall}`)?.scrollIntoView?.({ block: 'center' })
  }, [det.focusCall, det.tab])

  return (
    <div className="output">
      <div className="obar">
        <span className="note">{spans.length} rows. Output is untrusted text; links are not followed.</span>
        <span className="spacer" />
        {!det.follow && <button type="button" className="btn sm primary" onClick={() => d({ type: 'follow', on: true })}>Jump to latest</button>}
        {det.follow && <span className="note">following the newest output</span>}
      </div>
      <div
        className="olist" ref={box} tabIndex={0} aria-label="Agent output"
        onScroll={(e) => {
          const el = e.currentTarget
          if (det.follow && el.scrollTop + el.clientHeight < el.scrollHeight - 60) d({ type: 'follow', on: false })
        }}
      >
        <ul>
          {roots.map((sp) => (
            <span key={sp.id} style={{ display: 'contents' }}>
              <SpanRow s={s} sp={sp} focus={det.focusCall === sp.id} depth={0} />
              {kids(sp.id).map((k) => <SpanRow key={k.id} s={s} sp={k} focus={det.focusCall === k.id} depth={1} />)}
            </span>
          ))}
        </ul>
      </div>
    </div>
  )
}

function TableRows({ rows, det, depth = 0 }: { rows: Row[]; det: Detail; depth?: number }) {
  const { d } = useApp()
  return (
    <>
      {rows.map((r) => (
        <Fragment key={r.key}>
          <tr className={det.bd.tool === r.key ? 'sel' : ''}>
            <td style={{ paddingLeft: 6 + depth * 14 }}>
              <button type="button" className="rowbtn" aria-pressed={det.bd.tool === r.key} disabled={r.key === 'model'}
                onClick={() => d({ type: 'setBd', patch: { tool: det.bd.tool === r.key ? null : r.key } })}>
                {r.name}
              </button>
            </td>
            <td className="mono">{r.calls || ''}</td>
            <td className="mono">{fmtDur(r.total)}</td>
            <td className="mono">{(r.share * 100).toFixed(0)}%</td>
            <td className="mono">{r.calls ? fmtDur(r.median) : ''}</td>
            <td className="mono">{r.calls ? fmtDur(r.max) : ''}</td>
            <td className="mono">{r.errors || ''}</td>
          </tr>
          {r.children.length > 0 && <TableRows rows={r.children} det={det} depth={depth + 1} />}
        </Fragment>
      ))}
    </>
  )
}

function WindowInput({ det }: { det: Detail }) {
  const { d } = useApp()
  const [bad, setBad] = useState(false)
  const w = det.bd.win
  const apply = (i: 0 | 1, txt: string) => {
    if (!txt.trim()) {
      d({ type: 'setBd', patch: { win: null } })
      return
    }
    const v = parseClock(txt)
    if (v === null) { setBad(true); return }
    setBad(false)
    const other = w ? w[i === 0 ? 1 : 0] : v
    d({ type: 'setBd', patch: { win: i === 0 ? [v, Math.max(v, other)] : [Math.min(v, other), v] } })
  }
  return (
    <fieldset className="winbox">
      <legend>Time window</legend>
      <label>from <input aria-label="Window from" key={`a${w?.[0]}`} placeholder="HH:MM:SS" defaultValue={w ? fmtClock(w[0]) : ''}
        onBlur={(e) => apply(0, e.target.value)} onKeyDown={(e) => { if (e.key === 'Enter') apply(0, e.currentTarget.value) }} /></label>
      <label>to <input aria-label="Window to" key={`b${w?.[1]}`} placeholder="HH:MM:SS" defaultValue={w ? fmtClock(w[1]) : ''}
        onBlur={(e) => apply(1, e.target.value)} onKeyDown={(e) => { if (e.key === 'Enter') apply(1, e.currentTarget.value) }} /></label>
      <button type="button" className="btn sm" onClick={() => d({ type: 'setBd', patch: { win: null } })}>Clear</button>
      {bad && <span className="bad">Use HH:MM:SS</span>}
    </fieldset>
  )
}

function Timeline({ s, run, det }: { s: State; run: Run; det: Detail }) {
  const { d } = useApp()
  const a = Math.min(...run.phases.map((p) => p.start ?? s.simNow))
  const b = Math.max(a + 1, run.endWall ?? s.simNow)
  const x = (t: number) => ((t - a) / (b - a)) * 100
  const matched = new Set(filterSpans(run, det.bd, s.simNow).map((m) => m.id))
  const groups = [...new Set(run.spans.filter((sp) => sp.kind !== 'text' && !sp.parent).map(groupOf))]
  const drag = useRef<number | null>(null)
  const frac = (e: React.MouseEvent<HTMLDivElement>) => {
    const r = e.currentTarget.getBoundingClientRect()
    return a + ((e.clientX - r.left) / r.width) * (b - a)
  }
  return (
    <div className="timeline" aria-label="Timeline, one row per tool type">
      {groups.map((g) => (
        <div className="trow" key={g}>
          <span className="tl">{g}</span>
          <div
            className="tlane"
            onMouseDown={(e) => { drag.current = frac(e) }}
            onMouseUp={(e) => {
              const st = drag.current
              drag.current = null
              if (st === null) return
              const en = frac(e)
              if (Math.abs(en - st) > (b - a) * 0.01) d({ type: 'setBd', patch: { win: [Math.min(st, en), Math.max(st, en)] } })
            }}
          >
            {det.bd.win && <i className="winmark" style={{ left: `${x(det.bd.win[0])}%`, width: `${x(det.bd.win[1]) - x(det.bd.win[0])}%` }} />}
            {run.spans.filter((sp) => sp.kind !== 'text' && !sp.parent && groupOf(sp) === g).map((sp) => {
              const w = Math.max(0.25, x(sp.end ?? s.simNow) - x(sp.start))
              return (
                <button
                  key={sp.id} type="button" className={`bar${matched.has(sp.id) ? '' : ' dim'}${det.focusCall === sp.id ? ' focus' : ''}`}
                  style={{ left: `${x(sp.start)}%`, width: `${w}%` }}
                  title={`${sp.tool} ${sp.input}, ${fmtDur(durOf(sp, s.simNow))}, ${fmtClock(sp.start)}`}
                  aria-label={`${sp.tool} ${sp.input} ${fmtDur(durOf(sp, s.simNow))}`}
                  onMouseDown={(e) => e.stopPropagation()}
                  onClick={() => matched.has(sp.id) && d({ type: 'focusCall', id: sp.id })}
                />
              )
            })}
          </div>
        </div>
      ))}
      <div className="trow axis">
        <span className="tl" />
        <div className="tlane"><span className="mono">{fmtClock(a)}</span><span className="mono right">{fmtClock(b)}</span></div>
      </div>
    </div>
  )
}

function Breakdown({ s, run, det }: { s: State; run: Run; det: Detail }) {
  const { d } = useApp()
  const bd = breakdown(run, det.bd, s.simNow)
  const phase = run.phases.find((p) => p.key === det.bd.phase)
  return (
    <div className="bdwrap">
      <div className="bdfilters">
        <label>Tool <b className="mono">{det.bd.tool ?? 'all'}</b>
          {det.bd.tool && <button type="button" className="btn sm" onClick={() => d({ type: 'setBd', patch: { tool: null } })}>Clear</button>}
        </label>
        <label>Calls longer than{' '}
          <input
            type="number" min={0} step={0.5} aria-label="Minimum duration in seconds" value={det.bd.minDur}
            onChange={(e) => d({ type: 'setBd', patch: { minDur: Math.max(0, Number(e.target.value) || 0) } })}
          /> s
        </label>
        <WindowInput det={det} />
        {phase && (
          <button type="button" className="btn sm" onClick={() => d({ type: 'setBd', patch: { win: [phase.start ?? 0, phase.end ?? s.simNow] } })}>
            Window = this phase
          </button>
        )}
      </div>
      <p className="note">
        Clock time in scope {fmtDur(bd.wall)}. Summed call time {fmtDur(bd.summed)}; clock time with a call running {fmtDur(bd.union)} (parallel calls overlap).
        Hook timings are exact (telemetry); tool calls are approximate (stream timing). {bd.matched.length} calls match.
      </p>
      <table className="bdtable">
        <thead><tr><th>Tool</th><th>Calls</th><th>Total</th><th>Share</th><th>Median</th><th>Max</th><th>Errors</th></tr></thead>
        <tbody><TableRows rows={bd.rows} det={det} /></tbody>
      </table>
      <Timeline s={s} run={run} det={det} />
      <p className="note">Drag on a row to set the window. Click a bar to open the output at that call.</p>
    </div>
  )
}

function Review({ s, run }: { s: State; run: Run }) {
  const { d } = useApp()
  const [note, setNote] = useState('')
  const can = run.status === 'done' && run.review === 'pending'
  const add = run.diff.reduce((a, f) => a + f.add, 0)
  const del = run.diff.reduce((a, f) => a + f.del, 0)
  return (
    <div className="review">
      <section>
        <h3>Phase report</h3>
        <p>{run.report}</p>
      </section>
      <section>
        <h3>Gates</h3>
        <ul>{run.gates.length ? run.gates.map((g, i) => <li key={i}><span className={`gv g-${g.verdict}`}>{g.verdict}</span> {g.name}, {g.note}</li>) : <li className="note">none yet</li>}</ul>
      </section>
      <section>
        <h3>Reviewers</h3>
        <ul>{run.findings.map((f, i) => <li key={i}><b>Reviewer {f.reviewer}:</b> {f.text} <span className="note">({f.adjudicated})</span></li>)}</ul>
      </section>
      <section>
        <h3>Diff, {run.diff.length} files, +{add} -{del}</h3>
        {run.diff.map((f) => (
          <details key={f.path} open={f.dep || run.diff.indexOf(f) === 0}>
            <summary><span className="mono">{f.path}</span> <span className="mono note">+{f.add} -{f.del}</span>{f.dep && <span className="lab">dependency change</span>}</summary>
            <pre>{f.lines.join('\n')}</pre>
          </details>
        ))}
      </section>
      <section className="decide">
        {can ? (
          <>
            <textarea aria-label="Note for a re-prompt" placeholder="Note for a re-prompt" rows={2} value={note} onChange={(e) => setNote(e.target.value)} />
            <div className="acts">
              <button type="button" className="btn primary" onClick={() => d({ type: 'reviewDecide', runId: run.id, effect: 'accept' })}>Accept</button>
              <button type="button" className="btn" onClick={() => d({ type: 'reviewDecide', runId: run.id, effect: 'reprompt' })}>Re-prompt</button>
              <button type="button" className="btn danger" onClick={() => d({ type: 'reviewDecide', runId: run.id, effect: 'discard' })}>Discard</button>
            </div>
          </>
        ) : (
          <p className="note">{run.status === 'done' ? `Review decision: ${run.review}.` : 'This run has not finished. Its diff can be reviewed when it does.'}</p>
        )}
      </section>
      <span className="sr">{s.simNow}</span>
    </div>
  )
}

const TABS: { id: DetailTab; label: string }[] = [
  { id: 'output', label: 'Output' },
  { id: 'breakdown', label: 'Time breakdown' },
  { id: 'review', label: 'Review' },
]

export function RunDetail() {
  const { s, d } = useApp()
  const det = s.detail
  if (!det) return null
  const run = runOf(s, det.runId)
  if (!run) return null
  const task = taskOf(s, run.taskId)
  return (
    <aside className="detail" aria-label="Run detail">
      <header>
        <RunTag s={s} run={run} />
        <span className="mono id">{run.id}</span>
        <Chip st={run.status} />
        <span className="spacer" />
        <button type="button" className="btn sm" onClick={() => d({ type: 'closeDetail' })}>Close <Kbd>Esc</Kbd></button>
      </header>
      <div className="dtitle"><span className="mono">{run.taskId ?? 'scratch'}</span> {task?.title ?? run.title}</div>
      <div className="tabs" role="tablist">
        {TABS.map((t) => (
          <button key={t.id} type="button" role="tab" aria-selected={det.tab === t.id} onClick={() => d({ type: 'setTab', tab: t.id })}>{t.label}</button>
        ))}
      </div>
      {det.tab !== 'review' && <PhaseTabs run={run} d={det} />}
      <div className="dbody">
        {det.tab === 'output' && <Output s={s} run={run} det={det} />}
        {det.tab === 'breakdown' && <Breakdown s={s} run={run} det={det} />}
        {det.tab === 'review' && <Review s={s} run={run} />}
      </div>
    </aside>
  )
}
