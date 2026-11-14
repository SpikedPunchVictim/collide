// Spike S4: capture the user's login-shell environment the way Collide would when launched from Finder,
// then find node, claude, pnpm, gh (and git, cargo) on the captured PATH. Prints names and PATH only:
// other environment values can hold secrets.
//
//   node lib/env-capture.mjs [--runs 5] [--timeout-ms 10000] [--json]
//   ZDOTDIR=... node lib/env-capture.mjs ...   (synthetic rc files, to test the delimiting)
import { spawnSync } from "node:child_process";
import { randomBytes } from "node:crypto";
import { accessSync, constants, statSync } from "node:fs";
import { userInfo } from "node:os";
import { join } from "node:path";
import { performance } from "node:perf_hooks";

const TOOLS = ["node", "claude", "pnpm", "gh", "git", "cargo"];
const MODES = [
  { name: "login (-l -c)", flags: ["-l", "-c"] },
  { name: "login+interactive (-l -i -c)", flags: ["-l", "-i", "-c"] },
];

function parseArgs(argv) {
  const options = {};
  for (let i = 0; i < argv.length; i += 1) {
    const key = argv[i].replace(/^--/, "");
    if (key === "json") options.json = true;
    else { options[key] = argv[i + 1]; i += 1; }
  }
  return options;
}

/** The login shell: $SHELL if the launcher set it, else the user database (getpwuid), which needs no env. */
function loginShell() {
  if (process.env.SHELL) return { shell: process.env.SHELL, source: "$SHELL" };
  return { shell: userInfo().shell, source: "getpwuid (no $SHELL in the launch env)" };
}

/** Runs the shell once. Markers carry a fresh nonce; env -0 separates entries with NUL, so values may hold newlines. */
function captureOnce(shell, flags, timeoutMs) {
  const nonce = randomBytes(8).toString("hex");
  const begin = `__COLLIDE_ENV_BEGIN_${nonce}__`;
  const end = `__COLLIDE_ENV_END_${nonce}__`;
  const script = `printf '%s' '${begin}'; /usr/bin/env -0; printf '%s' '${end}'`;
  const started = performance.now();
  const result = spawnSync(shell, [...flags, script], {
    cwd: process.env.HOME, env: process.env, stdio: ["ignore", "pipe", "pipe"], timeout: timeoutMs, maxBuffer: 1 << 26,
  });
  const ms = performance.now() - started;
  const stdout = result.stdout ?? Buffer.alloc(0);
  const stderr = (result.stderr ?? Buffer.alloc(0)).toString("utf8");
  const b = stdout.indexOf(begin);
  const e = b < 0 ? -1 : stdout.indexOf(end, b + begin.length);
  let env = null;
  if (b >= 0 && e >= 0) {
    env = {};
    for (const entry of stdout.subarray(b + begin.length, e).toString("utf8").split("\0")) {
      const eq = entry.indexOf("=");
      if (eq > 0) env[entry.slice(0, eq)] = entry.slice(eq + 1);
    }
  }
  return {
    ms, status: result.status, signal: result.signal, timedOut: result.error?.code === "ETIMEDOUT", env,
    pollution: {
      stdoutBefore: b < 0 ? stdout.length : b,
      stdoutAfter: e < 0 ? 0 : stdout.length - e - end.length,
      stderrBytes: Buffer.byteLength(stderr),
      stderrFirstLine: stderr.split("\n").find(Boolean)?.slice(0, 160) ?? "",
    },
  };
}

function which(tool, path) {
  for (const dir of (path ?? "").split(":").filter(Boolean)) {
    const candidate = join(dir, tool);
    try {
      if (statSync(candidate).isFile()) {
        accessSync(candidate, constants.X_OK);
        return candidate;
      }
    } catch {
      // not here; keep looking
    }
  }
  return null;
}

