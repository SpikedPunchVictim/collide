// Q4, Terminal half. AppleEvents to Terminal time out in this environment (-1712), so the window is opened with
// `open -a Terminal <script>.command` (LaunchServices), and "closing the window" is emulated by SIGHUP to the shell
// that Terminal started and its tty session (what a window close delivers). One window starts two daemons:
// A with a plain setsid spawn, B with the one-shot launchd job. Each gets a pipe-mode and a PTY-mode `sleep` child.
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { rpc, spawnRun, listRuns, sleep, SPIKE, S2D } from '../lib/client.mjs';
import { alive, kill, say, coalitionOf } from '../lib/common.mjs';

const W = path.join(SPIKE, 'work/q4');
fs.mkdirSync(W, { recursive: true });
const SA = path.join(SPIKE, 'work/state-q4a'), SB = path.join(SPIKE, 'work/state-q4b');
for (const s of [SA, SB]) { fs.rmSync(s, { recursive: true, force: true }); fs.mkdirSync(path.join(s, 'runs'), { recursive: true }); }
for (const f of ['shell.pid', 'A.out', 'B.out']) fs.rmSync(path.join(W, f), { force: true });
const cmd = path.join(W, 'term.command');
fs.writeFileSync(cmd, `#!/bin/sh\necho $$ > ${W}/shell.pid\n${S2D} start --how setsid --state ${SA} > ${W}/A.out 2>&1\n${S2D} start --how launchd --state ${SB} > ${W}/B.out 2>&1\nsleep 600\n`, { mode: 0o755 });
execFileSync('open', ['-a', 'Terminal', cmd]);
for (let i = 0; i < 80 && !fs.existsSync(path.join(W, 'B.out')); i++) await sleep(250);
await sleep(1500);
const shell = +fs.readFileSync(path.join(W, 'shell.pid'), 'utf8').trim();
const termApp = +execFileSync('pgrep', ['-x', 'Terminal'], { encoding: 'utf8' }).trim().split('\n')[0];
say(`A.out: ${fs.readFileSync(path.join(W, 'A.out'), 'utf8').trim().slice(0, 120)}...`);
say(`B.out: ${fs.readFileSync(path.join(W, 'B.out'), 'utf8').trim().replace(/"state":"[^"]*"/, '"state":"..."').slice(0, 300)}`);
const pa = (await rpc({ cmd: 'ping' }, SA)).pid, pb = (await rpc({ cmd: 'ping' }, SB)).pid;
const runs = {};
for (const [tag, st] of [['A', SA], ['B', SB]]) for (const [id, mode] of [['c', 'pipe'], ['p', 'pty']]) runs[tag + id] = (await spawnRun({ id, argv: ['/bin/sleep', '3000'], cwd: '/tmp', mode, env: {} }, st)).run;
const sleeper = +execFileSync('pgrep', ['-P', String(shell)], { encoding: 'utf8' }).trim().split('\n')[0];
const rows = [['Terminal.app (not ours)', termApp], ['our shell in Terminal', shell], ['daemon A (setsid)', pa], ['shim A pipe', runs.Ac.shim_pid], ['child A pipe', runs.Ac.child_pid], ['shim A pty', runs.Ap.shim_pid], ['child A pty', runs.Ap.child_pid], ['daemon B (launchd)', pb], ['shim B pipe', runs.Bc.shim_pid], ['child B pipe', runs.Bc.child_pid], ['shim B pty', runs.Bp.shim_pid], ['child B pty', runs.Bp.child_pid]];
const co = coalitionOf(rows.map((r) => r[1]));
const ps = (pid) => { try { return execFileSync('ps', ['-o', 'ppid=,pgid=,tty=', '-p', String(pid)], { encoding: 'utf8' }).trim().replace(/\s+/g, ' '); } catch { return 'gone'; } };
say('name'.padEnd(26) + 'pid'.padEnd(8) + 'resource-coalition'.padEnd(20) + 'jetsam'.padEnd(10) + 'ppid pgid tty');
for (const [i, [n, p]] of rows.entries()) say(n.padEnd(26) + String(p).padEnd(8) + String(co[i].coalition).padEnd(20) + String(co[i].jetsam).padEnd(10) + ps(p));
say(`daemon A in Terminal's coalition: ${co[2].coalition === co[0].coalition}; daemon B in Terminal's coalition: ${co[7].coalition === co[0].coalition}`);

// the "close": hang up the shell's session the way closing the window does
const pids = Object.fromEntries(rows.map((r) => [r[0], r[1]]));
kill(sleeper, 'SIGHUP'); kill(shell, 'SIGHUP');
say(`SIGHUP to our shell ${shell} and its child ${sleeper} (window close emulated)`);
await sleep(5000);
say(`our shell alive: ${alive(shell)}; Terminal.app alive: ${alive(termApp)}`);
for (const [n, p] of rows.slice(2)) say(`  ${n.padEnd(22)} alive: ${alive(p)}`);
for (const [tag, st] of [['A', SA], ['B', SB]]) { try { say(`daemon ${tag} answers: ${JSON.stringify((await listRuns(st)).map((r) => r.id + '=' + r.phase))}`); } catch (e) { say(`daemon ${tag} does not answer: ${e.message}`); } }
// clean up what this script started
for (const [n, p] of rows.slice(2)) kill(p);
await sleep(500);
