#!/usr/bin/env node
// Q1: is result.usage per turn the same as the per-id usage deduped (input/cache from the stream, output from the transcript)?
// For each driver log with transcript: split the stream at result events; per turn, sum over distinct message.id.
import { readFileSync, readdirSync, existsSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';
const here = new URL('.', import.meta.url).pathname;
const root = join(homedir(), '.claude', 'projects');
const dirs = readdirSync(root).filter((d) => d.includes('collide-spikes-s9'));
const parse = (f) => readFileSync(f, 'utf8').split('\n').filter(Boolean).map((l) => { try { return JSON.parse(l); } catch { return null; } }).filter(Boolean);
const logs = [];
const walk = (d) => { for (const e of readdirSync(d, { withFileTypes: true })) { const p = join(d, e.name); if (e.isDirectory()) walk(p); else if (e.name.endsWith('.jsonl')) logs.push(p); } };
walk(join(here, 'out'));
let turns = 0, exactAll = 0, exactNoOut = 0, outStreamSumEq = 0; const bad = [];
for (const l of logs) {
  const rows = parse(l).filter((r) => r.dir === '<-').map((r) => r.msg);
  const sid = rows.find((m) => m.session_id)?.session_id;
  let tf; for (const d of dirs) if (sid && existsSync(join(root, d, sid + '.jsonl'))) tf = join(root, d, sid + '.jsonl');
  if (!tf) continue;
  const tr = new Map(); for (const r of parse(tf)) if (r.type === 'assistant' && r.message?.id) tr.set(r.message.id, r.message.usage);
  let cur = new Map();
  for (const m of rows) {
    if (m.type === 'assistant' && m.message?.id && !m.parent_tool_use_id) cur.set(m.message.id, m.message.usage);
    if (m.type === 'result' && m.usage) {
      turns++;
      const s = { i: 0, r: 0, w: 0, o: 0, os: 0 };
      for (const [id, u] of cur) { s.i += u.input_tokens; s.r += u.cache_read_input_tokens; s.w += u.cache_creation_input_tokens; s.os += u.output_tokens; s.o += tr.get(id)?.output_tokens ?? u.output_tokens; }
      const noOut = s.i === m.usage.input_tokens && s.r === m.usage.cache_read_input_tokens && s.w === m.usage.cache_creation_input_tokens;
      if (noOut) exactNoOut++;
      if (noOut && s.o === m.usage.output_tokens) exactAll++; else bad.push(`${l.split('/out/')[1]} result.usage=${JSON.stringify([m.usage.input_tokens, m.usage.cache_read_input_tokens, m.usage.cache_creation_input_tokens, m.usage.output_tokens])} deduped=${JSON.stringify([s.i, s.r, s.w, s.o])}`);
      if (s.os === m.usage.output_tokens) outStreamSumEq++;
      cur = new Map();
    }
  }
}
console.log(`turns checked=${turns}; result.usage input/cache fields equal the per-id sums from the stream: ${exactNoOut}/${turns}; those plus output_tokens equal to the per-id sum of the transcript's output_tokens: ${exactAll}/${turns}; output equal to the stream rows' own output_tokens summed per id: ${outStreamSumEq}/${turns}`);
for (const b of bad.slice(0, 8)) console.log('  differs: ' + b);
