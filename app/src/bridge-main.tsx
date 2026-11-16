// The entry of the page the browser tests load from the bridge (ts-0082-fp): the same `App`, with the owner client of
// the bridge in place of the shell's. The bridge's token is in the fragment of the address, which no request carries.
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { App } from './App'
import './index.css'
import { createBridgeOwner } from './owner/bridge.ts'

const token = new URLSearchParams(window.location.hash.slice(1)).get('token') ?? ''
const owner = createBridgeOwner({ base: window.location.origin, token })

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App owner={owner} />
  </StrictMode>,
)
