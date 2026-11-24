// The entry of the page the browser tests load over a fake owner (ts-0179-ke): the same `App`, with a `FakeOwner` that
// plays the scripted world named in the fragment of the address (`#world=two-boards`; `test/support/worlds.ts`). There
// is no Tauri, no bridge and no daemon. `window.collideFake` is the test's one view into the fake: the methods called,
// in order, as the client sent them.
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { App } from '../../src/App.tsx'
import '../../src/index.css'
import { WORLDS } from '../../test/support/worlds.ts'

declare global {
  interface Window {
    collideFake: { calls(): string[] }
  }
}

const name = new URLSearchParams(window.location.hash.slice(1)).get('world') ?? ''
const make = Object.hasOwn(WORLDS, name) ? WORLDS[name] : undefined
if (make === undefined) {
  throw new Error(`no fake world named "${name}"; the worlds are ${Object.keys(WORLDS).join(', ')}`)
}
const world = make()
window.collideFake = { calls: () => world.owner.calls.map((call) => call.method) }

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App owner={world.owner} />
  </StrictMode>,
)
