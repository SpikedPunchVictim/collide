import type { Readable, Writable } from 'node:stream'
import type {
  ApplyParamsOutput,
  ApplyResultInput,
  CollectionsChangedInput,
  GetParamsOutput,
  GetResultInput,
  InitializeParamsOutput,
  InitializeResultInput,
  ListParamsOutput,
  ListResultInput,
  PreviewResultInput,
  SourceStaleOutput,
  TaskChangedOutput,
  TaskCollisionOutput,
  TaskInvalidOutput,
  TaskRemovedOutput,
  RenameOutput,
  WatchParamsOutput,
  WatchResultInput,
} from './generated/index.ts'

type Awaitable<T> = T | Promise<T>

/**
 * The `watch` notifications and their params (provider protocol v0). `collections.changed` is for a source that
 * declares `collections`: it says what its collections, or what a task is in, changed to with no change to the task.
 */
export interface ProviderNotifications {
  'task.collision': TaskCollisionOutput
  'source.stale': SourceStaleOutput
  'task.changed': TaskChangedOutput
  'task.renamed': RenameOutput
  'task.removed': TaskRemovedOutput
  'task.invalid': TaskInvalidOutput
  'collections.changed': CollectionsChangedInput
}

/** Sends one notification to Collide. */
export type Notify = <M extends keyof ProviderNotifications>(method: M, params: ProviderNotifications[M]) => void

/** One function per JSON-RPC method; `watch` and `preview` are optional (0.10). */
export interface ProviderHandlers {
  initialize(params: InitializeParamsOutput): Awaitable<InitializeResultInput>
  list(params: ListParamsOutput): Awaitable<ListResultInput>
  get(params: GetParamsOutput): Awaitable<GetResultInput>
  apply(params: ApplyParamsOutput): Awaitable<ApplyResultInput>
  watch?(params: WatchParamsOutput, notify: Notify): Awaitable<WatchResultInput>
  /** `preview` takes `apply`'s params. */
  preview?(params: ApplyParamsOutput): Awaitable<PreviewResultInput>
}

/** Where the server reads and writes; the defaults are the process's stdin, stdout and stderr. */
export interface ProviderOptions {
  input?: Readable
  output?: Writable
  /** Where handler errors and other diagnostics go. Stdout carries messages only. */
  log?: (message: string) => void
}

type Id = string | number | null

const PARSE_ERROR = -32700
const INVALID_REQUEST = -32600
const METHOD_NOT_FOUND = -32601
const INVALID_PARAMS = -32602
const INTERNAL_ERROR = -32603

/** A failure that becomes a JSON-RPC error response. */
class RpcError extends Error {
  readonly code: number
  constructor(code: number, message: string) {
    super(message)
    this.code = code
  }
}

const isObject = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null && !Array.isArray(value)

const errorText = (error: unknown): string => (error instanceof Error ? error.message : String(error))

/**
 * Serves the provider protocol as newline-delimited JSON-RPC 2.0 (one message per line, stdout carries messages
 * only, stderr is for logs) until the input ends, then resolves once the handler in flight has answered.
 *
 * Requests are handled one at a time, in the order they arrive, and answered in that order. Any method may come
 * first: the SDK does not require `initialize`. `params` must be an object (absent means `{}`) and is otherwise
 * passed to the handler as received: the SDK does not validate it against the generated schemas. A method the pack
 * has no handler for, including an absent `watch` or `preview`, is "method not found". A handler that throws gets an
 * error response (-32603) and the server keeps serving. A message with no `id` is a notification: it is handled and
 * never answered. A `watch` handler must return once it is watching, since the next request waits for it; its
 * `notify` may be called at any time after that.
 */
export function defineProvider(handlers: ProviderHandlers, options: ProviderOptions = {}): Promise<void> {
  const input = options.input ?? process.stdin
  const output = options.output ?? process.stdout
  const log = options.log ?? ((message: string) => void process.stderr.write(`${message}\n`))

  const send = (message: object): void => {
    output.write(`${JSON.stringify(message)}\n`)
  }
  const respondError = (id: Id, error: RpcError): void => {
    send({ jsonrpc: '2.0', id, error: { code: error.code, message: error.message } })
  }
  const notify: Notify = (method, params) => send({ jsonrpc: '2.0', method, params })

  const call = (method: string, params: unknown): unknown => {
    switch (method) {
      case 'initialize':
        return handlers.initialize(params as InitializeParamsOutput)
      case 'list':
        return handlers.list(params as ListParamsOutput)
      case 'get':
        return handlers.get(params as GetParamsOutput)
      case 'apply':
        return handlers.apply(params as ApplyParamsOutput)
      case 'watch':
        if (handlers.watch) return handlers.watch(params as WatchParamsOutput, notify)
        break
      case 'preview':
        if (handlers.preview) return handlers.preview(params as ApplyParamsOutput)
        break
    }
    throw new RpcError(METHOD_NOT_FOUND, `Method not found: ${method}`)
  }

  const handleLine = async (line: string): Promise<void> => {
    let message: unknown
    try {
      message = JSON.parse(line)
    } catch (error) {
      respondError(null, new RpcError(PARSE_ERROR, `Parse error: ${errorText(error)}`))
      return
    }
    const request = isObject(message) ? message : {}
    const hasId = 'id' in request
    const rawId = hasId ? request.id : null
    const validId = rawId === null || typeof rawId === 'string' || typeof rawId === 'number'
    const id: Id = validId ? rawId : null
    if (request.jsonrpc !== '2.0' || typeof request.method !== 'string' || !validId) {
      respondError(id, new RpcError(INVALID_REQUEST, 'Invalid request: expected one JSON-RPC 2.0 request or notification object per line'))
      return
    }
    const method = request.method
    try {
      if (request.params !== undefined && !isObject(request.params)) throw new RpcError(INVALID_PARAMS, 'Invalid params: expected an object')
      const result = await call(method, request.params ?? {})
      if (!hasId) return
      if (result === undefined) throw new Error(`the ${method} handler returned no result`)
      send({ jsonrpc: '2.0', id, result })
    } catch (error) {
      log(`${method}: ${error instanceof Error ? (error.stack ?? error.message) : String(error)}`)
      if (!hasId) return
      respondError(id, error instanceof RpcError ? error : new RpcError(INTERNAL_ERROR, errorText(error)))
    }
  }

  const onOutputError = (error: Error): void => log(`output: ${error.message}`)

  return new Promise<void>((resolve, reject) => {
    let buffer = ''
    let queue: Promise<void> = Promise.resolve()
    let finished = false
    const enqueue = (line: string): void => {
      const text = line.endsWith('\r') ? line.slice(0, -1) : line
      if (text.trim() !== '') queue = queue.then(() => handleLine(text))
    }
    const finish = (): void => {
      if (finished) return
      finished = true
      enqueue(buffer)
      queue.then(resolve, reject)
    }
    output.on('error', onOutputError)
    input.setEncoding('utf8')
    input.on('data', (chunk: string) => {
      buffer += chunk
      const lines = buffer.split('\n')
      buffer = lines.pop() ?? ''
      lines.forEach(enqueue)
    })
    input.on('error', (error) => {
      finished = true
      reject(error)
    })
    input.on('end', finish)
    input.on('close', finish)
  }).finally(() => output.off('error', onOutputError))
}
