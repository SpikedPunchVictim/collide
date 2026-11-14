// Exploration: start interactive claude in a PTY under the daemon, print what the TUI shows.
import path from 'node:path';
import { attach, spawnRun, sleep, CLEAN, SPIKE } from '../lib/client.mjs';
import { strip } from '../lib/client.mjs';
const id = process.argv[2] || 'probe1';
await spawnRun({ id, argv: [CLEAN, '--model', 'haiku', '--allowedTools', 'Bash', 'Read'], cwd: path.join(SPIKE, 'work/repo'), mode: 'pty', env: { TERM: 'xterm-256color', COLORTERM: 'truecolor' }, rows: 40, cols: 120 });
const a = await attach(id);
await sleep(7000);
console.log(strip(a.text()).split('\n').filter((l) => l.trim()).slice(-25).join('\n'));
a.close();
