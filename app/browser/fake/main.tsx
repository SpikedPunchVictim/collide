// The entry of the page the browser tests load over a fake owner (ts-0179-ke): the same `App`, with a `FakeOwner` that
// plays the scripted world named in the fragment of the address (`#world=two-boards`; `test/support/worlds.ts`). There
// is no Tauri, no bridge and no daemon. `#shell=1` makes the page act as the shell's, with the folder picker
// (ts-0191-nv). `window.collideFake` is the test's one view into the fake: the methods called, in order, as the client
// sent them, the views `task.list` was asked for (ts-0187-hb), the links the shell was asked to open (ts-0183-ee), and
// (ts-0182-ad) the way to change a Field world and announce it.
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { App } from '../../src/App.tsx'
import '../../src/index.css'
import { FieldWorld } from '../../test/support/field-world.ts'
import { WORLDS } from '../../test/support/worlds.ts'

declare global {
  interface Window {
    collideFake: {
      calls(): string[]
      /** The `view` each `task.list` was asked for, in order: undefined for the whole board (ts-0187-hb). */
      views(): unknown[]
      /** Changes a listed task of a Field world, as an edit in a checkout would (nothing is announced). */
      edit(key: string, over: Record<string, unknown>): void
      /** The daemon's event that a task of `source` changed (a Field world). */
      announce(source: string, key?: string): void
      /** Sets what `task.changes` answers for a task of a Field world (nothing is announced) (ts-0189-fg). */
      setChanges(key: string, changes: unknown): void
      /** The daemon's event for a run of a Field world: `run.transition`, or `phase.transition` (ts-0189-fg). */
      announceRun(run: string, event?: 'run.transition' | 'phase.transition'): void
      /** The shell's notice that the daemon's event stream ended (`lost`) or came back (`back`). */
      connection(state: 'lost' | 'back'): void
      /** The links `shell.open_external` was asked to open, in order. The fake opens none (ts-0183-ee). */
      opened(): string[]
      /** What the next `shell.pick_folder` answers: the folder the person chose, or null for a cancel (ts-0191-nv). */
      pickNext(path: string | null): void
    }
  }
}

const name = new URLSearchParams(window.location.hash.slice(1)).get('world') ?? ''
const make = Object.hasOwn(WORLDS, name) ? WORLDS[name] : undefined
if (make === undefined) {
  throw new Error(`no fake world named "${name}"; the worlds are ${Object.keys(WORLDS).join(', ')}`)
}
const world = make()
function fieldWorld(): FieldWorld {
  if (!(world instanceof FieldWorld)) throw new Error(`the world "${name}" is not a Field world`)
  return world
}
// The shell's own method is answered here, as the shell answers it, and the call is kept for the test to read.
const opened: string[] = []
world.owner.handle('shell.open_external', ({ url }) => {
  opened.push(url)
  return {}
})
// The shell's folder picker, as the fake answers it: only a page told it is the shell's (`#shell=1`) shows it.
let nextPick: string | null = null
world.owner.handle('shell.pick_folder', () => ({ path: nextPick }))
window.collideFake = {
  calls: () => world.owner.calls.map((call) => call.method),
  views: () =>
    world.owner.calls
      .filter((call) => call.method === 'task.list')
      .map((call) => (call.params as { view?: unknown } | undefined)?.view),
  edit: (key, over) => fieldWorld().edit(key, over),
  announce: (source, key) => fieldWorld().announce(source, key),
  setChanges: (key, changes) => fieldWorld().changes.set(key, changes as never),
  announceRun: (run, event) => fieldWorld().announceRun(run, event),
  connection: (state) => world.owner.push({ method: 'shell.connection', params: { state } }),
  opened: () => [...opened],
  pickNext: (path) => {
    nextPick = path
  },
}

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App owner={world.owner} shell={new URLSearchParams(window.location.hash.slice(1)).get('shell') === '1'} />
  </StrictMode>,
)
