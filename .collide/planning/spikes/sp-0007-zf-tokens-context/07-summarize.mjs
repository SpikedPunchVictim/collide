#!/usr/bin/env node
// Shape summary of one driver log for the S9 addendum (07-hook-tool-deny.sh).
// Prints: every event from the first assistant tool_use up to the next assistant message (type/subtype, keys,
// a short body), the tool_result block(s), the result event, the exit, and the final assistant text.
// Usage: node 07-summarize.mjs <log.jsonl>
import { readFileSync } from 'node:fs';

const rows = readFileSync(process.argv[2], 'utf8').split('\n').filter(Boolean).map((l) => JSON.parse(l));
const out = rows.filter((r) => r.dir === '<-').map((r) => r.msg);
const clip = (s, n = 260) => { s = typeof s === 'string' ? s : JSON.stringify(s); return s.length > n ? s.slice(0, n) + '…' : s; };
const isToolUse = (m) => m.type === 'assistant' && (m.message?.content ?? []).some((c) => c.type === 'tool_use');
const first = out.findIndex(isToolUse);
const ids = (m) => (m.message?.content ?? []).filter((c) => c.type === 'tool_use').map((c) => `${c.name}:${clip(c.input, 80)}`);

if (first < 0) {
  console.log('  (no tool_use in the stream)');
} else {
  console.log(`  tool_use: ${ids(out[first]).join(' ')}`);
  // the window ends at the next assistant message with a different message.id
  const id0 = out[first].message.id;
  let end = out.length;
  for (let i = first + 1; i < out.length; i++) if (out[i].type === 'assistant' && out[i].message.id !== id0) { end = i; break; }
  for (let i = first + 1; i < end; i++) {
    const m = out[i];
    let d = `${m.type}${m.subtype ? '/' + m.subtype : ''}`;
    d += `  keys=[${Object.keys(m).join(',')}]`;
    if (m.type === 'system') {
      const extra = ['hook_event', 'hook_name', 'hook_id', 'outcome', 'exit_code', 'level', 'content', 'status', 'tool_name', 'tool_use_id', 'reason'].filter((k) => m[k] !== undefined).map((k) => `${k}=${clip(m[k], 120)}`);
      for (const k of ['stdout', 'stderr', 'output']) if (m[k] !== undefined) extra.push(`${k}=${clip(m[k], 160)}`);
      d += `  ${extra.join(' ')}`;
    } else if (m.type === 'user') {
      for (const c of m.message?.content ?? []) {
        if (c.type === 'tool_result') d += `\n      tool_result keys=[${Object.keys(c).join(',')}] is_error=${c.is_error} content=${clip(c.content, 400)}`;
        else d += `\n      ${c.type}: ${clip(c.text ?? c, 200)}`;
      }
      if (m.tool_use_result !== undefined) d += `\n      tool_use_result=${clip(m.tool_use_result, 200)}`;
    } else if (m.type === 'control_request') {
      d += `  request=${clip(m.request, 200)}`;
    } else if (m.type === 'assistant') {
      d += `  ${clip(m.message.content, 160)}`;
    }
    console.log(`  [${i - first}] ${d}`);
  }
}
const all = out.filter((m) => m.type === 'system').map((m) => m.subtype + (m.hook_event ? ':' + m.hook_event : ''));
const counts = {}; for (const s of all) counts[s] = (counts[s] || 0) + 1;
console.log(`  all system events: ${JSON.stringify(counts)}`);
console.log(`  can_use_tool requests: ${out.filter((m) => m.type === 'control_request' && m.request?.subtype === 'can_use_tool').length}`);
for (const m of out.filter((m) => m.type === 'result')) {
  console.log(`  result: ${JSON.stringify({ subtype: m.subtype, is_error: m.is_error, stop_reason: m.stop_reason, terminal_reason: m.terminal_reason, num_turns: m.num_turns, permission_denials: m.permission_denials, errors: m.errors, cost: m.total_cost_usd })}`);
  console.log(`  result.result: ${clip(m.result, 300)}`);
}
const lastText = [...out].reverse().find((m) => m.type === 'assistant' && m.message.content.some((c) => c.type === 'text'));
console.log(`  last assistant text: ${lastText ? clip(lastText.message.content.filter((c) => c.type === 'text').map((c) => c.text).join(' '), 400) : '(none)'}`);
console.log(`  stderr: ${clip(rows.filter((r) => r.dir === 'stderr').map((r) => r.msg.line).join(' | '), 300) || '(none)'}   exit: ${JSON.stringify(rows.find((r) => r.dir === 'exit')?.msg)}`);
