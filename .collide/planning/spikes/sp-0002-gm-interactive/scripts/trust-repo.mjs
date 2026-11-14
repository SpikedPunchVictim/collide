// One-off: accept claude's workspace-trust dialog for the scratch repo (Down, Enter), then /exit.
import { attach, spawnRun, sleep, SPIKE } from '../lib/client.mjs';
import { claudeArgv, ENV, REPO, stopRun } from '../lib/runs.mjs';
import { strip } from '../lib/client.mjs';
const dir = process.argv[2] || REPO;
const id = 'trust' + Date.now() % 100000;
await spawnRun({ id, argv: claudeArgv(), cwd: dir, mode: 'pty', env: ENV });
const a = await attach(id);
await sleep(5000);
if (/trust\s*this\s*folder/.test(strip(a.text()))) {
  console.log('trust dialog shown for', dir);
  a.input('\x1b[B'); await sleep(300); a.input('\r'); await sleep(4000);
} else console.log('no trust dialog for', dir);
console.log(strip(a.text()).split('\n').filter((l) => l.trim()).slice(-12).join('\n'));
a.input('/exit\r'); await sleep(1500);
await stopRun(id);
a.close();
