#!/usr/bin/env node
// Stream-json host for spike S9: plays the part Collide's Claude adapter will play.
//
// Starts `claude-clean.sh <claude args>` (or plain `claude` with --keep-env), writes user
// turns on stdin, answers can_use_tool control requests, optionally cancels mid-turn, and
// logs every line in both directions to a JSONL file:
//   {"t": ms since start, "dir": "<-" | "->" | "stderr" | "exit" | "note", "msg": <object>}
// "<-" lines are exactly what claude wrote on stdout (parsed; unparseable lines keep "raw").
//
// Usage:
//   node driver.mjs --log out/x.jsonl [--cwd dir] [--turn TEXT]... [--perm allow,deny,close]
//        [--init] [--cancel interrupt|SIGINT|SIGTERM] [--cancel-on-tool Bash]
//        [--cancel-delay-ms 3000] [--hold-after-cancel-s N] [--timeout-s 240]
//        [--keep-env] [--env K=V]...
//        -- <claude args>
import { spawn } from 'node:child_process';
import { createWriteStream, mkdirSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import readline from 'node:readline';

const here = dirname(fileURLToPath(import.meta.url));
const argv = process.argv.slice(2);
const sep = argv.indexOf('--');
if (sep < 0) fail('missing "--" before the claude arguments');
const own = argv.slice(0, sep);
const claudeArgs = argv.slice(sep + 1);

const opt = { turns: [], perm: [], env: {}, timeoutS: 240, cancelDelayMs: 3000 };
for (let i = 0; i < own.length; i++) {
  const k = own[i];
  const v = () => {
    if (i + 1 >= own.length) fail(`${k} needs a value`);
    return own[++i];
  };
  if (k === '--log') opt.log = v();
  else if (k === '--cwd') opt.cwd = v();
  else if (k === '--turn') opt.turns.push(v());
  else if (k === '--perm') opt.perm = v().split(',');
  else if (k === '--init') opt.init = true;
  else if (k === '--cancel') opt.cancel = v();
  else if (k === '--cancel-on-tool') opt.cancelOnTool = v();
  else if (k === '--cancel-delay-ms') opt.cancelDelayMs = Number(v());
  else if (k === '--hold-after-cancel-s') opt.holdAfterCancelS = Number(v());
  else if (k === '--timeout-s') opt.timeoutS = Number(v());
  else if (k === '--keep-env') opt.keepEnv = true;
  else if (k === '--env') {
    const [ek, ...rest] = v().split('=');
    opt.env[ek] = rest.join('=');
  } else fail(`unknown option ${k}`);
}
if (!opt.log) fail('--log is required');
if (opt.turns.length === 0) fail('at least one --turn is required');

mkdirSync(dirname(resolve(opt.log)), { recursive: true });
const log = createWriteStream(opt.log);
const t0 = Date.now();
const record = (dir, msg) => log.write(JSON.stringify({ t: Date.now() - t0, dir, msg }) + '\n');

const cmd = opt.keepEnv ? 'claude' : resolve(here, 'claude-clean.sh');
const child = spawn(cmd, claudeArgs, {
  cwd: opt.cwd ? resolve(opt.cwd) : process.cwd(),
  env: { ...process.env, ...opt.env },
  stdio: ['pipe', 'pipe', 'pipe'],
});
record('note', { started: cmd, args: claudeArgs, pid: child.pid, cwd: opt.cwd ?? '.' });

let stdinOpen = true;
const send = (obj) => {
  if (!stdinOpen) return record('note', { dropped_send: obj });
  record('->', obj);
  child.stdin.write(JSON.stringify(obj) + '\n');
};
const closeStdin = () => {
  if (!stdinOpen) return;
  stdinOpen = false;
  record('note', { stdin: 'closed' });
  child.stdin.end();
};
child.stdin.on('error', (e) => record('note', { stdin_error: e.code ?? String(e) }));

let nextTurn = 0;
const sendTurn = () => {
  const text = opt.turns[nextTurn++];
  send({
    type: 'user',
    message: { role: 'user', content: [{ type: 'text', text }] },
    parent_tool_use_id: null,
    session_id: '',
  });
};

if (opt.init) send({ type: 'control_request', request_id: 'host-init-1', request: { subtype: 'initialize' } });
sendTurn();

let permIndex = 0;
let cancelled = false;
const cancelNow = () => {
  if (cancelled) return;
  cancelled = true;
  if (opt.cancel === 'interrupt') {
    send({ type: 'control_request', request_id: 'host-interrupt-1', request: { subtype: 'interrupt' } });
  } else {
    record('note', { signal_sent: opt.cancel, pid: child.pid });
    child.kill(opt.cancel);
  }
};

const rl = readline.createInterface({ input: child.stdout });
rl.on('line', (line) => {
  let msg;
  try {
    msg = JSON.parse(line);
  } catch {
    return record('<-', { raw: line });
  }
  record('<-', msg);

  if (msg.type === 'control_request' && msg.request?.subtype === 'can_use_tool') {
    const decision = opt.perm[permIndex++] ?? 'deny';
    // "close": leave the request unanswered and close stdin, as a host that dies would.
    if (decision === 'close') return closeStdin();
    const response =
      decision === 'allow'
        ? { behavior: 'allow', updatedInput: msg.request.input }
        : { behavior: 'deny', message: process.env.S9_DENY_MESSAGE ?? 'S1 host denied this tool call.' };
    send({ type: 'control_response', response: { subtype: 'success', request_id: msg.request_id, response } });
  }

  if (opt.cancel && opt.cancelOnTool && msg.type === 'assistant') {
    const uses = (msg.message?.content ?? []).filter((c) => c.type === 'tool_use' && c.name === opt.cancelOnTool);
    if (uses.length && !cancelled) setTimeout(cancelNow, opt.cancelDelayMs);
  }

  if (msg.type === 'result') {
    if (nextTurn < opt.turns.length) sendTurn();
    else if (cancelled && opt.holdAfterCancelS) {
      // Keep stdin open to see whether the cancel alone ends the process.
      record('note', { holding_stdin_s: opt.holdAfterCancelS });
      setTimeout(closeStdin, opt.holdAfterCancelS * 1000);
    } else closeStdin();
  }
});

const errRl = readline.createInterface({ input: child.stderr });
errRl.on('line', (line) => record('stderr', { line }));

const timer = setTimeout(() => {
  record('note', { timeout_s: opt.timeoutS, action: 'SIGKILL own child', pid: child.pid });
  child.kill('SIGKILL');
}, opt.timeoutS * 1000);

child.on('exit', (code, signal) => {
  clearTimeout(timer);
  record('exit', { code, signal });
  log.end(() => {
    process.stdout.write(`driver: ${opt.log} exit code=${code} signal=${signal}\n`);
    process.exit(0);
  });
});

function fail(m) {
  process.stderr.write(`driver: ${m}\n`);
  process.exit(2);
}
