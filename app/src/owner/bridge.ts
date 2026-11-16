// The owner client of the browser tests (ts-0082-fp): the same two things as `tauri.ts`, `rpc` and one event channel,
// over the loopback HTTP of the test-only bridge (`src-tauri/src/bin/collide-bridge/`). Only `bridge-main.tsx` uses
// it, and that entry is built by the test build alone (`vite.bridge.config.ts`), so nothing of it is in the app or its
// bundle.
//
// The token is the one the bridge made for this run. It is sent as a bearer, and it is the page's own to hold: the
// webview has none, because the shell adds it, but a browser page is not behind a shell.
import {
  type ChannelEvent,
  INTERNAL_ERROR,
  type OwnerClient,
  OwnerError,
  type ResultOf,
  type RpcArgs,
  type WindowMethod,
} from './client.ts'

export interface BridgeOptions {
  /** The origin of the bridge, `http://127.0.0.1:<port>`. */
  readonly base: string
  readonly token: string
}

/** The body of an answer of `POST /rpc`: a result or an error of the owner's protocol. */
type RpcAnswer = { result: unknown } | { error: { code: number; message: string; data?: unknown } }

export function createBridgeOwner({ base, token }: BridgeOptions): OwnerClient {
  const headers = { authorization: `Bearer ${token}` }
  return {
    async rpc<M extends WindowMethod>(method: M, ...args: RpcArgs<M>): Promise<ResultOf<M>> {
      const response = await fetch(`${base}/rpc`, {
        method: 'POST',
        headers: { ...headers, 'content-type': 'application/json' },
        body: JSON.stringify({ method, params: args[0] }),
      })
      if (!response.ok) throw new OwnerError(INTERNAL_ERROR, `the bridge refused the call: ${response.status}`)
      const answer = (await response.json()) as RpcAnswer
      if ('error' in answer) throw new OwnerError(answer.error.code, answer.error.message, answer.error.data)
      return answer.result as ResultOf<M>
    },

    async subscribe(listener) {
      const stop = new AbortController()
      const response = await fetch(`${base}/events`, { headers, signal: stop.signal })
      if (!response.ok || response.body === null) {
        throw new OwnerError(INTERNAL_ERROR, `the bridge refused the event channel: ${response.status}`)
      }
      void readEvents(response.body, listener).catch(() => undefined)
      return () => stop.abort()
    },
  }
}

/** Reads the stream of server-sent events and calls `listener` with each `data:` line's JSON. */
export async function readEvents(body: ReadableStream<Uint8Array>, listener: (event: ChannelEvent) => void) {
  const reader = body.getReader()
  const decoder = new TextDecoder()
  let pending = ''
  for (;;) {
    const { done, value } = await reader.read()
    if (done) return
    pending += decoder.decode(value, { stream: true })
    for (let end = pending.indexOf('\n\n'); end !== -1; end = pending.indexOf('\n\n')) {
      const block = pending.slice(0, end)
      pending = pending.slice(end + 2)
      for (const line of block.split('\n')) {
        // A line that starts with a colon is a comment: the bridge's keep-alive.
        if (line.startsWith('data: ')) listener(JSON.parse(line.slice('data: '.length)) as ChannelEvent)
      }
    }
  }
}
