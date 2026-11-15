import type { HookContext } from './context.ts'
import type { HookInputOutput, HookResultInput } from './generated/index.ts'
import { unimplemented } from './unimplemented.ts'

/**
 * A hook: reads the input, and ends with a result. It returns a result (`pending` and `unverified` need this, as
 * `ctx` has no method for them), or `void` after `ctx.ok`, `ctx.fail` or `ctx.refuse` has written one.
 */
export type HookHandler = (input: HookInputOutput, ctx: HookContext) => Promise<HookResultInput | void>

/** Runs `handler` as the hook: reads stdin, builds the context, writes the result file (0.5, 0.6). */
export function defineHook(handler: HookHandler): Promise<void> {
  return unimplemented('sdk: defineHook')
}
