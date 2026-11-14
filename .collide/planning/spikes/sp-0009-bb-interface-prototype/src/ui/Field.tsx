import { useState } from 'react'
import { queueOrder } from '../reducer'
import { WORKFLOWS, celOf, fmtClock, fmtDur, isLive, runOf, sortTasks, statusOf, taskColumns, taskOf, visibleTasks } from '../world'
import { Chip, Kbd, RunTag, runColour } from './common'
import { useApp } from './store'
import type { Filter, Task, TaskStatus } from '../types'

const COLUMNS: TaskStatus[] = ['ready', 'blocked', 'running', 'review', 'done']
const SOURCES = ['tasks/', 'github']
const MAX_PER_COLUMN = 30

export function LineSwitcher({ where }: { where: 'top' | 'filter' }) {
  const { s, d } = useApp()
  if (where === 'top') {
    return (
      <label className="linebtn" title="Line switcher">
        <span className="lbl">Line</span>
        <select aria-label="Line" value={s.line} onChange={(e) => d({ type: 'setLine', line: e.target.value })}>
          {s.lines.map((l) => <option key={l} value={l}>{l}</option>)}
        </select>
      </label>
    )
  }
  return (
    <div className="fgroup" role="group" aria-label="Line">
      <span className="lbl">Line</span>
      {s.lines.map((l) => (
        <button key={l} type="button" className="fchip" aria-pressed={s.line === l} onClick={() => d({ type: 'setLine', line: l })}>
          {l}
        </button>
      ))}
    </div>
  )
}

function ChipGroup({ label, group, values }: { label: string; group: Exclude<keyof Filter, 'text' | 'ids'>; values: string[] }) {
  const { s, d } = useApp()
  return (
    <div className="fgroup" role="group" aria-label={label}>
      <span className="lbl">{label}</span>
      {values.map((v) => (
        <button
          key={v}
          type="button"
          className="fchip"
          aria-pressed={(s.filter[group] as string[]).includes(v)}
          onClick={() => d({ type: 'toggleFilter', group, value: v })}
        >
          {v}
        </button>
      ))}
    </div>
  )
}

function SaveView() {
  const { s, d } = useApp()
  const [name, setName] = useState('')
  if (!s.saving) return <button type="button" className="btn sm" onClick={() => d({ type: 'startSave' })}>Save as view</button>
  return (
    <form className="saveform" onSubmit={(e) => { e.preventDefault(); d({ type: 'saveView', name }) }}>
      <input aria-label="View name" autoFocus placeholder="View name" value={name} onChange={(e) => setName(e.target.value)} />
      <button type="submit" className="btn sm primary" disabled={!name.trim()}>Save</button>
      <button type="button" className="btn sm" onClick={() => d({ type: 'cancelSave' })}>Cancel</button>
    </form>
  )
}

function FilterBar() {
  const { s, d } = useApp()
  const labels = [...new Set(s.tasks.flatMap((t) => t.labels))].sort()
  const shown = visibleTasks(s).length
  return (
    <section className="filters" aria-label="Filter bar">
      <div className="frow">
        {s.variants.line === 'filter' && <LineSwitcher where="filter" />}
        <ChipGroup label="Collection" group="epic" values={s.epics.map((e) => e.id)} />
        <ChipGroup label="Status" group="status" values={['ready', 'blocked', 'backlog', 'running', 'review', 'done']} />
        <ChipGroup label="Label" group="label" values={labels} />
        <ChipGroup label="Source" group="source" values={SOURCES} />
      </div>
      <div className="frow">
        <input
          className="ftext" type="search" placeholder="type to filter by id or title" aria-label="Filter text"
          value={s.filter.text} onChange={(e) => d({ type: 'filterText', text: e.target.value })}
        />
        <div className="cel"><span className="lbl">CEL</span><code>{celOf(s.filter)}</code></div>
        <span className="note" role="status">{shown} of {s.tasks.length} tasks</span>
        {(s.fieldView.sort || s.fieldView.groupBy !== 'status') && (
          <span className="fchip viewmod" role="status">
            {s.fieldView.sort && `sorted by ${s.fieldView.sort.by} ${s.fieldView.sort.dir}`}
            {s.fieldView.sort && s.fieldView.groupBy !== 'status' && ', '}
            {s.fieldView.groupBy !== 'status' && `grouped by ${s.fieldView.groupBy}`}
            <button type="button" aria-label="Reset sort and grouping" onClick={() => d({ type: 'fieldViewReset' })}>x</button>
          </span>
        )}
        <span className="spacer" />
        <div className="views" role="group" aria-label="Views">
          <span className="lbl">Views</span>
          {s.views.filter((v) => v.surface !== 'tracks').map((v) => (
            <button key={v.id} type="button" className="fchip" aria-pressed={s.activeView === v.id} onClick={() => d({ type: 'openView', id: v.id })}>
              {v.name}
            </button>
          ))}
          {s.snapshots.filter((l) => l.surface === 'field').map((l) => (
            <button key={l.id} type="button" className="fchip snap" aria-pressed={s.lens?.id === l.id} title={`Snapshot from ${new Date(l.at).toLocaleTimeString()}`} onClick={() => d({ type: 'openSnapshot', id: l.id })}>
              {l.name}
            </button>
          ))}
        </div>
        <SaveView />
        <button type="button" className="btn sm" onClick={() => d({ type: 'clearFilter' })}>Clear</button>
        <button type="button" className="btn sm" onClick={() => d({ type: 'selectVisible' })}>Select all shown</button>
        <button type="button" className="btn primary sm" disabled={!s.selected.length} onClick={() => d({ type: 'openLaunch' })}>
          Launch {s.selected.length}
        </button>
      </div>
      {s.launch.open && <LaunchPreview />}
    </section>
  )
}

