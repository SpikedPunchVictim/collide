import path from 'node:path';
import { CLEAN, SPIKE, listRuns, rpc, spawnRun, sleep, attach, strip } from './client.mjs';
import { alive, kill } from './common.mjs';

export const REPO = path.join(SPIKE, 'work/repo');
export const claudeArgv = (extra = []) => [CLEAN, '--model', 'haiku', '--allowedTools', 'Bash', 'Read', ...extra];
export const ENV = { TERM: 'xterm-256color', COLORTERM: 'truecolor' };

export async function stopRun(id, sig = 'SIGTERM') {
  const r = (await listRuns()).find((x) => x.id === id);
  if (r && r.phase === 'running') { kill(r.child_pid, sig); for (let i = 0; i < 50 && alive(r.shim_pid); i++) await sleep(100); }
  return r;
}
