import { defineHook } from '../src/index.ts'

// A hook written with the SDK, run by `define-hook.test.ts` as a real child. The first argument picks what it does.
const behavior = process.argv[2]

await defineHook(async (input, ctx) => {
  switch (behavior) {
    case 'ok':
      return { status: 'ok', outputs: { a: 1 } }
    case 'pending':
      return { status: 'pending', message: 'not yet', retryAfter: 30 }
    case 'unverified':
      return { status: 'unverified', message: 'no network here' }
    case 'throws':
      throw new Error('the handler blew up')
    case 'nothing':
      return
    case 'bad-status':
      return { status: 'maybe' } as never
    case 'not-an-object':
      return 'ok' as never
    case 'context':
      return { status: 'ok', outputs: { hook: input.hook, state: ctx.state, config: ctx.config, dataDir: ctx.dataDir } }
    case 'data-dir-only':
      return { status: 'ok', outputs: { dataDir: ctx.dataDir } }
    default:
      throw new Error(`hook-script: unknown behavior ${behavior}`)
  }
})