function LaunchPreview() {
  const { s, d } = useApp()
  const n = s.selected.length
  return (
    <div className="launch" role="region" aria-label="Launch preview">
      <div className="lrow">
        <b>Launch {n} task{n === 1 ? '' : 's'}</b>
        <span className="mono note">{s.selected.join(', ')}</span>
      </div>
      <div className="lrow">
        <label>Workflow{' '}
          <select value={s.launch.workflow} onChange={(e) => d({ type: 'setLaunch', workflow: e.target.value })}>
            {WORKFLOWS.map((w) => <option key={w}>{w}</option>)}
          </select>
        </label>
        <span className="note">on line {s.line}</span>
        <fieldset className="radios">
          <label><input type="radio" name="mode" checked={s.launch.mode === 'separate'} onChange={() => d({ type: 'setLaunch', mode: 'separate' })} /> Separate runs</label>
          <label><input type="radio" name="mode" checked={s.launch.mode === 'group'} onChange={() => d({ type: 'setLaunch', mode: 'group' })} /> One group run</label>
        </fieldset>
      </div>
      <div className="lrow note">
        Estimated cost ${(n * 4.2).toFixed(2)}. Expected duration unknown: fewer than three earlier runs of this workflow.
      </div>
      <div className="lrow">
        <button type="button" className="btn primary" onClick={() => d({ type: 'confirmLaunch' })}>Launch</button>
        <button type="button" className="btn" onClick={() => d({ type: 'closeLaunch' })}>Cancel</button>
      </div>
    </div>
  )
}

function Card({ t, dim }: { t: Task; dim?: boolean }) {
  const { s, d } = useApp()
  const st = statusOf(s, t)
  const run = [...s.runs].reverse().find((r) => r.taskId === t.id && (isLive(r) || r.review === 'pending'))
  const lit = !!run && isLive(run)
  const col = run && lit ? runColour(s, run) : undefined
  const sel = s.selected.includes(t.id)
  const hl = !!run && s.hover === run.id
  const depsOpen = t.dependsOn.length > 0 && st === 'blocked'
  const marked = s.highlight?.surface === 'field' && s.highlight.ids.includes(t.id)
  return (
    <li>
      <article
        className={`card s-${st}${lit ? ' lit' : ''}${sel ? ' selected' : ''}${hl ? ' hl' : ''}${marked ? ' agent-hl' : ''}${dim ? ' dim' : ''}${run && lit ? ` r-${run.status}` : ''}`}
        style={col ? { borderColor: col, borderLeftColor: col } : undefined}
        data-task={t.id}
        tabIndex={0}
        onMouseEnter={() => run && d({ type: 'hover', runId: run.id })}
        onMouseLeave={() => run && d({ type: 'hover', runId: null })}
        onClick={() => d({ type: 'openDrawer', id: t.id })}
        onKeyDown={(e) => { if (e.key === 'Enter') d({ type: 'openDrawer', id: t.id }) }}
      >
        <div className="ct">
          <input
            type="checkbox" checked={sel} aria-label={`Select ${t.id}`}
            onClick={(e) => e.stopPropagation()} onChange={() => d({ type: 'toggleSelect', id: t.id })}
          />
          <span className="mono id">{t.id}</span>
          {run && lit && <span className="rt"><RunTag s={s} run={run} /></span>}
        </div>
        <div className="ttl">{t.title}</div>
        <div className="st">
          <Chip st={run && lit ? run.status : st}>
            {run && lit ? (run.status === 'working' ? `working, ${run.phases[run.phases.length - 1]?.label ?? 'starting'}` : undefined) : undefined}
          </Chip>
          {depsOpen && <span className="mono note">needs {t.dependsOn[0]}</span>}
        </div>
        <div className="cm">{t.labels.map((l) => <span key={l} className="lab">{l}</span>)}</div>
        {st === 'review' && run && (
          <button type="button" className="btn sm" onClick={(e) => { e.stopPropagation(); d({ type: 'selectRun', runId: run.id, tab: 'review' }) }}>
            Review
          </button>
        )}
        {run && lit && (
          <button type="button" className="btn sm" onClick={(e) => { e.stopPropagation(); d({ type: 'selectRun', runId: run.id }) }}>
            Output
          </button>
        )}
      </article>
    </li>
  )
}

