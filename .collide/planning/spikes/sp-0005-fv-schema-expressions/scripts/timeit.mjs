// usage: node scripts/timeit.mjs <label> <cmd> [args...]   runs the command (output hidden), prints wall seconds.
import { spawnSync } from 'node:child_process';
const [label, cmd, ...args] = process.argv.slice(2);
const t0 = process.hrtime.bigint();
const r = spawnSync(cmd, args, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
const s = Number(process.hrtime.bigint() - t0) / 1e9;
console.log(`${label.padEnd(58)} ${s.toFixed(2).padStart(7)} s  exit ${r.status}`);
if (r.status !== 0) console.log(r.stderr.split('\n').slice(-8).join('\n'));
