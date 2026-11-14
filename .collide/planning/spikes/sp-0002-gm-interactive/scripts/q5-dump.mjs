// Prints OTLP records from out/q5/otel.jsonl compactly (attribute values shown except identity fields).
import fs from 'node:fs';
const f = process.argv[2] || 'out/q5/otel.jsonl';
const skip = new Set(['user.id', 'user.email', 'user.account_uuid', 'user.account_id', 'organization.id', 'terminal.type']);
const val = (v) => v.stringValue ?? v.intValue ?? v.doubleValue ?? v.boolValue ?? JSON.stringify(v);
const attrs = (a) => Object.fromEntries((a || []).filter((x) => !skip.has(x.key)).map((x) => [x.key, val(x.value)]));
const l = fs.readFileSync(f, 'utf8').split('\n').filter(Boolean).map((x) => JSON.parse(x));
const want = process.argv[3];
for (const r of l) {
  const b = r.body;
  for (const rl of b.resourceLogs || []) for (const sl of rl.scopeLogs || []) for (const lr of sl.logRecords || []) {
    const a = attrs(lr.attributes);
    const name = a['event.name'] || lr.body?.stringValue;
    if (want && name !== want) continue;
    console.log(`LOG recv=${r.recv_ms} t=${Math.round(Number(lr.timeUnixNano) / 1e6)} ${JSON.stringify(a)}`);
  }
  for (const rm of b.resourceMetrics || []) for (const sm of rm.scopeMetrics || []) for (const m of sm.metrics || []) {
    if (want && m.name !== want) continue;
    const pts = (m.sum || m.gauge || {}).dataPoints || [];
    for (const p of pts) console.log(`MET recv=${r.recv_ms} ${m.name} unit=${m.unit} agg=${m.sum ? 'sum/' + m.sum.aggregationTemporality : 'gauge'} start=${Math.round(Number(p.startTimeUnixNano) / 1e6)} t=${Math.round(Number(p.timeUnixNano) / 1e6)} value=${p.asDouble ?? p.asInt} ${JSON.stringify(attrs(p.attributes))}`);
  }
}
