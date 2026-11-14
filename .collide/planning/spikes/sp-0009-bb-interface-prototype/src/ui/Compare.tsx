import { breakdown, NO_FILTER, phaseTimes } from '../breakdown'
import { fmtDur, runOf } from '../world'
import { RunTag } from './common'
import { useApp } from './store'

const sign = (n: number): string => `${n > 0 ? '+' : n < 0 ? '-' : ''}${fmtDur(Math.abs(n))}`

export function Compare() {
  const { s, d } = useApp()
  const c = s.compare
  const a = runOf(s, c?.a)
  const b = runOf(s, c?.b)
  if (!c || !a || !b) return <p className="note pad">Pick two finished runs in Tracks.</p>
  const pa = phaseTimes(a)
  const pb = phaseTimes(b)
  const keys = [...new Set([...pa, ...pb].map((p) => p.key))]
  const find = (xs: typeof pa, k: string) => xs.find((p) => p.key === k)
  const ta = breakdown(a, NO_FILTER, s.simNow).rows.filter((r) => r.key !== 'model')
  const tb = breakdown(b, NO_FILTER, s.simNow).rows.filter((r) => r.key !== 'model')
  const tkeys = [...new Set([...ta, ...tb].map((r) => r.key))]
  const totA = (a.endWall ?? 0) - a.startWall
  const totB = (b.endWall ?? 0) - b.startWall
  return (
    <div className="compare">
      <header className="trkhead">
        <h2>Compare</h2>
        <span><RunTag s={s} run={a} /> {a.taskId} against <RunTag s={s} run={b} /> {b.taskId}</span>
        <span className="spacer" />
        <button type="button" className="btn sm" onClick={() => d({ type: 'nav', surface: 'tracks' })}>Back to Tracks</button>
      </header>
      <p className="note">Side by side, with no verdict. Waiting on you is shown apart from active time.</p>
      <table className="cmp">
        <thead>
          <tr><th>Phase</th><th>{a.id} took</th><th>{a.id} waiting</th><th>{b.id} took</th><th>{b.id} waiting</th><th>Difference</th></tr>
        </thead>
        <tbody>
          {keys.map((k) => {
            const x = find(pa, k)
            const y = find(pb, k)
            const diff = (y?.wall ?? 0) - (x?.wall ?? 0)
            return (
              <tr key={k}>
                <td>{x?.label ?? y?.label}</td>
                <td className="mono">{x ? fmtDur(x.wall) : ''}</td>
                <td className="mono">{x && x.wait ? fmtDur(x.wait) : ''}</td>
                <td className="mono">{y ? fmtDur(y.wall) : ''}</td>
                <td className="mono">{y && y.wait ? fmtDur(y.wait) : ''}</td>
                <td className="mono">{sign(diff)}</td>
              </tr>
            )
          })}
          <tr className="total"><td>Whole run</td><td className="mono">{fmtDur(totA)}</td><td /><td className="mono">{fmtDur(totB)}</td><td /><td className="mono">{sign(totB - totA)}</td></tr>
        </tbody>
      </table>
      <table className="cmp">
        <thead><tr><th>Tool</th><th>{a.id} total</th><th>{a.id} calls</th><th>{b.id} total</th><th>{b.id} calls</th><th>Difference</th></tr></thead>
        <tbody>
          {tkeys.map((k) => {
            const x = ta.find((r) => r.key === k)
            const y = tb.find((r) => r.key === k)
            return (
              <tr key={k}>
                <td>{k}</td>
                <td className="mono">{x ? fmtDur(x.total) : ''}</td><td className="mono">{x?.calls ?? ''}</td>
                <td className="mono">{y ? fmtDur(y.total) : ''}</td><td className="mono">{y?.calls ?? ''}</td>
                <td className="mono">{sign((y?.total ?? 0) - (x?.total ?? 0))}</td>
              </tr>
            )
          })}
        </tbody>
      </table>
      <section className="note-box">
        <h3>Note on this comparison</h3>
        <textarea
          aria-label="Note on this comparison" rows={2} placeholder="Where did the time go? One line."
          value={c.note} onChange={(e) => d({ type: 'compareNote', text: e.target.value })}
        />
        <button type="button" className="btn primary sm" disabled={!c.note.trim()} onClick={() => d({ type: 'compareSave' })}>Save note</button>
        {c.saved && <span className="note" role="status">Saved: {c.saved}</span>}
      </section>
    </div>
  )
}
