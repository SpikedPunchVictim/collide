// Q3: kill and restart the prototype daemon; shims hold stdin and the PTY master; a SIGKILLed shim makes its phase `lost`.
import fs from 'node:fs';
import path from 'node:path';
import { attach, listRuns, rpc, sleep, spawnRun, S2D } from '../lib/client.mjs';
import { alive, kill, lines, newState, readJson, say, seqGaps, startDaemon, daemonPid } from '../lib/common.mjs';
import { SPIKE } from '../lib/client.mjs';

const state = newState('state-q3');
process.env.S2_STATE = state;
const agent = path.join(SPIKE, 'lib/fake-agent.mjs');
const argv = [process.execPath, agent];
const created = [];
const d1 = startDaemon('setsid', state);
const dpid1 = d1.info.ping.pid;
say(`daemon 1 up pid ${dpid1} in ${d1.ms.toFixed(0)} ms`);

try {
  for (const [id, mode] of [['pipe1', 'pipe'], ['pty1', 'pty'], ['pipe2', 'pipe'], ['pty2', 'pty'], ['sleep1', 'pipe']]) {
    const r = await spawnRun({ id, argv: id === 'sleep1' ? ['/bin/sleep', '120'] : argv, cwd: state, mode, env: { TERM: 'xterm-256color' } }, state);
    created.push(r.run);
    say(`spawned ${id} (${mode}): shim ${r.run.shim_pid} child ${r.run.child_pid} in ${r.spawn_ms.toFixed(0)} ms`);
  }
  // phase 1: talk to pipe1 and pty1 through daemon 1
  const a = {};
  for (const id of ['pipe1', 'pty1']) a[id] = await attach(id, { state });
  for (let i = 0; i < 3; i++) { for (const id of ['pipe1', 'pty1']) a[id].input(`before-${i}\n`); await sleep(150); }
  await sleep(600);
  for (const id of ['pipe1', 'pty1']) a[id].close();

  // kill the daemon with SIGKILL; talk to the shims directly (the daemon is not needed to hold stdin)
  const tKill = Date.now();
  kill(dpid1, 'SIGKILL');
  await sleep(300);
  say(`daemon 1 killed (SIGKILL); alive=${alive(dpid1)}`);
  const shimsAlive = created.map((r) => `${r.id}: shim ${alive(r.shim_pid)} child ${alive(r.child_pid)}`);
  say(`while the daemon is dead: ${shimsAlive.join('; ')}`);
  const sizeBefore = fs.statSync(path.join(state, 'runs/pipe1/out.log')).size;
  const dr = {};
  for (const id of ['pipe1', 'pty1']) dr[id] = await attach(id, { state, direct: true });
  for (let i = 0; i < 3; i++) { for (const id of ['pipe1', 'pty1']) dr[id].input(`while-dead-${i}\n`); await sleep(150); }
  // pipe2/pty2 get their quit while the daemon is down: their exit is recorded by the shim alone
  const q = {};
  for (const id of ['pipe2', 'pty2']) { q[id] = await attach(id, { state, direct: true }); q[id].input('quit\n'); }
  await sleep(2500);
  say(`pipe1 out.log grew ${fs.statSync(path.join(state, 'runs/pipe1/out.log')).size - sizeBefore} bytes while the daemon was dead`);
  for (const id of ['pipe2', 'pty2']) say(`${id} exit.json while daemon dead: ${JSON.stringify(readJson(path.join(state, 'runs', id, 'exit.json')))}`);
  for (const id of ['pipe1', 'pty1']) dr[id].close();
  for (const id of ['pipe2', 'pty2']) q[id].close();

  // restart the daemon after 2 s of downtime
  await sleep(Math.max(0, 2000 - (Date.now() - tKill)));
  const d2 = startDaemon('setsid', state);
  say(`daemon 2 up pid ${d2.info.ping.pid} in ${d2.ms.toFixed(0)} ms (start to answering)`);
  const log = lines(path.join(state, 'daemon.log')).map((l) => JSON.parse(l));
  for (const e of log.filter((e) => e.ev === 'adopt' || e.ev === 'up').slice(-6)) say(`daemon.log: ${JSON.stringify(e)}`);
  say('list after restart: ' + (await listRuns(state)).map((r) => `${r.id}=${r.phase}${r.exit ? '(code ' + r.exit.code + ', cpu ' + (r.exit.utime_s + r.exit.stime_s).toFixed(3) + 's, rss ' + (r.exit.maxrss_bytes / 1e6).toFixed(1) + 'MB)' : ''}`).join(', '));

  // after restart: reattach, replay, keep talking
  const b = {};
  for (const id of ['pipe1', 'pty1']) b[id] = await attach(id, { state });
  await sleep(400);
  for (const id of ['pipe1', 'pty1']) b[id].input('after-restart-0\n');
  await sleep(400);
  for (const id of ['pipe1', 'pty1']) b[id].input('after-restart-1\n');
  await sleep(600);
  for (const id of ['pipe1', 'pty1']) {
    const t = b[id].text();
    const recv = [...t.matchAll(/recv=(\d+) text=([^\s]+)/g)].map((m) => `${m[1]}:${m[2]}`);
    say(`${id}: reattach replay+live recv sequence = ${recv.join(' ')}`);
  }
  const pty = b.pty1.text();
  const ptySeq = [...pty.matchAll(/seq=(\d+)/g)].map((m) => +m[1]);
  let g = 0; for (let i = 1; i < ptySeq.length; i++) if (ptySeq[i] !== ptySeq[i - 1] + 1) g++;
  say(`pty1 ring replay + live: seq ${ptySeq[0]}..${ptySeq[ptySeq.length - 1]} (${ptySeq.length} lines), gaps ${g}`);
  say(`pipe1 out.log: ${JSON.stringify(seqGaps(path.join(state, 'runs/pipe1/out.log')))}`);
  say(`pipe1 recv lines in out.log: ${lines(path.join(state, 'runs/pipe1/out.log')).filter((l) => l.startsWith('recv')).map((l) => l.split(' ')[1]).join(' ')}`);
  for (const id of ['pipe1', 'pty1']) b[id].close();

  // SIGKILL a shim: the daemon sees it gone with no exit.json
  for (const id of ['pipe1', 'pty1']) {
    const r = created.find((x) => x.id === id);
    const t0 = Date.now();
    kill(r.shim_pid, 'SIGKILL');
    let phase = null;
    while (Date.now() - t0 < 3000) { phase = (await listRuns(state)).find((x) => x.id === id).phase; if (phase === 'lost') break; await sleep(20); }
    const seen = Date.now() - t0;
    await sleep(600);
    say(`SIGKILL shim of ${id}: phase=${phase} ${seen} ms after the kill; exit.json exists=${fs.existsSync(path.join(state, 'runs', id, 'exit.json'))}; child alive 600 ms later=${alive(r.child_pid)}`);
  }
  {
    const r = created.find((x) => x.id === 'sleep1');
    kill(r.shim_pid, 'SIGKILL');
    await sleep(800);
    say(`SIGKILL shim of sleep1 (a child that ignores stdin EOF): child alive=${alive(r.child_pid)}; phase=${(await listRuns(state)).find((x) => x.id === 'sleep1').phase}`);
  }
  const tail = lines(path.join(state, 'runs/pipe1/out.log')).slice(-2);
  say(`pipe1 last out.log lines (stdin closed when its holder died): ${tail.join(' | ')}`);
  say('final daemon.log phase events:');
  for (const e of lines(path.join(state, 'daemon.log')).map((l) => JSON.parse(l)).filter((e) => e.ev === 'phase')) say('  ' + JSON.stringify(e));
} finally {
  const pid = await daemonPid(state); if (pid) kill(pid);
  for (const r of created) { kill(r.shim_pid); kill(r.child_pid); }
  await sleep(300);
  say('cleanup: ' + created.map((r) => `${r.id} shim ${alive(r.shim_pid)} child ${alive(r.child_pid)}`).join('; '));
}
