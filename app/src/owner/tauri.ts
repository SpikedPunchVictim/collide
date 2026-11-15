// The shell's owner client: one Tauri command, `rpc`, and one event channel (0.2). The arguments of the command are the
// method and the params and nothing else: the shell adds the per-client token, so the webview never holds one.
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import {
  type ChannelEvent,
  type OwnerClient,
  type ResultOf,
  type RpcArgs,
  type WindowMethod,
  toOwnerError,
} from './client.ts'

/** The command the shell registers. */
export const RPC_COMMAND = 'rpc'

/** The event channel the shell emits on. `app/src-tauri/src/lib.rs` has the same name; a test keeps the two equal. */
export const EVENT_CHANNEL = 'collide:event'

export function createTauriOwner(): OwnerClient {
  return {
    async rpc<M extends WindowMethod>(method: M, ...args: RpcArgs<M>): Promise<ResultOf<M>> {
      try {
        return await invoke<ResultOf<M>>(RPC_COMMAND, { method, params: args[0] })
      } catch (failure) {
        throw toOwnerError(failure)
      }
    },
    async subscribe(listener) {
      return listen<ChannelEvent>(EVENT_CHANNEL, (event) => listener(event.payload))
    },
  }
}
