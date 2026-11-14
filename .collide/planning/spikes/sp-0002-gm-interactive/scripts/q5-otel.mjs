// Q5: an interactive take-over-style session (PTY under the shim) with Claude Code's OpenTelemetry on, to a local receiver.
// Drives three prompts (a timed Bash tool call, a Read, a no-tool answer), exits, and leaves out/q5/otel.jsonl for q5-analyze.mjs.
import fs from 'node:fs';
import path from 'node:path';
import { spawn } from 'node:child_process';
import { attach, spawnRun, sleep, strip, SPIKE, CLEAN } from '../lib/client.mjs';
import { claudeArgv, ENV, REPO, stopRun } from '../lib/runs.mjs';
import { startDaemon, say } from '../lib/common.mjs';

const NAME = process.env.Q5_NAME || 'q5';
const EXTRA = (process.env.Q5_EXTRA || '').split(' ').filter(Boolean);
const OUT = path.join(SPIKE, 'out', NAME);
fs.rmSync(OUT, { recursive: true, force: true });
fs.mkdirSync(OUT, { recursive: true });
const PORT = 14318;
const rx = spawn(process.execPath, [path.join(SPIKE, 'lib/otlp-receiver.mjs'), String(PORT), path.join(OUT, 'otel.jsonl')], { stdio: ['ignore', 'inherit', 'inherit'] });
await sleep(500);
startDaemon('setsid');
const id = NAME;
const env = {
  ...ENV,
  S1_PASS: 'CLAUDE_CODE_ENABLE_TELEMETRY', // claude-clean.sh would otherwise strip the CLAUDE* switch
  CLAUDE_CODE_ENABLE_TELEMETRY: '1',
  OTEL_METRICS_EXPORTER: 'otlp',
  OTEL_LOGS_EXPORTER: 'otlp',
  OTEL_EXPORTER_OTLP_PROTOCOL: 'http/json',
  OTEL_EXPORTER_OTLP_ENDPOINT: `http://127.0.0.1:${PORT}`,
  OTEL_METRIC_EXPORT_INTERVAL: '2000',
  OTEL_LOGS_EXPORT_INTERVAL: '1000',
  OTEL_RESOURCE_ATTRIBUTES: 'collide.run=run-q5,collide.span=span-q5',
};
const t0 = path.join(REPO, 't0.txt'), t1 = path.join(REPO, 't1.txt');
for (const f of [t0, t1]) fs.rmSync(f, { force: true });
await spawnRun({ id, argv: claudeArgv(EXTRA), cwd: REPO, mode: 'pty', env, rows: 40, cols: 120 });
const a = await attach(id);
const idle = async (ms = 2500, max = 90000) => { const t = Date.now(); let last = a.total; let lastAt = Date.now(); while (Date.now() - t < max) { await sleep(200); if (a.total !== last) { last = a.total; lastAt = Date.now(); } else if (Date.now() - lastAt > ms) return; } };
await sleep(6000); await idle();
const ask = async (text, label) => { a.input(text); await sleep(500); const t = Date.now(); a.input('\r'); say(`prompt ${label} sent at ${t}`); await sleep(3000); await idle(4000); };
if (process.env.Q5_PROMPT) await ask(process.env.Q5_PROMPT, 'custom');
else {
await ask("Use the Bash tool to run exactly this command and nothing else: bash -c 'date +%s%N > t0.txt; sleep 2; date +%s%N > t1.txt'. Then reply with the single word done.", 'timed-bash');
if (!process.env.Q5_ONLY_BASH) {
await ask('Use the Read tool on README.md and reply with its first line only.', 'read');
await ask('What is 17 times 3? Reply with just the number.', 'no-tool');
}
}
a.input('/exit\r');
await sleep(4000);
const r = await stopRun(id);
a.close();
await sleep(3000);
rx.kill();
if (process.env.Q5_PROMPT) { say(`otel.jsonl lines: ${fs.readFileSync(path.join(OUT, 'otel.jsonl'), 'utf8').split('\n').filter(Boolean).length}`); process.exit(0); }
const truth = [fs.readFileSync(t0, 'utf8'), fs.readFileSync(t1, 'utf8')].map((s) => +s.trim());
say(`ground truth for the timed Bash command, measured inside the command: ${((truth[1] - truth[0]) / 1e6).toFixed(1)} ms`);
fs.writeFileSync(path.join(OUT, 'truth.json'), JSON.stringify({ bash_ms: (truth[1] - truth[0]) / 1e6, t0_ms: truth[0] / 1e6, t1_ms: truth[1] / 1e6 }));
say(`otel.jsonl lines: ${fs.readFileSync(path.join(OUT, 'otel.jsonl'), 'utf8').split('\n').filter(Boolean).length}`);
