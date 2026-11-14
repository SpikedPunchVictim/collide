import { plannedPhases } from '../sim'
import { SIM_ZERO } from '../sim'
import { fmtClock, fmtDur, isLive, taskOf } from '../world'
import { Chip, RunTag, runColour } from './common'
import { useApp } from './store'
import type { Phase, Run, State } from '../types'

/** The usage-limit reset line is fixed fake data: 30 minutes after the sessions' fake 10:00 start. */
export const LIMIT_RESET = SIM_ZERO + 3600 + 1800

function pct(s: State, t: number): number {
  return ((t - s.tracks.start) / s.tracks.span) * 100
}

function Seg({ s, r, p, planned }: { s: State; r: Run; p: Phase; planned?: boolean }) {
  const { d } = useApp()
  const a = pct(s, p.start ?? s.simNow)
  const b = pct(s, p.end ?? (planned ? p.start ?? s.simNow : s.simNow))
  const left = Math.max(0, a)
  const right = Math.min(100, b)
  if (right <= 0 || left >= 100 || right - left < 0.05) return null
  const live = !planned && p.end === null
  const cls = planned ? 'seg planned' : live ? `seg live s-${r.status}` : 'seg done'
  const waits = planned ? [] : r.spans.filter((x) => x.kind === 'wait' && x.phase === p.key)
  const label = p.reviewer ? `review ${p.reviewer}` : p.label
  return (
    <button
      type="button"
      className={`${cls}${p.reviewer ? ` rev${p.reviewer}` : ''}`}
      style={{ left: `${left}%`, width: `${right - left}%` }}
      title={`${label}${p.start ? ` from ${fmtClock(p.start)}` : ''}${p.end ? ` to ${fmtClock(p.end)}` : ''}`}
      onClick={() => d({ type: 'selectRun', runId: r.id, phase: p.key, tab: 'output' })}
    >
      {right - left > 5 && <span className="lbl">{label}</span>}
      {waits.map((w) => {
        const wa = Math.max(a, pct(s, w.start))
        const wb = Math.min(b, pct(s, w.end ?? s.simNow))
        if (wb <= wa) return null
        return <i key={w.id} className="wait" style={{ left: `${((wa - a) / (b - a)) * 100}%`, width: `${((wb - wa) / (b - a)) * 100}%` }} title="waiting on you" />
      })}
    </button>
  )
}

function Row({ s, r }: { s: State; r: Run }) {
  const { d } = useApp()
  const task = taskOf(s, r.taskId)
  const planned = plannedPhases(r, s.simNow)
  const focus = s.tracks.focus
  const col = runColour(s, r)
  const pending = s.queue.filter((q) => q.runId === r.id && q.status === 'pending')
  return (
    <div
      className={`trk${s.hover === r.id ? ' hl' : ''}${focus?.runId === r.id ? ' focus' : ''}`}
      data-run={r.id}
      style={col ? { borderLeft: `4px solid ${col}` } : undefined}
      onMouseEnter={() => d({ type: 'hover', runId: r.id })}
      onMouseLeave={() => d({ type: 'hover', runId: null })}
    >
      <button type="button" className="who" onClick={() => d({ type: 'selectRun', runId: r.id })}>
        <span className="r1"><RunTag s={s} run={r} /><Chip st={r.status} /></span>
        <span className="mono note">{r.taskId ?? 'scratch'}</span>
        <span className="note ell">{task?.title ?? r.title}</span>
      </button>
      <div className="axis">
        {r.phases.map((p) => <Seg key={p.key} s={s} r={r} p={p} />)}
        {planned.map((p) => <Seg key={`p-${p.key}`} s={s} r={r} p={p} planned />)}
        {r.gates.map((g, i) => {
          const x = pct(s, g.at)
          if (x < 0 || x > 100) return null
          return <i key={i} className={`gate g-${g.verdict}`} style={{ left: `${x}%` }} title={`gate ${g.name}: ${g.verdict}, ${g.note}`} />
        })}
        {r.markers.map((m, i) => {
          const x = pct(s, m.at)
          if (x < 0 || x > 100) return null
          return <i key={i} className="finding" style={{ left: `${x}%` }} title={m.text} />
        })}
        {pending.map((q) => {
          const x = pct(s, q.raisedWall)
          if (x < 0 || x > 100) return null
          return (
            <i
              key={q.id} className={`decision${focus?.itemId === q.id ? ' focus' : ''}`} style={{ left: `${x}%` }}
              title={`${q.title}, asked at ${fmtClock(q.raisedWall)}`}
            />
          )
        })}
      </div>
      <div className="meta mono note">
        ${r.spend.toFixed(2)} of ${r.budget.toFixed(2)}
        <br />{fmtDur(((r.endWall ?? s.simNow) - r.startWall))}
      </div>
    </div>
  )
}

