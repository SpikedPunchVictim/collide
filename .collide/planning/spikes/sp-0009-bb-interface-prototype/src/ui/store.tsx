import { createContext, useCallback, useContext, useEffect, useReducer, type ReactNode } from 'react'
import { reduce, type Action } from '../reducer'
import { useToolBridge } from '../agent/page'
import { initialState } from '../world'
import type { State } from '../types'

interface Ctx {
  s: State
  d: (a: Action) => void
}

const AppCtx = createContext<Ctx | null>(null)

export function Provider({ children, facilitator = false, initial, clock = true }: {
  children: ReactNode
  facilitator?: boolean
  initial?: State
  /** tests turn the real-time clock off and advance the sim by hand */
  clock?: boolean
}) {
  const [s, dispatch] = useReducer(reduce, undefined, () => initial ?? initialState('orchard', facilitator))
  const d = useCallback((a: Action) => dispatch({ ...a, t: Date.now() }), [])
  useToolBridge(s, d)
  useEffect(() => {
    if (!clock) return
    const id = setInterval(() => dispatch({ type: 'tick', t: Date.now() }), 250)
    return () => clearInterval(id)
  }, [clock])
  return <AppCtx.Provider value={{ s, d }}>{children}</AppCtx.Provider>
}

export function useApp(): Ctx {
  const c = useContext(AppCtx)
  if (!c) throw new Error('useApp outside Provider')
  return c
}
