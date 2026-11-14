import { experimentResult } from '../experiment'
import { fmtDur, statusOf } from '../world'
import { useApp } from './store'
import type { Branch, Experiment } from '../types'

function Arm({ b }: { b: Branch }) {
  return (
    <div className="arm">
      <b className="mono">{b.name}</b>
      <span className="note"> merge base {b.mergeBase}</span>
      <ul className="stat mono">
        {b.stat.map((f) => (
          <li key={f.path} className={f.protectedPath ? 'prot' : ''}>
            {f.path} +{f.add} -{f.del} {f.protectedPath && <span className="lab">protected path</span>}
          </li>
        ))}
        {!b.stat.length && <li className="note">no changes</li>}
      </ul>
    </div>
  )
}

function Preview() {
  const { s } = useApp()
  const { taskId, armA, armB } = s.expDraft
  const a = s.branches.find((b) => b.name === armA)
  const b = s.branches.find((x) => x.name === armB)
  if (!taskId || !a || !b) return null
  const sibling = b.mergeBase === a.name || b.mergeBase === a.mergeBase
  return (
    <section className="preview" aria-label="Experiment preview">
      <h3>Preview</h3>
      <div className="arms"><Arm b={a} /><Arm b={b} /></div>
      {armA === armB && <p className="warn">Both arms are the same branch.</p>}
      {!sibling && <p className="warn">B is not A plus its change: the merge base of {b.name} is {b.mergeBase}, not {a.name}.</p>}
      <dl>
        <dt>Primary metric</dt><dd>critical path to green, in minutes</dd>
        <dt>Design</dt><dd>paired, alternating order. It can give a verdict with 6 pairs.</dd>
        <dt>Secondary metrics</dt><dd>1 supported at this pair count: spend</dd>
        <dt>Other live runs</dt><dd>{s.runs.filter((r) => r.status === 'working').length}</dd>
        <dt>Cost</dt><dd>$50.40 for 12 runs</dd>
        <dt>Expected duration</dt><dd>unknown: no earlier runs of this workflow have timings</dd>
      </dl>
    </section>
  )
}

function Result({ e }: { e: Experiment }) {
  const { s, d } = useApp()
  const r = experimentResult(e)
  const left = e.startedWall + e.duration - s.simNow
  if (e.status === 'running') {
    return (
      <section className="result" aria-label="Experiment progress">
        <h3>{e.armA} against {e.armB}</h3>
        <p>Running on {e.taskId}. About {fmtDur(Math.max(0, left))} to go.</p>
        <div className="prog"><i style={{ width: `${Math.min(100, ((s.simNow - e.startedWall) / e.duration) * 100)}%` }} /></div>
      </section>
    )
  }
  return (
    <section className="result" aria-label="Experiment result">
      <h3>Result: {e.armA} (A) against {e.armB} (B), on {e.taskId}</h3>
      <table className="cmp">
        <thead><tr><th>Phase</th><th>A</th><th>B</th><th>B minus A</th></tr></thead>
        <tbody>
          {r.rows.map((x) => (
            <tr key={x.phase}><td>{x.phase}</td><td className="mono">{x.a} min</td><td className="mono">{x.b} min</td><td className="mono">{x.b - x.a > 0 ? '+' : ''}{x.b - x.a} min</td></tr>
          ))}
          <tr className="total"><td>Critical path</td><td className="mono">{r.critA} min</td><td className="mono">{r.critB} min</td><td className="mono">{r.critB - r.critA > 0 ? '+' : ''}{r.critB - r.critA} min</td></tr>
          <tr><td>Outcome</td><td>{r.outcomeA}</td><td>{r.outcomeB}</td><td /></tr>
        </tbody>
      </table>
      <p className="verdict"><b>Verdict rule:</b> {r.verdict}</p>
      <div className="note-box">
        <h3>Your reading</h3>
        <textarea
          aria-label="What does the result say" rows={2} placeholder="Which arm won, and by how much?"
          value={s.expNote.text} onChange={(ev) => d({ type: 'expNote', text: ev.target.value })}
        />
        <button type="button" className="btn primary sm" disabled={!s.expNote.text.trim()} onClick={() => d({ type: 'expNoteSave' })}>Save reading</button>
        {s.expNote.saved && <span className="note" role="status">Saved: {s.expNote.saved}</span>}
      </div>
    </section>
  )
}

export function Experiments() {
  const { s, d } = useApp()
  const draft = s.expDraft
  const tasks = s.tasks.filter((t) => statusOf(s, t) !== 'done')
  const open = s.experiments.find((e) => e.id === s.expOpen)
  return (
    <div className="experiments">
      <header className="trkhead"><h2>Experiments</h2><span className="note">Pick a task, then two arms.</span></header>
      <form className="expform" onSubmit={(e) => { e.preventDefault(); d({ type: 'expStart' }) }}>
        <label>Task
          <select value={draft.taskId} onChange={(e) => d({ type: 'expDraft', patch: { taskId: e.target.value } })}>
            <option value="">choose a task</option>
            {tasks.map((t) => <option key={t.id} value={t.id}>{t.id} {t.title}</option>)}
          </select>
        </label>
        <label>Arm A
          <select value={draft.armA} onChange={(e) => d({ type: 'expDraft', patch: { armA: e.target.value } })}>
            <option value="">choose a branch</option>
            {s.branches.map((b) => <option key={b.name} value={b.name}>{b.name}</option>)}
          </select>
        </label>
        <label>Arm B
          <select value={draft.armB} onChange={(e) => d({ type: 'expDraft', patch: { armB: e.target.value } })}>
            <option value="">choose a branch</option>
            {s.branches.map((b) => <option key={b.name} value={b.name}>{b.name}</option>)}
          </select>
        </label>
        <button type="submit" className="btn primary" disabled={!draft.taskId || !draft.armA || !draft.armB || draft.armA === draft.armB}>Start experiment</button>
      </form>
      <Preview />
      {s.experiments.length > 0 && (
        <section className="explist" aria-label="Experiments">
          <h3>Experiments</h3>
          <ul>
            {s.experiments.map((e) => (
              <li key={e.id}>
                <button type="button" className="fchip" aria-pressed={s.expOpen === e.id} onClick={() => d({ type: 'expOpen', id: e.id })}>
                  {e.id} {e.taskId}: {e.armA} against {e.armB}, {e.status}
                </button>
              </li>
            ))}
          </ul>
        </section>
      )}
      {open && <Result e={open} />}
    </div>
  )
}
