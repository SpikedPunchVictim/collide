// Q1 (layers below the webview): keystroke-to-echo through daemon relay and directly on the shim, for `cat` (kernel echo) and claude's TUI.
import { attach, spawnRun, sleep, strip } from '../lib/client.mjs';
import { claudeArgv, ENV, REPO, stopRun } from '../lib/runs.mjs';
import { startDaemon, say } from '../lib/common.mjs';

const stats = (a) => { const s = [...a].sort((x, y) => x - y); const q = (p) => s[Math.min(s.length - 1, Math.floor(p * s.length))]; return `n=${s.length} min ${s[0].toFixed(2)} p50 ${q(0.5).toFixed(2)} p90 ${q(0.9).toFixed(2)} p95 ${q(0.95).toFixed(2)} max ${s[s.length - 1].toFixed(2)} ms`; };

async function measure(label, a, n, gap, pool) {
  const lat = [];
  let timeouts = 0;
  for (let i = 0; i < n; i++) {
    if (i % pool.length === 0 && i > 0) { a.input('\x7f'.repeat(pool.length)); await sleep(500); }
    const c = pool[i % pool.length];
    const t0 = performance.now();
    const got = new Promise((res) => { const l = (p, at) => { if (p.toString('latin1').includes(c)) { a.listeners = a.listeners.filter((x) => x !== l); res(at); } }; a.listeners.push(l); });
    a.input(c);
    const at = await Promise.race([got, sleep(2000).then(() => null)]);
    if (at === null) timeouts++; else lat.push(at - t0);
    await sleep(gap);
  }
  a.input('\x7f'.repeat(pool.length));
  say(`${label}: ${stats(lat)} timeouts ${timeouts}`);
  return lat;
}

startDaemon('setsid');
const pool = 'QZXJKWVYGFDBPNRTML';
for (const [id, argv, cwd, gap] of [['lat-cat', ['/bin/cat'], '/tmp', 60], ['lat-cl', claudeArgv(), REPO, 200]]) {
  await spawnRun({ id, argv, cwd, mode: 'pty', env: ENV, rows: 40, cols: 120 });
  await sleep(id === 'lat-cl' ? 8000 : 500);
  for (const direct of [false, true]) {
    const a = await attach(id, { direct });
    await sleep(500);
    await measure(`${id} ${direct ? 'client->shim direct' : 'client->daemon->shim'}`, a, 72, gap, pool);
    a.close();
  }
  if (id === 'lat-cl') { const a = await attach(id); await sleep(300); a.input('/exit\r'); await sleep(1500); a.close(); }
  await stopRun(id);
}
