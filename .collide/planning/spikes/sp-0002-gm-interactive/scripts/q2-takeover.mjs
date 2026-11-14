// Q2 + Q7: headless session -> take-over with `claude --resume <id>` in a shim-held PTY, through the real Tauri app;
// quit the app, SIGKILL the daemon, restart the daemon (one-shot launchd job), relaunch the app and attach again;
// then /compact and /clear in the take-over and check which session id a later headless --resume must use.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { randomUUID } from 'node:crypto';
import { spawn, spawnSync } from 'node:child_process';
import { attach, spawnRun, listRuns, rpc, sleep, strip, SPIKE, CLEAN } from '../lib/client.mjs';
import { ENV, REPO, stopRun } from '../lib/runs.mjs';
import { alive, kill, say, startDaemon, daemonPid } from '../lib/common.mjs';
import { runApp, readOut } from '../lib/app.mjs';

const OUT = path.join(SPIKE, 'out/q2');
const W = path.join(SPIKE, 'work/q2');
fs.rmSync(OUT, { recursive: true, force: true });
fs.rmSync(W, { recursive: true, force: true });
fs.mkdirSync(OUT, { recursive: true });
fs.mkdirSync(W, { recursive: true });
const PROJ = path.join(os.homedir(), '.claude/projects', REPO.replace(/[/.]/g, '-'));
const transcripts = () => new Set(fs.readdirSync(PROJ).filter((f) => f.endsWith('.jsonl')).map((f) => f.slice(0, -6)));
const hooksLog = path.join(OUT, 'hooks.jsonl');
const settings = path.join(W, 'settings.json');
const hookCmd = (ev) => `/bin/sh -c 'cat >> ${hooksLog}; echo >> ${hooksLog}'`;
fs.writeFileSync(settings, JSON.stringify({ hooks: { SessionStart: [{ hooks: [{ type: 'command', command: hookCmd() }] }], SessionEnd: [{ hooks: [{ type: 'command', command: hookCmd() }] }] } }));
const PORT = 14320;
const rx = spawn(process.execPath, [path.join(SPIKE, 'lib/otlp-receiver.mjs'), String(PORT), path.join(OUT, 'otel.jsonl')], { stdio: 'ignore' });
await sleep(400);
const otelEnv = {
  S1_PASS: 'CLAUDE_CODE_ENABLE_TELEMETRY', CLAUDE_CODE_ENABLE_TELEMETRY: '1', OTEL_METRICS_EXPORTER: 'otlp', OTEL_LOGS_EXPORTER: 'otlp',
  OTEL_EXPORTER_OTLP_PROTOCOL: 'http/json', OTEL_EXPORTER_OTLP_ENDPOINT: `http://127.0.0.1:${PORT}`, OTEL_METRIC_EXPORT_INTERVAL: '2000', OTEL_LOGS_EXPORT_INTERVAL: '1000',
  OTEL_RESOURCE_ATTRIBUTES: 'collide.run=run-q2',
};
const hookEvents = () => (fs.existsSync(hooksLog) ? fs.readFileSync(hooksLog, 'utf8').split('\n').filter(Boolean).map((l) => JSON.parse(l)) : []);
const headless = (args, label) => {
  const r = spawnSync(CLEAN, ['-p', ...args, '--model', 'haiku', '--max-budget-usd', '0.25', '--output-format', 'json'], { cwd: REPO, env: { PATH: process.env.PATH, HOME: process.env.HOME, USER: process.env.USER }, encoding: 'utf8', timeout: 150000 });
  let j; try { j = JSON.parse(r.stdout); } catch { j = { result: r.stdout.slice(0, 200) + r.stderr.slice(0, 200) }; }
  say(`headless ${label}: session_id ${j.session_id}; result: ${JSON.stringify(j.result)}; cost ${j.total_cost_usd}`);
  return j;
};

const known = transcripts();
startDaemon('launchd');
// 1. the headless phase, under a shim in pipe mode like the adapter would (stdout to out.log), then it ends
const sid = randomUUID();
const hl = await spawnRun({ id: 'hl1', argv: [CLEAN, '-p', 'The secret word is PLUM-731. Remember it. Reply with just: noted.', '--session-id', sid, '--model', 'haiku', '--max-budget-usd', '0.25', '--output-format', 'json'], cwd: REPO, mode: 'pipe', env: { USER: process.env.USER } });
for (let i = 0; i < 120; i++) { const r = (await listRuns()).find((x) => x.id === 'hl1'); if (r.phase !== 'running') break; await sleep(500); }
const hlOut = JSON.parse(fs.readFileSync(path.join(SPIKE, 'work/state/runs/hl1/out.log'), 'utf8'));
say(`headless phase (shim pipe mode): session ${hlOut.session_id}, result ${JSON.stringify(hlOut.result)}, cost ${hlOut.total_cost_usd}; exit ${JSON.stringify((await listRuns()).find((x) => x.id === 'hl1').exit.code)}`);

