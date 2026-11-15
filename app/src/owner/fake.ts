// An in-memory owner behind the `OwnerClient` interface (0.2: UI tests run in a browser against a fake owner). A test
// says how each method answers with `handle`, and pushes an event to the subscribers with `push`. A call with no
// handler is refused as the daemon refuses an unknown method.
import {
  type ChannelEvent,
  METHOD_NOT_FOUND,
  type OwnerClient,
  OwnerError,
  type ParamsOf,
  type ResultOf,
  type RpcArgs,
  type Unsubscribe,
  type WindowMethod,
  toOwnerError,
} from './client.ts'

type Handler<M extends WindowMethod> = (params: ParamsOf<M>) => ResultOf<M> | Promise<ResultOf<M>>

export interface RecordedCall {
  method: string
  params: unknown
}

export class FakeOwner implements OwnerClient {
  /** Every call, in order, with its params as they would cross the wire. */
  readonly calls: RecordedCall[] = []
  private readonly handlers = new Map<string, (params: never) => unknown>()
  private readonly listeners = new Set<(event: ChannelEvent) => void>()

  /** Sets how `method` answers. A handler may throw an `OwnerError` to answer with a failure. */
  handle<M extends WindowMethod>(method: M, handler: Handler<M>): this {
    this.handlers.set(method, handler as (params: never) => unknown)
    return this
  }

  /** Sends an event to every subscriber, as the shell's channel would. */
  push(event: ChannelEvent): void {
    for (const listener of [...this.listeners]) listener(structuredClone(event))
  }

  async rpc<M extends WindowMethod>(method: M, ...args: RpcArgs<M>): Promise<ResultOf<M>> {
    const params = args[0] === undefined ? undefined : structuredClone(args[0])
    this.calls.push({ method, params })
    const handler = this.handlers.get(method)
    if (handler === undefined) {
      throw new OwnerError(METHOD_NOT_FOUND, `method not found: \`${method}\``)
    }
    try {
      return structuredClone(await handler((params ?? {}) as never)) as ResultOf<M>
    } catch (failure) {
      throw toOwnerError(failure)
    }
  }

  async subscribe(listener: (event: ChannelEvent) => void): Promise<Unsubscribe> {
    this.listeners.add(listener)
    return () => {
      this.listeners.delete(listener)
    }
  }
}
