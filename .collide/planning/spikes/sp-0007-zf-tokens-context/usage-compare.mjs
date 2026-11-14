#!/usr/bin/env node
// Q1: compare the usage rows of a driver log (stream) with the same session's transcript, by message.id.
// Usage: node usage-compare.mjs <driver-log.jsonl> <session-id> [transcript-path]
// Reads the transcript read-only. Default path: ~/.claude/projects/<cwd with / and . as ->/<sid>.jsonl,
// found by looking for <sid>.jsonl under ~/.claude/projects/*collide-spikes-s9*.
import { readFileSync, readdirSync, existsSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';

const [logPath, sid, tp] = process.argv.slice(2);
const parse = (f) => readFileSync(f, 'utf8').split('\n').filter(Boolean).map((l) => { try { return JSON.parse(l); } catch { return null; } }).filter(Boolean);

const stream = parse(logPath).filter((r) => r.dir === '<-').map((r) => r.msg);
let tpath = tp;
if (!tpath) {
  const root = join(homedir(), '.claude', 'projects');
  for (const d of readdirSync(root)) if (d.includes('collide-spikes-s9') && existsSync(join(root, d, sid + '.jsonl'))) tpath = join(root, d, sid + '.jsonl');
}
if (!tpath) { console.log('transcript not found'); process.exit(1); }
const trans = parse(tpath);

function group(rows) {
  const m = new Map();
  for (const r of rows) {
    if (r.type !== 'assistant' || !r.message?.id) continue;
    const g = m.get(r.message.id) ?? { rows: 0, blocks: [], usages: new Set(), last: null };
    g.rows++;
    g.blocks.push((r.message.content ?? []).map((c) => c.type).join('+'));
    g.usages.add(JSON.stringify(r.message.usage));
    g.last = r.message.usage;
    m.set(r.message.id, g);
  }
  return m;
}
const gs = group(stream), gt = group(trans);
const ctx = (u) => (u.input_tokens ?? 0) + (u.cache_read_input_tokens ?? 0) + (u.cache_creation_input_tokens ?? 0);
const sum = (m, k) => [...m.values()].reduce((s, g) => s + (g.last?.[k] ?? 0), 0);
const sumRows = (m, k) => [...m.values()].reduce((s, g) => s + g.rows * (g.last?.[k] ?? 0), 0);
console.log(`stream rows (assistant)=${[...gs.values()].reduce((s, g) => s + g.rows, 0)} distinct message.id=${gs.size}`);
console.log(`transcript rows (assistant)=${[...gt.values()].reduce((s, g) => s + g.rows, 0)} distinct message.id=${gt.size}   (${tpath.replace(homedir(), '~')})`);
console.log('id\tstream_rows\tblocks\tidentical_usage_in_stream\ttrans_rows\ttrans_identical\tctx_stream\tctx_trans\tout_stream\tout_trans\tusage_equal');
for (const [id, a] of gs) {
  const b = gt.get(id);
  console.log([id.slice(-8), a.rows, a.blocks.join(','), a.usages.size === 1, b?.rows ?? '-', b ? b.usages.size === 1 : '-', ctx(a.last), b ? ctx(b.last) : '-', a.last.output_tokens, b?.last.output_tokens ?? '-', b ? JSON.stringify(a.last) === JSON.stringify(b.last) : '-'].join('\t'));
}
for (const k of ['input_tokens', 'cache_read_input_tokens', 'cache_creation_input_tokens', 'output_tokens'])
  console.log(`${k}: once per id=${sum(gs, k)}  summed over every stream row=${sumRows(gs, k)}  transcript once per id=${sum(gt, k)}  transcript every row=${sumRows(gt, k)}`);
const res = stream.filter((r) => r.type === 'result');
for (const r of res) console.log(`result.usage: input=${r.usage?.input_tokens} cache_read=${r.usage?.cache_read_input_tokens} cache_write=${r.usage?.cache_creation_input_tokens} output=${r.usage?.output_tokens}  total_cost_usd=${r.total_cost_usd}`);
