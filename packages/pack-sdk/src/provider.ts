import type {
  ApplyParamsOutput,
  ApplyResultInput,
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
import { unimplemented } from './unimplemented.ts'

type Awaitable<T> = T | Promise<T>

/** The `watch` notifications and their params (provider protocol v0). */
export interface ProviderNotifications {
  'task.collision': TaskCollisionOutput
  'source.stale': SourceStaleOutput
  'task.changed': TaskChangedOutput
  'task.renamed': RenameOutput
  'task.removed': TaskRemovedOutput
  'task.invalid': TaskInvalidOutput
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

/**
 * Serves the provider protocol on stdin and stdout, as newline-delimited JSON-RPC, until stdin closes.
 * Built for real in ts-0028-hq.
 */
export function defineProvider(handlers: ProviderHandlers): Promise<void> {
  return unimplemented('sdk: defineProvider')
}
