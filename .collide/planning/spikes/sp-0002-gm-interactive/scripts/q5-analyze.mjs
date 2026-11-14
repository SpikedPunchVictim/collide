// Q5: compare OTel (logs + metrics) with the session transcript: tokens, cost, per-tool timings, delivery delay.
// Usage: node q5-analyze.mjs [otel.jsonl] [truth.json]
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const otelFile = process.argv[2] || 'out/q5/otel.jsonl';
const truthFile = process.argv[3] || 'out/q5/truth.json';
const recs = fs.readFileSync(otelFile, 'utf8').split('\n').filter(Boolean).map((x) => JSON.parse(x));
const val = (v) => v.stringValue ?? (v.intValue !== undefined ? Number(v.intValue) : undefined) ?? v.doubleValue ?? v.boolValue;
const attrs = (a) => Object.fromEntries((a || []).map((x) => [x.key, val(x.value)]));

const logs = [];
const mets = [];
for (const r of recs) {
  for (const rl of r.body.resourceLogs || []) for (const sl of rl.scopeLogs || []) for (const lr of sl.logRecords || []) {
    const a = attrs(lr.attributes);
    logs.push({ recv: r.recv_ms, t: Number(lr.timeUnixNano) / 1e6, name: a['event.name'], a, res: attrs(rl.resource.attributes) });
  }
  for (const rm of r.body.resourceMetrics || []) for (const sm of rm.scopeMetrics || []) for (const m of sm.metrics || []) {
    for (const p of (m.sum || m.gauge || {}).dataPoints || []) mets.push({ recv: r.recv_ms, name: m.name, temporality: m.sum?.aggregationTemporality, t: Number(p.timeUnixNano) / 1e6, v: p.asDouble ?? Number(p.asInt), a: attrs(p.attributes) });
  }
}
const sessions = [...new Set(logs.map((l) => l.a['session.id']).filter(Boolean))];
console.log('sessions seen by receiver:', sessions.join(', '));
const sid = process.argv[4] || sessions[0];

// ---- transcript
const root = path.join(os.homedir(), '.claude/projects');
let tfile = null;
for (const d of fs.readdirSync(root)) { const f = path.join(root, d, sid + '.jsonl'); if (fs.existsSync(f)) tfile = f; }
const tr = fs.readFileSync(tfile, 'utf8').split('\n').filter(Boolean).map((x) => JSON.parse(x));
const byMsg = new Map();
for (const r of tr) if (r.type === 'assistant' && r.message?.id) byMsg.set(r.message.id, r.message.usage);
const sumT = { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 };
for (const u of byMsg.values()) { sumT.input += u.input_tokens; sumT.output += u.output_tokens; sumT.cacheRead += u.cache_read_input_tokens; sumT.cacheWrite += u.cache_creation_input_tokens; }
const assistantRecords = tr.filter((r) => r.type === 'assistant').length;
const cs = tr.filter((r) => r.type === 'cost-state').pop();

// ---- OTel tokens
const apis = logs.filter((l) => l.name === 'api_request' && l.a['session.id'] === sid);
const sumO = (rows) => rows.reduce((s, l) => ({ input: s.input + l.a.input_tokens, output: s.output + l.a.output_tokens, cacheRead: s.cacheRead + l.a.cache_read_tokens, cacheWrite: s.cacheWrite + l.a.cache_creation_tokens, cost: s.cost + l.a.cost_usd }), { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, cost: 0 });
const bySource = {};
for (const l of apis) (bySource[l.a.query_source] ||= []).push(l);
console.log('\n== tokens');
console.log('transcript, assistant messages deduped by message.id (%d records, %d unique ids), main thread only:', assistantRecords, byMsg.size, sumT);
for (const [k, v] of Object.entries(bySource)) console.log(`OTel api_request events, query_source=${k} (${v.length}):`, sumO(v));
console.log('OTel api_request events, all sources:', sumO(apis));
if (cs) console.log('transcript cost-state (all sources, written once at exit):', JSON.stringify(cs.modelUsage), 'totalCostUSD', cs.totalCostUSD, 'totalToolDuration', cs.totalToolDuration);
const lastMetric = {};
for (const m of mets.filter((m) => m.name === 'claude_code.token.usage' && m.a['session.id'] === sid)) lastMetric[m.a.type + '/' + m.a.model] = m;
console.log('OTel metric claude_code.token.usage, last data point per type:', Object.fromEntries(Object.entries(lastMetric).map(([k, m]) => [k, m.v])), 'temporality', Object.values(lastMetric)[0]?.temporality);
const dups = {};
for (const m of mets.filter((m) => m.name === 'claude_code.token.usage')) dups[m.a.type] = (dups[m.a.type] || 0) + m.v;
console.log('OTel metric token.usage summed over all exports (if delta):', dups);
const costMet = mets.filter((m) => m.name === 'claude_code.cost.usage');
console.log('OTel metric cost.usage: points', costMet.length, 'sum', costMet.reduce((s, m) => s + m.v, 0).toFixed(6), 'last', costMet.at(-1)?.v);

