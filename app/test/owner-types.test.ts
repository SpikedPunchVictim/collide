import { expectTypeOf, test } from 'vitest'
import {
  FakeOwner,
  type OwnerClient,
  type ParamsOf,
  type WindowMethod,
  createTauriOwner,
} from '../src/owner/index.ts'

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
