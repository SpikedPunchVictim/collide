import { expectTypeOf, test } from 'vitest'
import {
  FakeOwner,
  type OwnerClient,
  type ParamsOf,
  type WindowMethod,
  createTauriOwner,
} from '../src/owner/index.ts'
import type { AskNoticeOutput, AskOutcomeOutput } from '../src/owner/generated.ts'

// These checks run in `pnpm typecheck` (vitest strips types, so the `@ts-expect-error` lines and `expectTypeOf` are
// the assertions). They show that the per-client token has no way into the webview's API: the shell adds it (0.2).

/** The methods one of whose params types has the key `K`. */
type MethodsWithKey<K extends string> = {
  [M in WindowMethod]: K extends keyof ParamsOf<M> ? M : never
}[WindowMethod]

test('no method of the window takes a token or an actor', () => {
  expectTypeOf<MethodsWithKey<'token'>>().toBeNever()
  expectTypeOf<MethodsWithKey<'actor'>>().toBeNever()
})

test('the client has no member to hold or pass a token, and the shell client takes no options', () => {
  expectTypeOf<OwnerClient>().not.toHaveProperty('token')
  expectTypeOf<keyof OwnerClient>().toEqualTypeOf<'rpc' | 'subscribe'>()
  expectTypeOf(createTauriOwner).parameters.toEqualTypeOf<[]>()
})

test('rpc takes the method and its params and nothing after them', () => {
  expectTypeOf<Parameters<OwnerClient['rpc']>[0]>().toEqualTypeOf<WindowMethod>()
  expectTypeOf<ParamsOf<'ask.start'>>().toEqualTypeOf<{ agent: string; ask: string; request: string }>()
})

// Never called: the calls would reject at run time. Each line is an error the compiler must report.
async function refused(owner: OwnerClient) {
  // @ts-expect-error `initialize` carries the token, and the shell makes the handshake, so it is not a method here.
  await owner.rpc('initialize', { protocol: 1, token: 'secret' })
  // @ts-expect-error a params type has no `token`.
  await owner.rpc('task.list', { board: 'b', token: 'secret' })
  // @ts-expect-error the window's `ask.start` has no `token` and no `endpoint`: the shell adds them.
  await owner.rpc('ask.start', { ask: 'a1', agent: 'claude', request: 'show the queue', token: 'secret' })
  // @ts-expect-error there is no options argument to pass a token in.
  await owner.rpc('daemon.status', {}, { token: 'secret' })
  // @ts-expect-error there is no method that is not in the table.
  await owner.rpc('daemon.token')
}

test('the refused calls above exist so that the typecheck sees them', () => {
  expectTypeOf(refused).toBeFunction()
  expectTypeOf(FakeOwner).toBeConstructibleWith()
})

// The ask's notices as the window reads them (ts-0201-us, carried from ts-0034-hh): the generated types, which the
// shell emits from the daemon's `ask.event` and `ask.outcome`.
type AskEnd = AskOutcomeOutput['end']

test('an ask ends in one of six ways, and a lost ask is one of them', () => {
  expectTypeOf<AskEnd['type']>().toEqualTypeOf<
    'finished' | 'cancelled' | 'timed_out' | 'stopped_at_budget' | 'failed' | 'lost'
  >()
  expectTypeOf<{ type: 'lost' }>().toExtend<AskEnd>()
  expectTypeOf<{ type: 'timed_out'; after_s: number }>().toExtend<AskEnd>()
  expectTypeOf<{ type: 'stopped_at_budget'; budget: { amount: number; currency: string } }>().toExtend<
    AskEnd
  >()
  expectTypeOf<{ type: 'failed'; text: string }>().toExtend<AskEnd>()
  // @ts-expect-error a lost ask has no text: the box writes it.
  const lost: AskEnd = { type: 'lost', text: 'gone' }
  expectTypeOf(lost).toBeObject()
})

test('an ask notice is an event or an outcome, each with the ask it belongs to', () => {
  expectTypeOf<AskNoticeOutput['method']>().toEqualTypeOf<'ask.event' | 'ask.outcome'>()
  expectTypeOf<Extract<AskNoticeOutput, { method: 'ask.outcome' }>['params']['ask']>().toBeString()
  type Outcome = Extract<AskNoticeOutput, { method: 'ask.outcome' }>
  expectTypeOf<Outcome['params']['outcome']>().toEqualTypeOf<AskOutcomeOutput>()
  expectTypeOf<Extract<AskNoticeOutput, { method: 'ask.event' }>['params']['ask']>().toBeString()
  expectTypeOf<keyof Extract<AskNoticeOutput, { method: 'ask.event' }>['params']>().toEqualTypeOf<'ask' | 'event'>()
})

test('an outcome has its end, its cost and the quality of the figures', () => {
  expectTypeOf<keyof AskOutcomeOutput>().toEqualTypeOf<'end' | 'cost' | 'quality'>()
  expectTypeOf<AskOutcomeOutput['cost']['type']>().toEqualTypeOf<'reported' | 'not_reported'>()
  expectTypeOf<AskOutcomeOutput['quality']>().toEqualTypeOf<'exact' | 'partial' | 'unreported'>()
})
