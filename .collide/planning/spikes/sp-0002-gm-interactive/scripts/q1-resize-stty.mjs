// Q1 resize: a /bin/sh PTY through the app; after each window resize ask the child `stty size` (what the kernel's winsize says).
import { spawnRun } from '../lib/client.mjs';
import { ENV, stopRun } from '../lib/runs.mjs';
import { startDaemon, say } from '../lib/common.mjs';
import { runApp, readOut } from '../lib/app.mjs';
startDaemon('launchd');
await spawnRun({ id: 'sh1', argv: ['/bin/sh'], cwd: '/tmp', mode: 'pty', env: ENV });
const r = await runApp('q1-resize-stty', { attach: 'sh1', renderer: 'webgl', bench: [{ type: 'wait', ms: 1500 }, { type: 'resize', sizes: [[700, 500], [1200, 800], [900, 600], [1000, 700]], stty: true, hold: 300 }] });
for (const s of readOut(r.dir, 'resize.json')) say(`window ${s.w}x${s.h}: xterm ${s.after.cols}x${s.after.rows}; child's stty size (rows cols) = ${s.stty}; xterm resize after ${s.to_xterm_resize_ms} ms`);
await stopRun('sh1');
