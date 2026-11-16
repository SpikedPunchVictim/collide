import { LoopPage } from './loop/LoopPage.tsx'
import { useLoop } from './loop/useLoop.ts'
import { type OwnerClient, createTauriOwner } from './owner/index.ts'

// Until Stage 6a brings the tabs, the window is the one page of the thin loop (Stage 3c). `AppShell` and its surfaces
// stay in the tree and are tested; Stage 6a shows them again, over the owner client.
const shellOwner = createTauriOwner()

export interface AppProps {
  /** The way to the daemon. The shell's, unless a test gives another. */
  readonly owner?: OwnerClient
}

export function App({ owner = shellOwner }: AppProps) {
  const { state, loop } = useLoop(owner)
  return <LoopPage state={state} actions={loop} nowMs={Date.now()} />
}
