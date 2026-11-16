// The page's hook: one `Loop` for an owner client, opened when the page mounts and closed when it goes. React only
// reads the state and calls the methods; the work is in `loop.ts`.
import { useEffect, useMemo, useSyncExternalStore } from 'react'
import type { OwnerClient } from '../owner/client.ts'
import { type Loop, createLoop } from './loop.ts'
import type { LoopState } from './state.ts'

export function useLoop(owner: OwnerClient): { state: LoopState; loop: Loop } {
  const loop = useMemo(() => createLoop(owner), [owner])
  const state = useSyncExternalStore(loop.subscribe.bind(loop), loop.getState.bind(loop), loop.getState.bind(loop))
  useEffect(() => {
    void loop.open()
    return () => loop.close()
  }, [loop])
  return { state, loop }
}
