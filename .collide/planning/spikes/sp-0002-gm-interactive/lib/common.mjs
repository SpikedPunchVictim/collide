import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { S2D, SPIKE, STATE, rpc, sleep } from './client.mjs';

export const say = (...a) => console.log(...a);
export const alive = (pid) => { try { process.kill(pid, 0); return true; } catch (e) { return e.code === 'EPERM'; } };
export const kill = (pid, sig = 'SIGKILL') => { try { process.kill(pid, sig); return true; } catch { return false; } };
export const sh = (cmd, args, opts = {}) => execFileSync(cmd, args, { encoding: 'utf8', ...opts });

export function startDaemon(how = 'setsid', state = STATE) {
  fs.mkdirSync(path.join(state, 'runs'), { recursive: true });
  const t0 = performance.now();
  const out = sh(S2D, ['start', '--how', how, '--state', state], { env: { ...process.env, S2_STATE: state } });
  return { ms: performance.now() - t0, info: JSON.parse(out) };
}
export async function daemonPid(state = STATE) { try { return (await rpc({ cmd: 'ping' }, state)).pid; } catch { return null; } }
export function readJson(p) { try { return JSON.parse(fs.readFileSync(p, 'utf8')); } catch { return null; } }
export function coalitionOf(pids) {
  const out = sh(S2D, ['coalition', ...pids.map(String)]);
  return out.trim().split('\n').map((l) => JSON.parse(l));
}
export function psName(pid) { try { return sh('ps', ['-o', 'comm=', '-p', String(pid)]).trim().split('/').pop(); } catch { return null; } }
export function newState(name) {
  const s = path.join(SPIKE, 'work', name);
  fs.rmSync(s, { recursive: true, force: true });
  fs.mkdirSync(path.join(s, 'runs'), { recursive: true });
  return s;
}
export function lines(file) { try { return fs.readFileSync(file, 'utf8').split('\n').filter(Boolean); } catch { return []; } }
export function seqGaps(file) {
  const seqs = lines(file).map((l) => /seq=(\d+)/.exec(l)).filter(Boolean).map((m) => +m[1]);
  let gaps = 0;
  for (let i = 1; i < seqs.length; i++) if (seqs[i] !== seqs[i - 1] + 1) gaps++;
  return { n: seqs.length, first: seqs[0], last: seqs[seqs.length - 1], gaps };
}
