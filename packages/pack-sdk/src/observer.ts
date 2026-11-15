import type { ObserverEventOutput } from './generated/index.ts'
import { unimplemented } from './unimplemented.ts'

/** The SDK writes the acknowledgement, `{seq}`, when this returns (0.6); the handler returns nothing. */
export type ObserverHandler = (event: ObserverEventOutput) => Promise<void> | void

/** Reads events as JSONL on stdin and acknowledges each one on stdout when `handler` returns. */
export function defineObserver(handler: ObserverHandler): Promise<void> {
  return unimplemented('sdk: defineObserver')
}