// 2. take over: --resume in a PTY held by a shim
await spawnRun({ id: 'to1', argv: [CLEAN, '--resume', sid, '--model', 'haiku', '--allowedTools', 'Bash', 'Read', '--settings', settings], cwd: REPO, mode: 'pty', env: { ...ENV, ...otelEnv }, rows: 42, cols: 130 });
const pidOf = async () => (await listRuns()).find((x) => x.id === 'to1');
const r0 = await pidOf();
say(`take-over run to1: shim ${r0.shim_pid}, claude ${r0.child_pid}`);
const cfg = (bench) => ({ start: 'launchd', attach: 'to1', renderer: 'webgl', bench });
const ask = (text, name, expect) => [{ type: 'send', text }, { type: 'wait', ms: 400 }, { type: 'send', text: '\r' }, { type: 'waitfor', text: expect, timeout: 60000 }, { type: 'wait', ms: 1500 }, { type: 'snap', name }];
// app instance 1: type a prompt that needs the headless session's context
let r = await runApp('q2-app1', cfg([{ type: 'wait', ms: 3500 }, { type: 'quiet', ms: 1500, max: 20000 }, { type: 'snap', name: 'q2-1-resumed' }, ...ask('Reply with the secret word, upper case, nothing else.', 'q2-2-answer1', 'PLUM-731'), { type: 'dump', name: 'a1' }]), { timeoutMs: 150000 });
const a1 = readOut(r.dir, 'screen-a1.json').text;
say(`app instance 1: take-over answered from the headless session's context: ${a1.includes('PLUM-731')}  (claude pid ${r0.child_pid} alive: ${alive(r0.child_pid)})`);
say(`app quit with the AppKit quit event; claude alive: ${alive(r0.child_pid)}, shim alive: ${alive(r0.shim_pid)}`);

// 3. SIGKILL the daemon, restart it as a one-shot launchd job
const dp = await daemonPid();
kill(dp, 'SIGKILL');
await sleep(800);
say(`daemon ${dp} SIGKILLed; shim alive ${alive(r0.shim_pid)}, claude alive ${alive(r0.child_pid)}`);
const d2 = startDaemon('launchd');
say(`daemon restarted (launchd one-shot): pid ${d2.info.ping.pid} in ${d2.ms.toFixed(0)} ms; run to1 phase after adopt: ${(await pidOf()).phase}`);

// 4. app instance 2: attach again (ring replay), keep talking
r = await runApp('q2-app2', cfg([{ type: 'wait', ms: 2500 }, { type: 'quiet', ms: 1500, max: 20000 }, { type: 'snap', name: 'q2-3-reattached' }, { type: 'dump', name: 'replay' }, ...ask('Now reply with the secret word in lower case, then a hyphen, then the number 4242.', 'q2-4-answer2', 'plum-731-4242'), { type: 'dump', name: 'a2' }]), { timeoutMs: 150000 });
const rp = readOut(r.dir, 'screen-replay.json').text, a2 = readOut(r.dir, 'screen-a2.json').text;
say(`app instance 2 (after app quit + daemon restart): replay shows the earlier answer: ${rp.includes('PLUM-731')}; new answer received: ${a2.toLowerCase().includes('plum-731-4242')}; claude still the same pid ${alive(r0.child_pid)}`);

// 5. /compact then /clear through the PTY (node client; the app is not needed for this part)
const a = await attach('to1', { rows: 42, cols: 130 });
const idle = async (ms = 3000, max = 90000) => { const t = Date.now(); let last = a.total, at = Date.now(); while (Date.now() - t < max) { await sleep(200); if (a.total !== last) { last = a.total; at = Date.now(); } else if (Date.now() - at > ms) return; } };
const cmd = async (text) => { a.input(text); await sleep(600); a.input('\r'); await sleep(2000); await idle(4000); };
const snapshotIds = (label) => { const ev = hookEvents(); const n = transcripts(); say(`${label}: hook events so far: ${ev.map((e) => `${e.hook_event_name}:${e.source || e.reason || ''}:${e.session_id.slice(0, 8)}`).join(' ')}; new transcripts: ${[...n].filter((x) => !known.has(x)).map((x) => x.slice(0, 8)).join(' ')}`); };
snapshotIds('after resume');
await cmd('/compact');
snapshotIds('after /compact');
const idAfterCompact = hookEvents().filter((e) => e.hook_event_name === 'SessionStart').at(-1)?.session_id;
await cmd('Reply with the secret word, upper case, nothing else.');
say(`after /compact the take-over still knows the word: ${/PLUM-731/.test(strip(a.text().slice(-6000)).replace(/Reply with the secret word, upper case, nothing else\./g, ''))}`);
await cmd('/clear');
snapshotIds('after /clear');
const idAfterClear = hookEvents().filter((e) => e.hook_event_name === 'SessionStart').at(-1)?.session_id;
a.input('/exit\r');
await sleep(4000);
await stopRun('to1');
a.close();
await sleep(2500);
rx.kill();

const all = hookEvents();
fs.writeFileSync(path.join(OUT, 'hook-events.json'), JSON.stringify(all.map(({ session_id, hook_event_name, source, reason, transcript_path }) => ({ session_id, hook_event_name, source, reason, transcript_path })), null, 1));
say('SessionStart/SessionEnd hook payloads: ' + all.map((e) => `${e.hook_event_name}(${e.source || e.reason}) ${e.session_id}`).join('\n   '));
// OTel session ids seen
const ot = fs.readFileSync(path.join(OUT, 'otel.jsonl'), 'utf8');
const sids = [...new Set([...ot.matchAll(/"session\.id","value":\{"stringValue":"([0-9a-f-]{36})"/g)].map((m) => m[1]))];
say(`OTel session.id values seen by the receiver during the take-over: ${sids.join(' ')}`);
const newT = [...transcripts()].filter((x) => !known.has(x));
say(`transcripts in the project dir created since start: ${newT.join(' ')}`);
say(`headless session ${sid}; after /compact: ${idAfterCompact}; after /clear: ${idAfterClear}`);

// 6. a later headless --resume: which id keeps the context?
const ids = [...new Set([sid, idAfterCompact, idAfterClear].filter(Boolean))];
for (const id of ids) headless(['--resume', id, 'What is the secret word? Reply with the word, or the single word unknown if you were never told one.'], `--resume ${id === sid ? 'original ' : id === idAfterClear ? 'post-clear ' : 'post-compact '}${id}`);