// ---- tool timings
console.log('\n== per-tool timings');
const toolUseTs = new Map();
const toolResTs = new Map();
for (const r of tr) {
  if (r.type === 'assistant') for (const c of r.message.content || []) if (c.type === 'tool_use') toolUseTs.set(c.id, { ts: Date.parse(r.timestamp), name: c.name });
  if (r.type === 'user' && Array.isArray(r.message?.content)) for (const c of r.message.content) if (c.type === 'tool_result') toolResTs.set(c.tool_use_id, Date.parse(r.timestamp));
}
const truth = fs.existsSync(truthFile) ? JSON.parse(fs.readFileSync(truthFile, 'utf8')) : null;
for (const l of logs.filter((l) => l.name === 'tool_result' && l.a['session.id'] === sid)) {
  const u = toolUseTs.get(l.a.tool_use_id);
  const resTs = toolResTs.get(l.a.tool_use_id);
  console.log(`${l.a.tool_name}: OTel duration_ms=${l.a.duration_ms}; transcript tool_use->tool_result timestamps = ${resTs - u.ts} ms; decision=${l.a.decision_type}/${l.a.decision_source}${l.a.tool_name === 'Bash' && truth ? `; command's own clock: ${truth.bash_ms.toFixed(1)} ms` : ''}`);
}
const decisions = logs.filter((l) => l.name === 'tool_decision');
console.log('tool_decision events:', decisions.map((d) => JSON.stringify({ tool: d.a.tool_name, decision: d.a.decision, source: d.a.source ?? d.a.decision_source, t: d.t - (decisions[0]?.t || 0) })).join(' '));

// ---- delivery
console.log('\n== delivery delay (receiver clock minus event timestamp, same host)');
const delay = (rows) => { const d = rows.map((r) => r.recv - r.t).sort((a, b) => a - b); return d.length ? `n=${d.length} min ${d[0].toFixed(0)} p50 ${d[Math.floor(d.length / 2)].toFixed(0)} max ${d.at(-1).toFixed(0)} ms` : 'none'; };
console.log('logs:', delay(logs));
console.log('metrics:', delay(mets));
for (const n of ['api_request', 'tool_result', 'user_prompt']) console.log(`  ${n}:`, delay(logs.filter((l) => l.name === n)));

// ---- leakage
console.log('\n== what is in the stream');
const raw = fs.readFileSync(otelFile, 'utf8');
for (const needle of ['17 times 3', 'date +%s%N', 'first line only', 'README.md', 'sleep 2']) console.log(`  contains "${needle}": ${raw.includes(needle)}`);
console.log('  user_prompt attrs:', JSON.stringify(Object.keys(logs.find((l) => l.name === 'user_prompt').a)), 'prompt=', JSON.stringify(logs.find((l) => l.name === 'user_prompt').a.prompt));
console.log('  event names:', [...new Set(logs.map((l) => l.name))].join(', '));
console.log('  metric names:', [...new Set(mets.map((m) => m.name))].join(', '));
console.log('  attributes on a metric point:', Object.keys(mets.find((m) => m.name === 'claude_code.token.usage').a).join(', '));
console.log('  resource attributes:', Object.keys(logs[0].res).join(', '));
