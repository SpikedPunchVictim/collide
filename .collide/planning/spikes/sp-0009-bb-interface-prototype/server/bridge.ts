// Tool calls reach the page over Vite's own HMR websocket: a custom event out, a custom event back. The browser holds the
// state, so the page runs the tool (src/agent/exec.ts) and sends the outcome.
import { randomUUID } from 'node:crypto'
import type { ToolOutcome } from '../src/agent/exec'

export const CALL_EVENT = 'collide:tool-call'
export const RESULT_EVENT = 'collide:tool-result'

/** The part of Vite's `server.ws` the bridge uses. */
export interface WsLike {
  send(payload: { type: 'custom'; event: string; data: unknown }): void
  on(event: string, handler: (data: any, client: unknown) => void): void
  clients: { size: number }
}

export interface PageLink {
  call(name: string, args: unknown, source: string): Promise<ToolOutcome>
  pages(): number
}

export function createPageLink(ws: WsLike, timeoutMs = 10_000): PageLink {
  const pending = new Map<string, (o: ToolOutcome) => void>()
  ws.on(RESULT_EVENT, (data: { id: string } & ToolOutcome) => {
    const done = pending.get(data?.id)
    if (!done) return
    pending.delete(data.id)
    done(data.ok ? { ok: true, result: data.result } : { ok: false, error: String(data.error) })
  })
  return {
    pages: () => ws.clients.size,
    call(name, args, source) {
      if (ws.clients.size === 0) {
        return Promise.resolve({ ok: false, error: 'No Collide page is open. Open http://127.0.0.1:5173/ in a browser, then call the tool again.' })
      }
      const id = randomUUID()
      return new Promise((resolve) => {
        const timer = setTimeout(() => {
          pending.delete(id)
          resolve({ ok: false, error: `The page did not answer within ${timeoutMs / 1000} s (is the tab in the background or reloading?).` })
        }, timeoutMs)
        pending.set(id, (o) => {
          clearTimeout(timer)
          resolve(o)
        })
        ws.send({ type: 'custom', event: CALL_EVENT, data: { id, name, args, source } })
      })
    },
  }
}
