import type { HookInputOutput, HookResultInput, ReportLineOutput } from './generated/index.ts'
import { unimplemented } from './unimplemented.ts'

type SpanStart = Extract<ReportLineOutput, { kind: 'span.start' }>
type Metric = Extract<ReportLineOutput, { kind: 'metric' }>
type Progress = Extract<ReportLineOutput, { kind: 'progress' }>

/** `ctx.report`: appends report lines to the file named by `COLLIDE_REPORT` (0.5, Reporting). */
export interface HookReport {
  /** Writes `span.start`, runs `run`, then writes `span.end` with `ok` or `error`. Returns what `run` returns. */
  span<T>(name: string, run: () => T | Promise<T>, attrs?: NonNullable<SpanStart['attrs']>): Promise<T>
  /** Writes a `metric` line. */
  metric(name: Metric['name'], value: Metric['value'], unit?: NonNullable<Metric['unit']>): void
  /** Writes a `progress` line. */
  progress(done: Progress['done'], total?: NonNullable<Progress['total']>): void
}

/**
 * `ctx.ok`, `ctx.fail` and `ctx.refuse` write the result file (`COLLIDE_RESULT`) and return what they wrote, so a
 * handler can end with `return ctx.ok(outputs)`.
 */
export interface HookResultWriter {
  ok(outputs?: NonNullable<HookResultInput['outputs']>): Promise<HookResultInput>
  fail(message: NonNullable<HookResultInput['message']>): Promise<HookResultInput>
  refuse(message: NonNullable<HookResultInput['message']>): Promise<HookResultInput>
}

/**
 * The context a hook handler gets (0.6): a convenience over stdin, the result file and the report file, which
 * are the contract.
 */
export interface HookContext extends HookResultWriter {
  /** The `state` of the hook input: the workspace's and the run's outputs so far, and the point's. */
  readonly state: HookInputOutput['state']
  /** The pack's settings, from the hook input. */
  readonly config: HookInputOutput['config']
  /** The pack's data directory for this board (`COLLIDE_PACK_DATA`, 0.7). */
  readonly dataDir: string
  readonly report: HookReport
}

/** Builds `state`, `config` and `dataDir` from the hook input and the environment. */
export function createHookContext(input: HookInputOutput): HookContext {
  return unimplemented('sdk: ctx')
}

/** Builds `ctx.report`, over the file named by `COLLIDE_REPORT`. Span ids are `<pid>-<counter>` (0.5). */
export function createReport(): HookReport {
  return unimplemented('sdk: ctx.report')
}

/** Builds `ctx.ok`, `ctx.fail` and `ctx.refuse`, over the file named by `COLLIDE_RESULT`. */
export function createResultWriter(): HookResultWriter {
  return unimplemented('sdk: ctx.result')
}
