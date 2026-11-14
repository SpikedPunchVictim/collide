// Spike S4: hook cold start. Each variant is spawned the way collide-run.mjs spawns a hook and timed from spawn
// to exit. Variants run round-robin, so load from other processes on the machine spreads across all of them.
//
//   node lib/bench.mjs --plan PLAN.json --runs 20 --warmup 2 --out DIR
//
// PLAN.json: [{ "name", "args": [...], "cwd", "input": FILE|null, "env": {} }, ...]
import { readFileSync } from "node:fs";
import { cpus, loadavg } from "node:os";
import { join } from "node:path";
import { judge, runHook } from "./collide-run.mjs";

function parseArgs(argv) {
  const options = {};
  for (let i = 0; i < argv.length; i += 2) options[argv[i].replace(/^--/, "")] = argv[i + 1];
  return options;
}

function stats(samples) {
  const sorted = [...samples].sort((a, b) => a - b);
  const at = (q) => sorted[Math.min(sorted.length - 1, Math.floor(q * (sorted.length - 1) + 0.5))];
  const ms = (value) => Number(value.toFixed(1));
  return {
    n: sorted.length, medianMs: ms(at(0.5)), minMs: ms(sorted[0]), p10Ms: ms(at(0.1)), p90Ms: ms(at(0.9)),
    maxMs: ms(sorted[sorted.length - 1]), iqrMs: ms(at(0.75) - at(0.25)),
  };
}

const options = parseArgs(process.argv.slice(2));
const plan = JSON.parse(readFileSync(options.plan, "utf8"));
const runs = Number(options.runs ?? 20);
const warmup = Number(options.warmup ?? 2);
const samples = new Map(plan.map((variant) => [variant.name, []]));
const failures = new Map(plan.map((variant) => [variant.name, 0]));
const loadBefore = loadavg();

for (let round = 0; round < warmup + runs; round += 1) {
  for (const variant of plan) {
    const run = await runHook({
      args: variant.args, cwd: variant.cwd, env: variant.env ?? {},
      input: variant.input ? readFileSync(variant.input) : "", out: join(options.out, variant.name),
    });
    const ok = variant.expectResult ? judge(run).verdict === "ok" : run.code === 0;
    if (!ok) failures.set(variant.name, failures.get(variant.name) + 1);
    if (round >= warmup) samples.get(variant.name).push(run.wallMs);
  }
}

const loadAfter = loadavg();
console.log(JSON.stringify({
  node: process.version, cpus: cpus().length,
  loadavg1m: { before: Number(loadBefore[0].toFixed(2)), after: Number(loadAfter[0].toFixed(2)) },
  runs, warmup,
}));
for (const variant of plan) {
  console.log(JSON.stringify({ name: variant.name, failures: failures.get(variant.name), ...stats(samples.get(variant.name)) }));
}
