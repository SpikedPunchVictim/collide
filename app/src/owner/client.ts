// The window's one way to the owner (0.2, The UI speaks the owner's protocol; ts-0017-qb): `rpc(method, params)` and one
// event subscription. The app is written against `OwnerClient`; the shell's is `createTauriOwner`, and a browser test
// uses `FakeOwner`, so no test needs a daemon.
//
// There is no token here. The shell holds the per-client token and adds it when it forwards a call, so nothing in this
// interface, a params type or an option can carry one. `test/owner-types.test.ts` keeps it that way.
import type { AskNoticeOutput, OwnerNoticeOutput, UiToolsRequestOutput } from './generated.ts'
import type { ASK_METHODS, OWNER_METHODS, UI_TOOLS_METHODS } from './methods.ts'

type Methods = typeof OWNER_METHODS & typeof ASK_METHODS & typeof UI_TOOLS_METHODS

/** Every method the window may call. */
export type WindowMethod = keyof Methods

export type ParamsOf<M extends WindowMethod> = Methods[M]['params']

export type ResultOf<M extends WindowMethod> = Methods[M]['result']

/** The arguments after the method name: the params, which may be left out when every key of them is optional. */
export type RpcArgs<M extends WindowMethod> = {} extends ParamsOf<M>
  ? [params?: ParamsOf<M>]
  : [params: ParamsOf<M>]

/** The event of 0.2: an agent's `tools/call`, for the window to run and answer with `ui_tools.reply`. */
export type UiToolsRequestEvent = { method: 'ui_tools.request'; params: UiToolsRequestOutput }

/** The name of the shell's notice about the daemon's event stream. `app/src-tauri/src/shell.rs` has the same name. */
export const CONNECTION_EVENT = 'shell.connection'

/**
 * The shell's notice that the daemon's event stream ended (`lost`) or came back (`back`). On `back` the events between
 * may be gone from the daemon's log, so a window reloads its state instead of waiting for them.
 */
export type ConnectionEvent = { method: typeof CONNECTION_EVENT; params: { state: 'lost' | 'back' } }

/** What arrives on the event channel: the daemon's notices, an ask's, and the shell's own notices. */
export type ChannelEvent = OwnerNoticeOutput | AskNoticeOutput | UiToolsRequestEvent | ConnectionEvent

export type Unsubscribe = () => void

export interface OwnerClient {
  /** Calls a method and gives its result. A JSON-RPC error rejects with an `OwnerError`. */
  rpc<M extends WindowMethod>(method: M, ...args: RpcArgs<M>): Promise<ResultOf<M>>
  /** Listens to the event channel until the returned function is called. */
  subscribe(listener: (event: ChannelEvent) => void): Promise<Unsubscribe>
}

/** A JSON-RPC error of the owner (or of the shell): a code from `core`'s `CODES`, and the message to show. */
export class OwnerError extends Error {
  readonly code: number
  readonly data?: unknown

  constructor(code: number, message: string, data?: unknown) {
    super(message)
    this.name = 'OwnerError'
    this.code = code
    this.data = data
  }
}

/** The JSON-RPC code of an unknown method. */
export const METHOD_NOT_FOUND = -32601
/** The JSON-RPC code of a failure that is the receiver's own. */
export const INTERNAL_ERROR = -32603

/** Reads what a failed call rejected with: the shell sends a serialized `RpcError`. */
export function toOwnerError(failure: unknown): OwnerError {
  if (failure instanceof OwnerError) return failure
  if (typeof failure === 'object' && failure !== null && 'code' in failure && 'message' in failure) {
    const { code, message, data } = failure as { code: unknown; message: unknown; data?: unknown }
    if (typeof code === 'number' && typeof message === 'string') return new OwnerError(code, message, data)
  }
  return new OwnerError(INTERNAL_ERROR, `the shell failed the call: ${String(failure)}`)
}