function Ticks({ s }: { s: State }) {
  const step = s.tracks.span <= 900 ? 60 : s.tracks.span <= 3600 ? 300 : s.tracks.span <= 7200 ? 600 : 1800
  const ticks: number[] = []
  for (let t = Math.ceil(s.tracks.start / step) * step; t < s.tracks.start + s.tracks.span; t += step) ticks.push(t)
  return (
    <div className="ticks" aria-hidden="true">
      {ticks.map((t) => (
        <span key={t} className="tick mono" style={{ left: `${pct(s, t)}%` }}>{fmtClock(t).slice(0, 5)}</span>
      ))}
    </div>
  )
}

function Finished() {
  const { s, d } = useApp()
  const done = s.runs.filter((r) => r.status === 'done')
  if (!done.length) return null
  return (
    <section className="finished" aria-label="Finished runs">
      <header>
        <h2>Finished runs</h2>
        <span className="note">Tick two to compare them.</span>
        <span className="spacer" />
        <button type="button" className="btn sm primary" disabled={s.compareSel.length !== 2} onClick={() => d({ type: 'openCompare' })}>
          Compare {s.compareSel.length === 2 ? '2 runs' : ''}
        </button>
      </header>
      <table>
        <thead><tr><th /><th>Run</th><th>Task</th><th>Started</th><th>Took</th><th>Waiting on you</th><th>Review</th><th /></tr></thead>
        <tbody>
          {done.map((r) => {
            const wait = r.spans.filter((x) => x.kind === 'wait').reduce((a, x) => a + ((x.end ?? s.simNow) - x.start), 0)
            return (
              <tr key={r.id}>
                <td><input type="checkbox" aria-label={`Compare ${r.id}`} checked={s.compareSel.includes(r.id)} onChange={() => d({ type: 'compareToggle', runId: r.id })} /></td>
                <td><RunTag s={s} run={r} /></td>
                <td className="mono">{r.taskId}</td>
                <td className="mono">{fmtClock(r.startWall)}</td>
                <td className="mono">{fmtDur((r.endWall ?? 0) - r.startWall)}</td>
                <td className="mono">{wait ? fmtDur(wait) : 'none'}</td>
                <td>{r.review}</td>
                <td><button type="button" className="btn sm" onClick={() => d({ type: 'selectRun', runId: r.id, tab: 'breakdown' })}>Open</button></td>
              </tr>
            )
          })}
        </tbody>
      </table>
    </section>
  )
}

export function Tracks() {
  const { s, d } = useApp()
  const live = s.runs.filter((r) => isLive(r) || (r.endWall !== null && r.endWall > s.tracks.start && r.status === 'done' && s.simNow - r.endWall < 3600))
  const nowX = pct(s, s.simNow)
  const limX = pct(s, LIMIT_RESET)
  return (
    <div className="tracks">
      <header className="trkhead">
        <h2>Tracks</h2>
        <span className="note">{fmtClock(s.tracks.start)} to {fmtClock(s.tracks.start + s.tracks.span)}, {fmtDur(s.tracks.span)} across</span>
        <span className="spacer" />
        <button type="button" className="btn sm" aria-label="Pan back" onClick={() => d({ type: 'pan', dir: -1 })}>&larr;</button>
        <button type="button" className="btn sm" aria-label="Pan forward" onClick={() => d({ type: 'pan', dir: 1 })}>&rarr;</button>
        <button type="button" className="btn sm" aria-label="Zoom out" onClick={() => d({ type: 'zoom', dir: -1 })}>&minus;</button>
        <button type="button" className="btn sm" aria-label="Zoom in" onClick={() => d({ type: 'zoom', dir: 1 })}>+</button>
        <button type="button" className="btn sm" aria-pressed={s.tracks.follow} onClick={() => d({ type: 'trackFollow', on: !s.tracks.follow })}>Follow now</button>
        {s.returnTo && <button type="button" className="btn sm primary" onClick={() => d({ type: 'back' })}>Back to {s.returnTo} (Esc)</button>}
      </header>
      <div className="trkgrid">
        <div className="trk axisrow"><span /><Ticks s={s} /><span /></div>
        <div className="rows">
          {live.map((r) => <Row key={r.id} s={s} r={r} />)}
          {!live.length && <p className="note pad">No live runs. Launch a task from the Field.</p>}
          <div className="vline now" style={{ left: `calc(170px + (100% - 170px - 120px) * ${nowX / 100})` }} title={`now ${fmtClock(s.simNow)}`}><span>now</span></div>
          {limX > 0 && limX < 100 && (
            <div className="vline limit" style={{ left: `calc(170px + (100% - 170px - 120px) * ${limX / 100})` }} title="usage limit resets"><span>limit resets {fmtClock(LIMIT_RESET).slice(0, 5)}</span></div>
          )}
        </div>
      </div>
      <p className="legend note">
        Segment width is real time. Hatched means waiting on you. Dashed is the projected remainder. Diamonds are gates, triangles are findings, circles are decisions asked.
      </p>
      <Finished />
    </div>
  )
}
