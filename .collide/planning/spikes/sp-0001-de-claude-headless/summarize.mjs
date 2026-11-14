#!/usr/bin/env node
// Prints the event sequence of one or more logs written by driver.mjs (or raw stream-json
// files written straight from claude's stdout), one label per line, with repeats collapsed.
//
// Usage: node summarize.mjs [--quiet] <log.jsonl>...
//   --quiet  print only the per-log totals line: results, cost_usd (the session's cumulative
//            total_cost_usd), spent_usd (what this process added to it), exit
import { readFileSync } from 'node:fs';

const args = process.argv.slice(2);
const quiet = args[0] === '--quiet';
const files = quiet ? args.slice(1) : args;

for (const file of files) {
  const rows = readFileSync(file, 'utf8')
    .split('\n')
    .filter(Boolean)
    .map((l) => {
      try {
        const o = JSON.parse(l);
        return o.dir ? o : { dir: '<-', msg: o };
      } catch {
        return { dir: '<-', msg: { raw: l } };
      }
    });

  const labels = rows.map(label).filter(Boolean);
  if (!quiet) {
    console.log(`# ${file}`);
    let prev = null;
    let n = 0;
    const flush = () => prev && console.log(n > 1 ? `${prev}  x${n}` : prev);
    for (const l of labels) {
      if (l === prev) n++;
      else {
        flush();
        prev = l;
        n = 1;
      }
    }
    flush();
  }
  console.log(`= ${file}: ${totals(rows)}`);
}

function label({ dir, msg }) {
  if (dir === 'note') return `.. note ${JSON.stringify(msg).slice(0, 160)}`;
  if (dir === 'stderr') return `!! stderr ${msg.line.slice(0, 200)}`;
  if (dir === 'exit') return `== exit code=${msg.code} signal=${msg.signal}`;
  const arrow = dir === '->' ? '->' : '<-';
  if (msg.raw !== undefined) return `${arrow} (not json) ${msg.raw.slice(0, 200)}`;
  const t = msg.type;
  if (t === 'system') return `${arrow} system/${msg.subtype}`;
  if (t === 'assistant') {
    const parts = (msg.message?.content ?? []).map((c) =>
      c.type === 'tool_use' ? `tool_use:${c.name}` : c.type === 'text' ? `text:${JSON.stringify(c.text.slice(0, 60))}` : c.type,
    );
    const err = msg.error ? ` error=${msg.error}` : '';
    return `${arrow} assistant[${parts.join(',')}]${err}`;
  }
  if (t === 'user') {
    const content = msg.message?.content;
    if (typeof content === 'string') return `${arrow} user[text]`;
    const parts = (content ?? []).map((c) => (c.type === 'tool_result' ? `tool_result${c.is_error ? '(error)' : ''}` : c.type));
    return `${arrow} user[${parts.join(',')}]`;
  }
  if (t === 'result') return `${arrow} result/${msg.subtype} is_error=${msg.is_error} cost=${msg.total_cost_usd}`;
  if (t === 'control_request') return `${arrow} control_request/${msg.request?.subtype}${msg.request?.tool_name ? ':' + msg.request.tool_name : ''}`;
  if (t === 'control_response') {
    const r = msg.response?.response;
    return `${arrow} control_response/${msg.response?.subtype}${r?.behavior ? ' behavior=' + r.behavior : ''}`;
  }
  if (t === 'rate_limit_event') return `${arrow} rate_limit_event status=${msg.rate_limit_info?.status}`;
  if (t === 'stream_event') return `${arrow} stream_event/${msg.event?.type}`;
  return `${arrow} ${t}${msg.subtype ? '/' + msg.subtype : ''}`;
}

function totals(rows) {
  const results = rows.filter((r) => r.dir === '<-' && r.msg.type === 'result');
  const costs = results.map((r) => r.msg.total_cost_usd ?? 0);
  const exit = rows.find((r) => r.dir === 'exit');
  const cost = costs.length ? Math.max(...costs) : 0;
  // total_cost_usd is cumulative per session, and a --resume'd process starts from the session's
  // earlier total. What this process spent = its last total minus the total it started from.
  const first = results[0]?.msg;
  const sinceFirst = Object.values(first?.model_usage_since_last_result ?? {}).reduce((s, u) => s + (u.costUSD ?? 0), 0);
  const startedFrom = first && first.model_usage_since_last_result ? (first.total_cost_usd ?? 0) - sinceFirst : 0;
  const spent = Math.max(0, cost - startedFrom);
  return `results=${results.length} cost_usd=${cost.toFixed(4)} spent_usd=${spent.toFixed(4)} exit=${exit ? `${exit.msg.code}/${exit.msg.signal}` : 'n/a'}`;
}
