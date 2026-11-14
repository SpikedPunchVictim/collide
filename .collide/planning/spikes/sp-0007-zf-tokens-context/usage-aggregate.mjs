#!/usr/bin/env node
// Q1, wider sample: every driver log under out/ against the transcript of its session (read-only; only
// sessions this spike started: the session_id on the log's own events).
// Prints: message ids, ids repeated across several stream rows, repeated rows with identical usage, and
// how the stream's usage differs from the transcript's for the same id (input/cache vs output tokens).
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
const sessions = new Set();
const streamRows = new Map(); // id -> [usage...]
for (const l of logs) for (const r of parse(l)) {
  const m = r.msg; if (r.dir !== '<-' || !m) continue;
  if (m.session_id) sessions.add(m.session_id);
  if (m.type === 'assistant' && m.message?.id && !m.parent_tool_use_id) { const a = streamRows.get(m.message.id) ?? []; a.push(m.message.usage); streamRows.set(m.message.id, a); }
}
const trans = new Map();
let transcriptsFound = 0;
for (const s of sessions) for (const d of dirs) { const f = join(root, d, s + '.jsonl'); if (existsSync(f)) { transcriptsFound++; for (const r of parse(f)) if (r.type === 'assistant' && r.message?.id) { const a = trans.get(r.message.id) ?? []; a.push(r.message.usage); trans.set(r.message.id, a); } } }
const eq = (a, b) => JSON.stringify(a) === JSON.stringify(b);
let ids = streamRows.size, rep = 0, repSame = 0, inTrans = 0, tRep = 0, tRepSame = 0, cacheEq = 0, outEq = 0, outLess = 0, outMore = 0, maxRows = 0;
const keys = ['input_tokens', 'cache_read_input_tokens', 'cache_creation_input_tokens'];
for (const [id, us] of streamRows) {
  maxRows = Math.max(maxRows, us.length);
  if (us.length > 1) { rep++; if (us.every((u) => eq(u, us[0]))) repSame++; }
  const t = trans.get(id); if (!t) continue; inTrans++;
  if (t.length > 1) { tRep++; if (t.every((u) => eq(u, t[0]))) tRepSame++; }
  const a = us[us.length - 1], b = t[t.length - 1];
  if (keys.every((k) => a[k] === b[k])) cacheEq++;
  if (a.output_tokens === b.output_tokens) outEq++; else if (a.output_tokens < b.output_tokens) outLess++; else outMore++;
}
console.log(`driver logs=${logs.length} sessions=${sessions.size} transcripts found=${transcriptsFound}`);
console.log(`stream: distinct assistant message.id=${ids}; ids on more than one row=${rep} (max rows per id ${maxRows}); of those, identical usage on every row=${repSame}`);
console.log(`transcript: ids matched=${inTrans}; ids on more than one row=${tRep}; of those, identical usage on every row=${tRepSame}`);
console.log(`stream vs transcript, same id: input+cache fields equal=${cacheEq}/${inTrans}; output_tokens equal=${outEq}, stream lower=${outLess}, stream higher=${outMore}`);