/** Finds each tool on PATH and runs `tool --version` with exactly that env, as Collide would spawn it. */
function resolveTools(env) {
  const found = {};
  for (const tool of TOOLS) {
    const path = which(tool, env.PATH);
    let version = null;
    if (path) {
      const r = spawnSync(path, ["--version"], { env, encoding: "utf8", timeout: 15000 });
      version = r.status === 0 ? r.stdout.split("\n")[0].trim() : `exit ${r.status}: ${(r.stderr ?? "").split("\n")[0]}`;
    }
    found[tool] = { path, version };
  }
  return found;
}

function median(values) {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.floor(sorted.length / 2)];
}

const options = parseArgs(process.argv.slice(2));
const runs = Number(options.runs ?? 5);
const timeoutMs = Number(options["timeout-ms"] ?? 10000);
const { shell, source } = loginShell();
const report = {
  launch: {
    ppid: process.ppid,
    names: Object.keys(process.env).sort(),
    PATH: process.env.PATH,
    shell, shellSource: source,
    tools: resolveTools(process.env),
  },
  modes: [],
};

for (const mode of MODES) {
  const attempts = Array.from({ length: runs }, () => captureOnce(shell, mode.flags, timeoutMs));
  const last = attempts[attempts.length - 1];
  report.modes.push({
    name: mode.name,
    runs,
    medianMs: Number(median(attempts.map((a) => a.ms)).toFixed(1)),
    minMs: Number(Math.min(...attempts.map((a) => a.ms)).toFixed(1)),
    maxMs: Number(Math.max(...attempts.map((a) => a.ms)).toFixed(1)),
    parsed: attempts.filter((a) => a.env !== null).length,
    timedOut: attempts.filter((a) => a.timedOut).length,
    exitCodes: [...new Set(attempts.map((a) => a.status))],
    pollution: last.pollution,
    variableCount: last.env ? Object.keys(last.env).length : 0,
    // Only set by the synthetic rc files in scripts/06: proves NUL delimiting keeps a value with a newline whole.
    multilineIntact: last.env && "S4_MULTILINE" in last.env ? last.env.S4_MULTILINE === "line one\nline two" : undefined,
    PATH: last.env?.PATH ?? null,
    tools: last.env ? resolveTools(last.env) : null,
  });
}

if (options.json) {
  process.stdout.write(`${JSON.stringify(report)}\n`);
} else {
  const toolLine = (tools) => TOOLS.map((t) => `${t}=${tools[t].path ? "yes" : "NO"}`).join(" ");
  console.log(`launch env: ${report.launch.names.length} variables (${report.launch.names.join(" ")})`);
  console.log(`launch PATH: ${report.launch.PATH}`);
  console.log(`login shell: ${shell} (from ${source})`);
  console.log(`tools on the launch PATH: ${toolLine(report.launch.tools)}`);
  for (const mode of report.modes) {
    console.log("");
    console.log(`${mode.name}: median ${mode.medianMs} ms (min ${mode.minMs}, max ${mode.maxMs}) over ${mode.runs} runs; ` +
      `parsed ${mode.parsed}/${mode.runs}; timed out ${mode.timedOut}; exit ${mode.exitCodes.join(",")}`);
    console.log(`  pollution: ${mode.pollution.stdoutBefore} B stdout before the begin marker, ` +
      `${mode.pollution.stdoutAfter} B after the end marker, ${mode.pollution.stderrBytes} B stderr` +
      (mode.pollution.stderrFirstLine ? ` (first line: ${JSON.stringify(mode.pollution.stderrFirstLine)})` : ""));
    console.log(`  captured: ${mode.variableCount} variables; PATH: ${mode.PATH}`);
    if (mode.tools) {
      for (const tool of TOOLS) {
        const t = mode.tools[tool];
        console.log(`    ${tool.padEnd(7)} ${t.path ? `${t.path}  (${t.version})` : "not found"}`);
      }
    }
  }
}