function Board() {
  const { s } = useApp()
  if (s.lens?.surface === 'field') return <LensBoard />
  const tasks = sortTasks(s, visibleTasks(s), s.fieldView.sort)
  const cols = taskColumns(s, tasks, s.fieldView.groupBy, COLUMNS)
  return (
    <div className="cols" aria-label="Field board">
      {cols.map((c) => (
        <div className="col" key={c.key} aria-label={c.key}>
          <h3>{c.label} <span className="count">{c.tasks.length}</span></h3>
          <ul>
            {c.tasks.slice(0, MAX_PER_COLUMN).map((t) => <Card key={t.id} t={t} />)}
            {c.tasks.length > MAX_PER_COLUMN && <li className="note">+{c.tasks.length - MAX_PER_COLUMN} more, narrow the filter</li>}
          </ul>
        </div>
      ))}
    </div>
  )
}

/** The agent's grouping: named groups, each with its reason. Drawn with the same cards; the launch stays the user's. */
function LensBoard() {
  const { s, d } = useApp()
  const lens = s.lens!
  const inGroup = new Set(lens.groups.flatMap((g) => g.ids))
  const rest = visibleTasks(s).filter((t) => !inGroup.has(t.id))
  const gone = lens.groups.flatMap((g) => g.ids).filter((id) => !taskOf(s, id)).length
  return (
    <div className="lens" aria-label="Lens">
      <header className="lenshead">
        <b>{lens.title}</b>
        <span className="note">
          Agent grouping, a snapshot from {new Date(lens.at).toLocaleTimeString()}{lens.name ? `, saved as "${lens.name}"` : ''}. It does not update when tasks change.
          {gone > 0 && ` ${gone} task${gone === 1 ? ' is' : 's are'} no longer here.`}
        </span>
        <span className="spacer" />
        <button type="button" className="btn sm" onClick={() => d({ type: 'lensClear' })}>Clear lens</button>
      </header>
      <div className="cols">
        {lens.groups.map((g) => {
          const tasks = sortTasks(s, g.ids.map((id) => taskOf(s, id)).filter((x): x is Task => !!x), { by: 'id', dir: 'asc' })
          return (
            <div className="col lensgroup" key={g.name} aria-label={g.name}>
              <h3>{g.name} <span className="count">{tasks.length}</span></h3>
              <p className="reason">{g.reason}</p>
              <button type="button" className="btn sm" onClick={() => d({ type: 'selectIds', ids: tasks.filter((t) => statusOf(s, t) !== 'done').map((t) => t.id) })}>
                Select these {tasks.length}
              </button>
              <ul>{tasks.map((t) => <Card key={t.id} t={t} />)}</ul>
            </div>
          )
        })}
        {lens.rest === 'dim' && rest.length > 0 && (
          <div className="col lensrest" aria-label="not grouped">
            <h3>not grouped <span className="count">{rest.length}</span></h3>
            <p className="reason">The agent left these out.</p>
            <ul>{sortTasks(s, rest, { by: 'id', dir: 'asc' }).slice(0, MAX_PER_COLUMN).map((t) => <Card key={t.id} t={t} dim />)}</ul>
          </div>
        )}
      </div>
    </div>
  )
}

function EmptyField() {
  const { s, d } = useApp()
  return (
    <div className="empty-field">
      <h2>Nothing here yet</h2>
      <p className="note">No tasks and no pack in this repository. Describe a change, or start from what Collide found.</p>
      <div className="prompt">
        <textarea
          aria-label="Describe a task" rows={3} placeholder="Type a task, then Launch"
          value={s.promptText} onChange={(e) => d({ type: 'promptText', text: e.target.value })}
        />
        <button type="button" className="btn primary" disabled={!s.promptText.trim()} onClick={() => d({ type: 'promptLaunch' })}>Launch</button>
      </div>
      <div className="ways">
        <button type="button" className="btn" onClick={() => d({ type: 'newSession' })}>New session</button>
        <button type="button" className="btn" onClick={() => d({ type: 'addPack' })}>Add a pack</button>
        {s.packAdded && <span className="note" role="status">The prototype has no pack catalogue. The built-in pack is used.</span>}
      </div>
      <ul className="found">
        <li>claude is installed and logged in</li>
        <li>this folder is a git repository</li>
        <li>a GitHub source is available: origin is on GitHub, gh is logged in</li>
        <li>built-in packs: build-review</li>
      </ul>
    </div>
  )
}

