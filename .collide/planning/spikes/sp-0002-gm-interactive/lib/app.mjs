// Drive the Tauri app: write app.json, launch the .app, wait for the bench's `done` report, move its output.
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { SPIKE, STATE, sleep } from './client.mjs';

export const APP_SH = path.join(SPIKE, 'scripts/app.sh');
export const appPid = () => { try { return +execFileSync(APP_SH, ['pid'], { encoding: 'utf8' }).trim() || null; } catch { return null; } };
export function writeCfg(cfg, state = STATE) { fs.writeFileSync(path.join(state, 'app.json'), JSON.stringify(cfg)); }
export function launchApp() { execFileSync(APP_SH, ['launch']); }
export function quitApp() { try { execFileSync(APP_SH, ['quit']); } catch {} }
export async function runApp(name, cfg, { timeoutMs = 240000, keep = false } = {}) {
  const out = path.join(SPIKE, 'out/app');
  fs.rmSync(out, { recursive: true, force: true });
  writeCfg(cfg);
  launchApp();
  const t0 = Date.now();
  while (Date.now() - t0 < timeoutMs && !fs.existsSync(path.join(out, 'done.json')) && !fs.existsSync(path.join(out, 'fatal.json'))) await sleep(500);
  const done = fs.existsSync(path.join(out, 'done.json'));
  if (!keep) { quitApp(); await sleep(1500); }
  const dest = path.join(SPIKE, 'out', name);
  fs.rmSync(dest, { recursive: true, force: true });
  if (fs.existsSync(out)) fs.renameSync(out, dest);
  return { done, dir: dest };
}
export const readOut = (dir, f) => JSON.parse(fs.readFileSync(path.join(dir, f), 'utf8'));
export const fmt = (s) => `n=${s.n} min ${s.min.toFixed(1)} p50 ${s.p50.toFixed(1)} p90 ${s.p90.toFixed(1)} p95 ${s.p95.toFixed(1)} max ${s.max.toFixed(1)} mean ${s.mean.toFixed(1)} ms`;
