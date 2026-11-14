// The page's half of the agent: the tool bridge (HMR websocket) and the ask request. Neither runs in tests; they are thin.
import { useEffect, useRef } from 'react'
import { reduce, type Action } from '../reducer'
import type { State } from '../types'
import { execTool } from './exec'

interface CallData {
  id: string
  name: string
  args: unknown
  source: string
}

/** Run each tool call the dev server forwards against live state, dispatch its action, send the outcome back. */
export function useToolBridge(state: State, dispatch: (a: Action) => void): void {
  const shadow = useRef<State>(state)
  useEffect(() => {
    shadow.current = state
  }, [state])
  useEffect(() => {
    const hot = import.meta.hot
    if (!hot) return
    const onCall = (c: CallData) => {
      // a second call can arrive before React has rendered the first one's action; the shadow keeps the order
      const base = shadow.current
      const out = execTool(base, c.name, c.args, c.source)
      if (out.ok && out.action) {
        shadow.current = reduce(base, { ...out.action, t: Date.now() })
        dispatch(out.action)
      }
      hot.send('collide:tool-result', out.ok ? { id: c.id, ok: true, result: out.result } : { id: c.id, ok: false, error: out.error })
    }
    hot.on('collide:tool-call', onCall)
    return () => hot.off?.('collide:tool-call', onCall)
  }, [dispatch])
}

type Ev =
  | { type: 'start'; agent: string; argv: string[] }
  | { type: 'line'; text: string }
  | { type: 'tool'; name: string; summary: string; ok: boolean; error?: string; ms: number }
  | { type: 'end'; ok: boolean; text: string; error: string; usage: State['agent']['ask']['usage']; calls: number; changes: number }

/** Send a request to the dev server, which launches the configured agent; relay its events as actions. */
export async function runAsk(request: string, agentId: string | null, d: (a: Action) => void, signal: AbortSignal): Promise<void> {
  let ended = false
  try {
    const res = await fetch('/__agent/ask', {
      method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ request, agentId }), signal,
    })
    if (!res.ok || !res.body) {
      const text = await res.text()
      let msg = text
      try {
        msg = (JSON.parse(text) as { error?: string }).error ?? text
      } catch {
        /* plain text */
      }
      d({ type: 'askEnd', ok: false, error: `The server refused the request (${res.status}): ${msg}`, changes: 0 })
      return
    }
    const reader = res.body.getReader()
    const dec = new TextDecoder()
    let buf = ''
    for (;;) {
      const { done, value } = await reader.read()
      if (done) break
      buf += dec.decode(value, { stream: true })
      let i: number
      while ((i = buf.indexOf('\n')) >= 0) {
        const line = buf.slice(0, i).trim()
        buf = buf.slice(i + 1)
        if (!line) continue
        const ev = JSON.parse(line) as Ev
        if (ev.type === 'start') d({ type: 'askLine', text: `launched ${ev.agent}` })
        else if (ev.type === 'line') d({ type: 'askLine', text: ev.text })
        else if (ev.type === 'tool') d({ type: 'askLine', tool: true, text: `${ev.summary} ${ev.ok ? 'ok' : `failed: ${ev.error}`} (${ev.ms} ms)` })
        else if (ev.type === 'end') {
          ended = true
          d({ type: 'askEnd', ok: ev.ok, text: ev.text, error: ev.error, usage: ev.usage ?? undefined, calls: ev.calls, changes: ev.changes })
        }
      }
    }
    if (!ended) d({ type: 'askEnd', ok: false, error: 'The connection closed before the agent finished.', changes: 0 })
  } catch (e) {
    if (signal.aborted) d({ type: 'askEnd', ok: false, error: 'Cancelled.', changes: 0 })
    else d({ type: 'askEnd', ok: false, error: `Could not reach the dev server: ${e instanceof Error ? e.message : String(e)}`, changes: 0 })
  }
}

export async function loadAgentSettings(d: (a: Action) => void): Promise<void> {
  try {
    const res = await fetch('/__settings/agents')
    if (!res.ok) throw new Error(`HTTP ${res.status}`)
    d({ type: 'settingsLoaded', settings: await res.json() })
  } catch (e) {
    d({ type: 'settingsError', error: `Settings need the dev server (pnpm dev): ${e instanceof Error ? e.message : String(e)}` })
  }
}

export async function saveAgentSettings(settings: unknown, d: (a: Action) => void): Promise<void> {
  try {
    const res = await fetch('/__settings/agents', { method: 'PUT', headers: { 'content-type': 'application/json' }, body: JSON.stringify(settings) })
    const body = await res.json()
    if (!res.ok) d({ type: 'settingsError', error: (body as { error?: string }).error ?? `HTTP ${res.status}` })
    else d({ type: 'settingsSaved' })
  } catch (e) {
    d({ type: 'settingsError', error: String(e) })
  }
}
