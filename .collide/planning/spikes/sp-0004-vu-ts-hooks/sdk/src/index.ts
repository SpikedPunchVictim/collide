// Stand-in for @collide/pack-sdk (spike S4). Zero runtime dependencies: node: built-ins only.
// The contract is the files (plan 0.5): JSON on stdin, the verdict in $COLLIDE_RESULT, report lines in
// $COLLIDE_REPORT. This module is a convenience over them.
import { appendFileSync, renameSync, writeFileSync } from "node:fs";

export const SDK_VERSION = "0.1.0";
/** URL this module was loaded from. Shows whether Node resolved a symlink to its realpath. */
export const SDK_URL: string = import.meta.url;

export type Status = "ok" | "fail" | "refused" | "error";
export type Outputs = Record<string, unknown>;

export interface Task {
  key: string;
  shortId: string;
  path?: string;
  fields?: Record<string, string>;
}

export interface HookState {
  run: Record<string, unknown>;
  workspace: Record<string, unknown>;
  point: Record<string, unknown>;
}

export interface HookInput<C = Record<string, unknown>> {
  protocol: number;
  hook: string;
  pack: string;
  board: unknown;
  line: unknown;
  experiment: unknown;
  run: unknown;
  launch: unknown;
  config: C;
  tasks: Task[];
  phase: unknown;
  workspace: unknown;
  history: unknown[];
  state: HookState;
}

/** Returned by ctx.ok, ctx.fail and ctx.refuse. A handler must return one. */
export interface Verdict {
  readonly status: Status;
}

export interface SpanScope {
  /** The span this scope reports under; undefined at the top level. */
  readonly id: string | undefined;
  span<T>(name: string, fn: (span: SpanScope) => T | Promise<T>, attrs?: Record<string, unknown>): Promise<T>;
  metric(name: string, value: number, unit?: string): void;
  progress(done: number, total: number): void;
}

export interface HookContext<C> {
  readonly config: C;
  readonly state: HookState;
  readonly dataDir: string | undefined;
  readonly report: SpanScope;
  ok(outputs?: Outputs): Verdict;
  fail(message: string): Verdict;
  refuse(message: string): Verdict;
}

export type HookHandler<C> = (input: HookInput<C>, ctx: HookContext<C>) => Verdict | Promise<Verdict>;

interface ResultFile {
  status: Status;
  message: string;
  outputs: Outputs;
}

const MAX_REPORT_LINE_BYTES = 4096;
let spanCounter = 0;
let warnedNoReport = false;

function nextSpanId(): string {
  spanCounter += 1;
  return `${process.pid}-${spanCounter}`;
}

function writeReportLine(record: Record<string, unknown>): void {
  const path = process.env.COLLIDE_REPORT;
  if (!path) {
    if (!warnedNoReport) {
      process.stderr.write("collide sdk: COLLIDE_REPORT is not set; report lines are dropped\n");
      warnedNoReport = true;
    }
    return;
  }
  const line = `${JSON.stringify({ v: 1, t: new Date().toISOString(), ...record })}\n`;
  const bytes = Buffer.byteLength(line);
  if (bytes > MAX_REPORT_LINE_BYTES) {
    throw new Error(`report line is ${bytes} bytes, over the ${MAX_REPORT_LINE_BYTES}-byte cap (kind ${String(record.kind)})`);
  }
  // One appendFileSync call per line: O_APPEND, one write(2) for a line this small.
  appendFileSync(path, line);
}

function createScope(parent: string | undefined): SpanScope {
  const parentField = parent === undefined ? {} : { parent };
  return {
    id: parent,
    async span(name, fn, attrs) {
      const id = nextSpanId();
      writeReportLine({ kind: "span.start", id, name, ...parentField });
      try {
        const value = await fn(createScope(id));
        writeReportLine({ kind: "span.end", id, status: "ok", ...(attrs === undefined ? {} : { attrs }) });
        return value;
      } catch (error) {
        writeReportLine({ kind: "span.end", id, status: "fail", attrs: { ...attrs, error: String(error) } });
        throw error;
      }
    },
    metric(name, value, unit) {
      writeReportLine({ kind: "metric", name, value, ...(unit === undefined ? {} : { unit }), ...parentField });
    },
    progress(done, total) {
      writeReportLine({ kind: "progress", done, total, ...parentField });
    },
  };
}

function writeResult(result: ResultFile): void {
  const path = process.env.COLLIDE_RESULT;
  if (!path) {
    throw new Error("COLLIDE_RESULT is not set: a hook must be started by Collide (plan 0.5)");
  }
  // Write then rename, so a crash mid-write leaves no partial file (missing file = error).
  const temporary = `${path}.${process.pid}.tmp`;
  writeFileSync(temporary, JSON.stringify(result));
  renameSync(temporary, path);
}

async function readInput(): Promise<unknown> {
  const chunks: Buffer[] = [];
  for await (const chunk of process.stdin) {
    chunks.push(chunk as Buffer);
  }
  const text = Buffer.concat(chunks).toString("utf8");
  if (text.trim() === "") {
    throw new Error("no hook input on stdin (plan 0.5 sends one JSON object)");
  }
  return JSON.parse(text);
}

function describe(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

async function runHook<C>(handler: HookHandler<C>): Promise<void> {
  let verdict: Verdict | undefined;
  const decide = (status: Status, message: string, outputs: Outputs): Verdict => {
    if (verdict !== undefined) {
      throw new Error(`a verdict was already given (${verdict.status}); a hook gives exactly one`);
    }
    writeResult({ status, message, outputs });
    verdict = { status };
    return verdict;
  };

  try {
    const input = (await readInput()) as HookInput<C>;
    const ctx: HookContext<C> = {
      config: input.config,
      state: input.state,
      dataDir: process.env.COLLIDE_PACK_DATA,
      report: createScope(process.env.COLLIDE_REPORT_PARENT || undefined),
      ok: (outputs = {}) => decide("ok", "", outputs),
      fail: (message) => decide("fail", message, {}),
      refuse: (message) => decide("refused", message, {}),
    };
    await handler(input, ctx);
    if (verdict === undefined) {
      decide("error", "the handler returned without calling ctx.ok, ctx.fail or ctx.refuse", {});
      process.exitCode = 1;
    }
  } catch (error) {
    const stack = error instanceof Error && error.stack ? error.stack : describe(error);
    process.stderr.write(`collide hook error: ${stack}\n`);
    if (verdict === undefined) {
      try {
        decide("error", describe(error), {});
      } catch (writeError) {
        process.stderr.write(`collide sdk: could not write the result file: ${describe(writeError)}\n`);
      }
    }
    process.exitCode = 1;
  }
}

/** Runs the handler once: reads stdin, then writes the verdict to $COLLIDE_RESULT. */
export function defineHook<C = Record<string, unknown>>(handler: HookHandler<C>): Promise<void> {
  return runHook(handler);
}
