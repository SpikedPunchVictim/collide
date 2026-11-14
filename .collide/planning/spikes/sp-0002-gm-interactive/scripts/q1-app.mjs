// Q1 (full path): xterm.js in the real Tauri .app, ipc::Channel, daemon relay, shim, PTY. cat baseline (webgl and dom) then claude's TUI.
import { spawnRun, sleep, attach, strip } from '../lib/client.mjs';
import { claudeArgv, ENV, REPO, stopRun } from '../lib/runs.mjs';
import { startDaemon, say } from '../lib/common.mjs';
import { runApp, readOut, fmt } from '../lib/app.mjs';

startDaemon('launchd');
const which = process.argv[2] || 'all';
const res = {};
if (which === 'all' || which === 'cat') {
  for (const renderer of ['webgl', 'dom']) {
    const id = 'cat-' + renderer;
    await spawnRun({ id, argv: ['/bin/cat'], cwd: '/tmp', mode: 'pty', env: ENV });
    const r = await runApp('q1-' + id, { attach: id, renderer, bench: [{ type: 'wait', ms: 1500 }, { type: 'latency', n: 200, label: 'cat', gap: 60, pool: 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789' }, { type: 'snap', name: 'cat-' + renderer }, { type: 'dump', name: 'cat' }] });
    const l = readOut(r.dir, 'latency-cat.json');
    say(`cat via app (${renderer}): timeouts ${l.timeouts}\n  keystroke -> echo chunk arrives in webview: ${fmt(l.echo)}\n  -> xterm parsed:                            ${fmt(l.parsed)}\n  -> next animation frame:                    ${fmt(l.frame)}`);
    await stopRun(id);
  }
}
if (which === 'all' || which === 'claude') {
  for (const renderer of ['webgl', 'dom']) {
    const id = 'cl-' + renderer;
    await spawnRun({ id, argv: claudeArgv(), cwd: REPO, mode: 'pty', env: ENV });
    const bench = [
      { type: 'wait', ms: 2000 }, { type: 'quiet', ms: 1500, max: 20000 }, { type: 'snap', name: 'claude-welcome-' + renderer }, { type: 'dump', name: 'claude-welcome' },
      { type: 'latency', n: 100, label: 'claude', gap: 200 },
    ];
    if (renderer === 'webgl') bench.push(
      { type: 'send', text: 'Use the Bash tool to run `ls` and `sleep 3`, then answer with the file names in a bullet list and one markdown table of 2 rows.' }, { type: 'wait', ms: 400 }, { type: 'send', text: '\r' },
      { type: 'wait', ms: 5000 }, { type: 'snap', name: 'claude-tool-' + renderer }, { type: 'quiet', ms: 3000, max: 60000 }, { type: 'snap', name: 'claude-answer-' + renderer }, { type: 'dump', name: 'claude-answer' },
      { type: 'resize', sizes: [[700, 520], [1280, 860], [900, 600]], snapPrefix: 'claude-resize-', hold: 800 }, { type: 'dump', name: 'claude-after-resize' });
    const r = await runApp('q1-' + id, { attach: id, renderer, bench }, { timeoutMs: 300000 });
    const l = readOut(r.dir, 'latency-claude.json');
    say(`claude via app (${renderer}): timeouts ${l.timeouts}\n  keystroke -> echo chunk arrives in webview: ${fmt(l.echo)}\n  -> xterm parsed:                            ${fmt(l.parsed)}\n  -> next animation frame:                    ${fmt(l.frame)}`);
    try { const rz = readOut(r.dir, 'resize.json'); for (const s of rz) say(`  resize to ${s.w}x${s.h}: ${s.before.cols}x${s.before.rows} -> ${s.after.cols}x${s.after.rows}; xterm resize after ${s.to_xterm_resize_ms} ms, first redraw output after ${s.to_first_redraw_ms} ms, quiet after ${s.to_quiet_ms.toFixed(0)} ms, ${s.chunks} chunks`); } catch {}
    const a = await attach(id); await sleep(300); a.input('\x15/exit\r'); await sleep(1500); a.close();
    await stopRun(id);
  }
}
