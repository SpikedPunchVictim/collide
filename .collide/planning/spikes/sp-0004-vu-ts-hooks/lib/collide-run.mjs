// Spike S4: run a hook the way Collide's hook executor would (plan 0.5): argv only, cwd set, JSON on stdin,
// COLLIDE_RESULT and COLLIDE_REPORT set, stdout/stderr to files. Then judge the verdict and check the report.
//
//   node lib/collide-run.mjs --hook ABS.ts --cwd DIR --input FILE.json --out DIR [--report-parent ID]
//
// Prints one JSON summary on stdout.
import { spawn } from "node:child_process";
import { closeSync, existsSync, mkdirSync, openSync, readFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { performance } from "node:perf_hooks";
import { fileURLToPath } from "node:url";

const STATUSES = new Set(["ok", "fail", "refused", "error"]);
const KINDS = new Set(["span.start", "span.end", "metric", "progress"]);
const ISO_UTC = /^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(\.\d+)?Z$/;

/** Spawns the hook and resolves when it exits. Times spawn to exit, which is what a user waits for. */
export function runHook({ node = process.execPath, args, cwd, input, out, env = {}, timeoutMs = 30000 }) {
  mkdirSync(out, { recursive: true });
  const resultPath = join(out, "result.json");
  const reportPath = join(out, "report.jsonl");
  rmSync(resultPath, { force: true });
  rmSync(reportPath, { force: true });
  const stdoutFd = openSync(join(out, "stdout.log"), "w");
  const stderrFd = openSync(join(out, "stderr.log"), "w");
  const childEnv = {
    PATH: process.env.PATH,
    HOME: process.env.HOME,
    COLLIDE_RUN: "run-s4",
    COLLIDE_SPAN: "span-s4",
    COLLIDE_RESULT: resultPath,
    COLLIDE_REPORT: reportPath,
    COLLIDE_PACK_DATA: join(out, "pack-data"),
    ...env,
  };
  return new Promise((resolve) => {
    const started = performance.now();
    const child = spawn(node, args, { cwd, env: childEnv, stdio: ["pipe", stdoutFd, stderrFd] });
    const timer = setTimeout(() => child.kill("SIGKILL"), timeoutMs);
    child.stdin.on("error", () => {}); // EPIPE when a hook exits without reading stdin
    child.stdin.end(input);
    child.on("close", (code, signal) => {
      const wallMs = performance.now() - started;
      clearTimeout(timer);
      closeSync(stdoutFd);
      closeSync(stderrFd);
      resolve({ code, signal, wallMs, resultPath, reportPath, out });
    });
  });
}

/** Applies 0.5: the result file's status is the verdict; non-zero exit or a missing/invalid file is error. */
export function judge(run) {
  let result = null;
  let problem = null;
  if (!existsSync(run.resultPath)) problem = "result file missing";
  else {
    try {
      result = JSON.parse(readFileSync(run.resultPath, "utf8"));
      if (!STATUSES.has(result.status)) problem = `invalid status ${JSON.stringify(result.status)}`;
      else if (typeof result.outputs !== "object" || result.outputs === null) problem = "outputs is not an object";
    } catch (error) {
      problem = `result file is not JSON: ${error.message}`;
    }
  }
  if (run.code !== 0) problem = problem ?? `exit code ${run.code}${run.signal ? ` (${run.signal})` : ""}`;
  return { verdict: problem ? "error" : result.status, problem, result };
}

export function checkReport(reportPath) {
  if (!existsSync(reportPath)) return { lines: 0, invalid: [], kinds: {}, unclosed: [] };
  const invalid = [];
  const kinds = {};
  const open = new Map();
  const lines = readFileSync(reportPath, "utf8").split("\n").filter(Boolean);
  lines.forEach((line, index) => {
    let record;
    try {
      record = JSON.parse(line);
    } catch {
      invalid.push({ line: index + 1, why: "not JSON" });
      return;
    }
    const why = [];
    if (Buffer.byteLength(line) > 4096) why.push("over 4 KB");
    if (record.v !== 1) why.push("v is not 1");
    if (typeof record.t !== "string" || !ISO_UTC.test(record.t) || Number.isNaN(Date.parse(record.t))) why.push("bad t");
    if (!KINDS.has(record.kind)) why.push(`unknown kind ${record.kind}`);
    if (record.kind?.startsWith("span.") && !/^\d+-\d+$/.test(record.id ?? "")) why.push("span id is not <pid>-<n>");
    if (record.kind === "span.start") {
      if (open.has(record.id)) why.push("span id reused while open");
      open.set(record.id, record.name);
    }
    if (record.kind === "span.end" && !open.delete(record.id)) why.push("span.end without span.start");
    kinds[record.kind] = (kinds[record.kind] ?? 0) + 1;
    if (why.length > 0) invalid.push({ line: index + 1, why: why.join(", ") });
  });
  return { lines: lines.length, invalid, kinds, unclosed: [...open.keys()], records: lines.map((l) => JSON.parse(l)) };
}

export function stderrSummary(out) {
  const text = readFileSync(join(out, "stderr.log"), "utf8");
  const codes = [...new Set(text.match(/\b(ERR_[A-Z_]+|MODULE_TYPELESS_PACKAGE_JSON|ExperimentalWarning)\b/g) ?? [])];
  return { bytes: Buffer.byteLength(text), codes, firstLine: text.split("\n").find(Boolean)?.slice(0, 200) ?? "" };
}

function parseArgs(argv) {
  const options = {};
  for (let i = 0; i < argv.length; i += 2) options[argv[i].replace(/^--/, "")] = argv[i + 1];
  return options;
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const env = options["report-parent"] ? { COLLIDE_REPORT_PARENT: options["report-parent"] } : {};
  const run = await runHook({
    args: [options.hook], cwd: options.cwd, input: readFileSync(options.input), out: options.out, env,
  });
  const report = checkReport(run.reportPath);
  delete report.records;
  const summary = {
    ...judge(run), exitCode: run.code, signal: run.signal, wallMs: Number(run.wallMs.toFixed(1)),
    report, stderr: stderrSummary(run.out),
  };
  process.stdout.write(`${JSON.stringify(summary)}\n`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) await main();
