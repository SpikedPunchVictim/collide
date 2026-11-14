// Q4: start the daemon from the real Tauri .app and from a Terminal window, with a plain setsid spawn and with the one-shot
// launchd job; compare coalition ids (proc_pidinfo flavor 20); then Cmd-Q the app, force-quit it, and close the Terminal window.
// Usage: node q4-coalition.mjs [scenario ...]   scenarios: app-setsid-quit app-launchd-quit app-setsid-force app-launchd-force term-setsid-close term-launchd-close
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync, spawnSync } from 'node:child_process';
import { rpc, spawnRun, listRuns, sleep, SPIKE, STATE, S2D } from '../lib/client.mjs';
import { alive, kill, say, coalitionOf, psName } from '../lib/common.mjs';
import { appPid, launchApp, quitApp, writeCfg } from '../lib/app.mjs';

const W = path.join(SPIKE, 'work/q4');
fs.mkdirSync(W, { recursive: true });
const all = ['app-setsid-quit', 'app-launchd-quit', 'app-setsid-force', 'app-launchd-force', 'term-setsid-close', 'term-launchd-close'];
const todo = process.argv.slice(2).length ? process.argv.slice(2) : all;
const osa = (script) => spawnSync('osascript', ['-e', script], { encoding: 'utf8', timeout: 20000 });
const ps = (pid) => { try { return execFileSync('ps', ['-o', 'ppid=,pgid=,sess=,comm=', '-p', String(pid)], { encoding: 'utf8' }).trim().replace(/\s+/g, ' '); } catch { return 'gone'; } };

async function cleanState() {
  try { const pid = (await rpc({ cmd: 'ping' })).pid; kill(pid); } catch {}
  for (const r of fs.existsSync(path.join(STATE, 'runs')) ? fs.readdirSync(path.join(STATE, 'runs')) : []) {
    try { const i = JSON.parse(fs.readFileSync(path.join(STATE, 'runs', r, 'shim.json'), 'utf8')); kill(i.shim_pid); kill(i.child_pid); } catch {}
  }
  await sleep(500);
  fs.rmSync(STATE, { recursive: true, force: true });
  fs.mkdirSync(path.join(STATE, 'runs'), { recursive: true });
}

for (const sc of todo) {
  const [src, how, event] = sc.split('-');
  say(`\n=== scenario ${sc}`);
  await cleanState();
  let srcPid = null, srcName = null, termWid = null, termShell = null;
  const t0 = Date.now();
  if (src === 'app') {
    writeCfg({ start: how });
    launchApp();
    for (let i = 0; i < 60 && !fs.existsSync(path.join(STATE, 'app-start.json')); i++) await sleep(250);
    srcPid = appPid();
    srcName = 'S2Spike.app';
    say(`app-start.json: ${fs.readFileSync(path.join(STATE, 'app-start.json'), 'utf8').replace(/"state":"[^"]*"/, '"state":"..."')}`);
  } else {
    const shellPidFile = path.join(W, 'term-shell.pid');
    fs.rmSync(shellPidFile, { force: true });
    fs.rmSync(path.join(W, 'term.out'), { force: true });
    const cmd = `echo $$ > ${shellPidFile}; ${S2D} start --how ${how} --state ${STATE} > ${path.join(W, 'term.out')} 2>&1`;
    const r = osa(`tell application "Terminal"\nset t to do script "${cmd}"\nreturn id of (first window whose selected tab is t)\nend tell`);
    termWid = r.stdout.trim();
    say(`Terminal window id ${termWid} (osascript rc ${r.status} ${r.stderr.trim()})`);
    for (let i = 0; i < 60 && !fs.existsSync(path.join(W, 'term.out')); i++) await sleep(250);
    await sleep(1500);
    termShell = +fs.readFileSync(shellPidFile, 'utf8').trim();
    srcPid = termShell;
    srcName = 'Terminal shell';
    say(`term.out: ${fs.readFileSync(path.join(W, 'term.out'), 'utf8').trim().replace(/"state":"[^"]*"/, '"state":"..."').slice(0, 300)}`);
  }
  const dpid = (await rpc({ cmd: 'ping' })).pid;
  const ids = {};
  for (const [id, mode] of [['q4c', 'pipe'], ['q4p', 'pty']]) {
    const r = await spawnRun({ id, argv: ['/bin/sleep', '3000'], cwd: '/tmp', mode, env: {} });
    ids[id] = r.run;
  }
  const rows = [[srcName, srcPid]];
  if (src === 'term') rows.push(['Terminal.app', +execFileSync('pgrep', ['-x', 'Terminal'], { encoding: 'utf8' }).trim().split('\n')[0]]);
  rows.push(['daemon', dpid], ['shim q4c', ids.q4c.shim_pid], ['child q4c', ids.q4c.child_pid], ['shim q4p', ids.q4p.shim_pid], ['child q4p', ids.q4p.child_pid]);
  const co = coalitionOf(rows.map((r) => r[1]));
  say('name'.padEnd(16) + 'pid'.padEnd(8) + 'resource-coalition'.padEnd(20) + 'jetsam-coalition'.padEnd(18) + 'ppid pgid sess comm');
  for (const [i, [name, pid]] of rows.entries()) say(name.padEnd(16) + String(pid).padEnd(8) + String(co[i].coalition).padEnd(20) + String(co[i].jetsam).padEnd(18) + ps(pid));
  const same = co[rows.findIndex((r) => r[0] === 'daemon')].coalition === co[0].coalition;
  say(`daemon is in the ${srcName}'s coalition: ${same}`);

  // the event
  const pids = { daemon: dpid, shimC: ids.q4c.shim_pid, childC: ids.q4c.child_pid, shimP: ids.q4p.shim_pid, childP: ids.q4p.child_pid };
  if (event === 'quit') { const r = osa('tell application "S2Spike" to quit'); say(`Cmd-Q equivalent: AppleEvent quit (rc ${r.status} ${r.stderr.trim()})`); }
  else if (event === 'force') { kill(srcPid, 'SIGKILL'); say(`force-quit: SIGKILL to the app pid ${srcPid}`); }
  else if (event === 'close') {
    const r = osa(`tell application "Terminal" to close (first window whose id is ${termWid}) saving no`);
    say(`closed Terminal window ${termWid} (osascript rc ${r.status} ${r.stderr.trim()})`);
    await sleep(1500);
    say(`after close: window still exists: ${osa(`tell application "Terminal" to return exists (window id ${termWid})`).stdout.trim()}; shell ${termShell} alive ${alive(termShell)}`);
  }
  await sleep(5000);
  const after = Object.fromEntries(Object.entries(pids).map(([k, p]) => [k, alive(p)]));
  say(`${srcName} pid ${srcPid} alive after the event: ${alive(srcPid)}`);
  say(`5 s after: daemon alive ${after.daemon}, shim q4c ${after.shimC}, child q4c ${after.childC}, shim q4p ${after.shimP}, child q4p ${after.childP}`);
  try { say(`daemon still answers: ${JSON.stringify((await listRuns()).map((r) => r.id + '=' + r.phase))}`); } catch (e) { say(`daemon does not answer: ${e.message}`); }
  // clean up what this scenario started
  if (src === 'app') { quitApp(); await sleep(1000); const p = appPid(); if (p) kill(p); }
  for (const p of Object.values(pids)) kill(p);
  await sleep(500);
  if (src === 'term') { if (alive(termShell)) say(`(terminal shell ${termShell} left alive; killing it)`), kill(termShell, 'SIGHUP'); }
}