function QueueRail() {
  const { s, d } = useApp()
  const items = queueOrder(s)
  return (
    <aside className="rail" aria-label="Queue">
      <header><h2>Queue</h2><span className="badge">{items.length}</span></header>
      <ul>
        {items.slice(0, 8).map((q) => {
          const run = runOf(s, q.runId)
          return (
            <li key={q.id}>
              <button
                type="button" className={`railitem${run && s.hover === run.id ? ' hl' : ''}`}
                onMouseEnter={() => d({ type: 'hover', runId: q.runId })} onMouseLeave={() => d({ type: 'hover', runId: null })}
                onClick={() => { d({ type: 'nav', surface: 'queue' }); d({ type: 'queueSelect', id: q.id }) }}
              >
                <span className="k">{q.kind}</span>
                <span className="t">{q.title}</span>
                <span className="mono note">{run && <RunTag s={s} run={run} />} {fmtDur(s.simNow - q.raisedWall)}</span>
              </button>
            </li>
          )
        })}
        {!items.length && <li className="note">Nothing is waiting on you.</li>}
      </ul>
    </aside>
  )
}

function TracksStrip() {
  const { s, d } = useApp()
  const live = s.runs.filter(isLive)
  return (
    <section className="strip" aria-label="Tracks strip">
      <header><h2>Tracks</h2><span className="note">{live.length} live</span></header>
      {live.map((r) => {
        const ph = r.phases.filter((p) => p.end === null).map((p) => p.reviewer ? `review ${p.reviewer}` : p.label).join(' + ') || 'starting'
        const last = r.script[r.script.length - 1]?.at || 1
        const pct = Math.min(100, ((s.simNow - r.startWall - r.shift) / last) * 100)
        return (
          <button
            key={r.id} type="button" className={`stripitem s-${r.status}${s.hover === r.id ? ' hl' : ''}`}
            onMouseEnter={() => d({ type: 'hover', runId: r.id })} onMouseLeave={() => d({ type: 'hover', runId: null })}
            onClick={() => d({ type: 'nav', surface: 'tracks' })}
          >
            <RunTag s={s} run={r} />
            <span className="mono">{r.taskId}</span>
            <span className="ph">{ph}</span>
            <span className="prog"><i style={{ width: `${pct}%` }} /></span>
          </button>
        )
      })}
      {!live.length && <span className="note">No live runs.</span>}
    </section>
  )
}

export function Field() {
  const { s, d } = useApp()
  const drawer = s.tasks.find((t) => t.id === s.drawerTask)
  return (
    <div className="field">
      {s.tasks.length > 0 && <FilterBar />}
      <div className="fieldbody">
        <div className="board">
          {s.tasks.length === 0 ? <EmptyField /> : <Board />}
        </div>
        {drawer && (
          <aside className="drawer" aria-label="Task drawer">
            <header>
              <h2>Task</h2><span className="mono">{drawer.id}</span>
              <span className="spacer" />
              <button type="button" className="btn sm" onClick={() => d({ type: 'closeDrawer' })}>Close</button>
            </header>
            <h3>{drawer.title}</h3>
            <p>{drawer.body}</p>
            <dl>
              <dt>Collection</dt><dd className="mono">{drawer.epic || 'none'}</dd>
              <dt>Status on {s.line}</dt><dd>{statusOf(s, drawer)}</dd>
              <dt>Depends on</dt><dd className="mono">{drawer.dependsOn.join(', ') || 'nothing'}</dd>
              <dt>Blocks</dt><dd className="mono">{s.tasks.filter((t) => t.dependsOn.includes(drawer.id)).map((t) => t.id).join(', ') || 'nothing'}</dd>
              <dt>Source</dt><dd className="mono">{drawer.source}</dd>
              <dt>Runs</dt>
              <dd>
                {s.runs.filter((r) => r.taskId === drawer.id).map((r) => (
                  <button key={r.id} type="button" className="btn sm" onClick={() => d({ type: 'selectRun', runId: r.id })}>{r.id}</button>
                ))}
                {!s.runs.some((r) => r.taskId === drawer.id) && 'none'}
              </dd>
            </dl>
          </aside>
        )}
        <QueueRail />
      </div>
      <TracksStrip />
      <p className="note hint"><Kbd>1</Kbd> Field <Kbd>2</Kbd> Tracks <Kbd>3</Kbd> Queue <Kbd>4</Kbd> Experiments</p>
    </div>
  )
}
