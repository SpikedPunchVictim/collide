import { expect, test } from 'vitest'
import { defineHook, defineObserver, defineProvider } from '../src/index.ts'
import type {
  ApplyParamsOutput,
  ApplyResultInput,
  GetParamsOutput,
  GetResultInput,
  HookContext,
  HookHandler,
  HookInputOutput,
  HookResultInput,
  InitializeParamsOutput,
  InitializeResultInput,
  ListParamsOutput,
  ListResultInput,
  ObserverEventOutput,
  ObserverHandler,
  PreviewResultInput,
  ProviderHandlers,
  WatchParamsOutput,
  WatchResultInput,
} from '../src/index.ts'

// These checks run in `pnpm typecheck`: vitest strips the types, so the `@ts-expect-error` lines are the assertions.
// Nothing here calls a `define*` function: they are stubs until their stages.

type Equal<A, B> = (<T>() => T extends A ? 1 : 2) extends <T>() => T extends B ? 1 : 2 ? true : false
const same = <A, B>(_proof: Equal<A, B>) => undefined

test('handler parameters and results are the generated types, not copies', () => {
  same<Parameters<HookHandler>[0], HookInputOutput>(true)
  same<Awaited<ReturnType<HookHandler>>, HookResultInput | void>(true)
  same<Parameters<ProviderHandlers['initialize']>[0], InitializeParamsOutput>(true)
  same<Awaited<ReturnType<ProviderHandlers['initialize']>>, InitializeResultInput>(true)
  same<Parameters<ProviderHandlers['list']>[0], ListParamsOutput>(true)
  same<Awaited<ReturnType<ProviderHandlers['list']>>, ListResultInput>(true)
  same<Parameters<ProviderHandlers['get']>[0], GetParamsOutput>(true)
  same<Awaited<ReturnType<ProviderHandlers['get']>>, GetResultInput>(true)
  same<Parameters<ProviderHandlers['apply']>[0], ApplyParamsOutput>(true)
  same<Awaited<ReturnType<ProviderHandlers['apply']>>, ApplyResultInput>(true)
  const watch = {} as NonNullable<ProviderHandlers['watch']>
  same<Parameters<typeof watch>[0], WatchParamsOutput>(true)
  same<Awaited<ReturnType<typeof watch>>, WatchResultInput>(true)
  const preview = {} as NonNullable<ProviderHandlers['preview']>
  same<Parameters<typeof preview>[0], ApplyParamsOutput>(true)
  same<Awaited<ReturnType<typeof preview>>, PreviewResultInput>(true)
  same<Parameters<ObserverHandler>[0], ObserverEventOutput>(true)
  // @ts-expect-error the proof helper itself can fail: a hook input is not an apply result.
  same<Parameters<HookHandler>[0], ApplyResultInput>(true)
  expect(true).toBe(true)
})

test('a hook handler returning the wrong result type does not compile', () => {
  const pending: HookHandler = async () => ({ status: 'pending', retryAfter: 30 })
  const nothing: HookHandler = async () => {}
  // @ts-expect-error a status outside the six.
  const maybe: HookHandler = async () => ({ status: 'maybe' })
  // @ts-expect-error a result needs a status.
  const verdict: HookHandler = async () => ({ message: 'fine' })
  // @ts-expect-error a handler answers with a result, not a string.
  const text: HookHandler = async () => 'ok'
  expect([pending, nothing, maybe, verdict, text]).toHaveLength(5)
})

test('the context has the members the plan names, typed from the hook input', () => {
  const handler: HookHandler = async (input, ctx) => {
    const hook: string = input.hook
    const config: HookInputOutput['config'] = ctx.config
    const state: HookInputOutput['state'] = ctx.state
    const dataDir: string = ctx.dataDir
    await ctx.report.span('build', async () => 1, { cache: 'miss' })
    ctx.report.metric('tests.passed', 412, 'count')
    ctx.report.progress(3, 10)
    expect([hook, config, state, dataDir]).toHaveLength(4)
    return ctx.ok({ branch: 'main' })
  }
  expect(handler).toBeTypeOf('function')
})

test('ctx.ok, ctx.fail, ctx.refuse and ctx.report reject wrong arguments', () => {
  const check = (ctx: HookContext) => {
    // @ts-expect-error outputs are an object, not a number.
    ctx.ok(5)
    // @ts-expect-error a message is a string.
    ctx.fail(5)
    // @ts-expect-error a message is a string.
    ctx.refuse({ reason: 'no' })
    // @ts-expect-error a metric value is a number.
    ctx.report.metric('tests.passed', 'many')
    // @ts-expect-error progress counts are numbers.
    ctx.report.progress('3')
    // @ts-expect-error `ctx.dataDir` is read-only data, a string and not a function.
    ctx.dataDir()
  }
  expect(check).toBeTypeOf('function')
})

test('provider handlers answer in the generated result types', () => {
  const refused: ProviderHandlers['apply'] = async () => ({ outcome: 'refused', message: 'illegal transition' })
  // @ts-expect-error a refusal carries the source's message.
  const noMessage: ProviderHandlers['apply'] = async () => ({ outcome: 'refused' })
  // @ts-expect-error an outcome outside ok, conflict and refused.
  const later: ProviderHandlers['apply'] = async () => ({ outcome: 'later' })
  // @ts-expect-error `list` answers with a list result, not a number.
  const list: ProviderHandlers['list'] = async () => 5
  const preview: NonNullable<ProviderHandlers['preview']> = async () => ({ outcome: 'ok', items: [], files: [] })
  // @ts-expect-error preview answers ok, conflict or refused, like apply.
  const badPreview: NonNullable<ProviderHandlers['preview']> = async () => ({ outcome: 'maybe' })
  expect([refused, noMessage, later, list, preview, badPreview]).toHaveLength(6)
})

test('a provider must handle initialize, list, get and apply; watch and preview are optional', () => {
  const initialize = {} as ProviderHandlers['initialize']
  const list = {} as ProviderHandlers['list']
  const get = {} as ProviderHandlers['get']
  const apply = {} as ProviderHandlers['apply']
  const define = () => {
    defineProvider({ initialize, list, get, apply })
    // @ts-expect-error `get` is required.
    defineProvider({ initialize, list, apply })
    // @ts-expect-error `export` is not a provider method.
    defineProvider({ initialize, list, get, apply, export: apply })
    // @ts-expect-error a handler with the wrong result type is refused where it is passed.
    defineProvider({ initialize, list, get, apply, preview: async () => 5 })
  }
  expect(define).toBeTypeOf('function')
})

test('an observer handler gets an event and returns nothing; the SDK acknowledges it', () => {
  const seen: ObserverHandler = async (event) => {
    const seq: number = event.seq
    expect(seq).toBeTypeOf('number')
  }
  // @ts-expect-error an observer handler returns nothing: the acknowledgement is the SDK's.
  const answers: ObserverHandler = async () => 5
  const define = () => {
    defineObserver(seen)
    // @ts-expect-error a handler takes the event, not a string.
    defineObserver(async (event: string) => {})
  }
  const hook = () => defineHook(async (_input, ctx) => ctx.ok({}))
  expect([answers, define, hook]).toHaveLength(3)
})
