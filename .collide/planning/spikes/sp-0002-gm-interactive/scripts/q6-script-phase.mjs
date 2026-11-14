// Q6: an ordinary script (not the adapter) starts `claude -p` and passes the receiver's settings in the child's environment.
// Does the receiver see that session, tagged with the script's collide.run? And does a short -p process flush before it exits?
import fs from 'node:fs';
import path from 'node:path';
import { spawn, spawnSync } from 'node:child_process';
import { sleep, SPIKE, CLEAN } from '../lib/client.mjs';
import { REPO } from '../lib/runs.mjs';

const OUT = path.join(SPIKE, 'out/q6');
fs.rmSync(OUT, { recursive: true, force: true });
fs.mkdirSync(OUT, { recursive: true });
const PORT = 14319;
const rx = spawn(process.execPath, [path.join(SPIKE, 'lib/otlp-receiver.mjs'), String(PORT), path.join(OUT, 'otel.jsonl')], { stdio: 'ignore' });
await sleep(500);
const base = {
  PATH: process.env.PATH, HOME: process.env.HOME, USER: process.env.USER, S1_PASS: 'CLAUDE_CODE_ENABLE_TELEMETRY',
  CLAUDE_CODE_ENABLE_TELEMETRY: '1', OTEL_METRICS_EXPORTER: 'otlp', OTEL_LOGS_EXPORTER: 'otlp',
  OTEL_EXPORTER_OTLP_PROTOCOL: 'http/json', OTEL_EXPORTER_OTLP_ENDPOINT: `http://127.0.0.1:${PORT}`,
};
const cases = [
  ['defaults', { ...base, OTEL_RESOURCE_ATTRIBUTES: 'collide.run=run-script-A,collide.span=span-A' }],
  ['interval-1s', { ...base, OTEL_METRIC_EXPORT_INTERVAL: '1000', OTEL_LOGS_EXPORT_INTERVAL: '1000', OTEL_RESOURCE_ATTRIBUTES: 'collide.run=run-script-B,collide.span=span-B' }],
  ['no-env', { PATH: process.env.PATH, HOME: process.env.HOME, USER: process.env.USER }], // control: the script passes nothing
];
const summary = [];
for (const [name, env] of cases) {
  const before = fs.existsSync(path.join(OUT, 'otel.jsonl')) ? fs.readFileSync(path.join(OUT, 'otel.jsonl'), 'utf8').split('\n').filter(Boolean).length : 0;
  const t0 = Date.now();
  const r = spawnSync(CLEAN, ['-p', 'Reply with the single word ok.', '--model', 'haiku', '--max-budget-usd', '0.25', '--output-format', 'json'], { cwd: REPO, env, encoding: 'utf8', timeout: 120000 });
  const t1 = Date.now();
  const j = JSON.parse(r.stdout);
  await sleep(name === 'defaults' ? 75000 : 5000); // defaults: wait past the 60 s metric interval to see whether anything arrives late
  const lines = fs.readFileSync(path.join(OUT, 'otel.jsonl'), 'utf8').split('\n').filter(Boolean).slice(before).map((x) => JSON.parse(x));
  const rows = lines.map((l) => ({ at_ms: l.recv_ms - t0, path: l.path, sessions: [...new Set(JSON.stringify(l.body).match(/"stringValue":"[0-9a-f-]{36}"/g) || [])].length, hasSid: JSON.stringify(l.body).includes(j.session_id), run: (JSON.stringify(l.body).match(/"collide\.run","value":\{"stringValue":"([^"]+)"/) || [])[1] }));
  const bySig = { logs: rows.filter((x) => x.path === '/v1/logs'), metrics: rows.filter((x) => x.path === '/v1/metrics') };
  console.log(`case ${name}: claude exit ${r.status} after ${t1 - t0} ms; session ${j.session_id}; total_cost_usd ${j.total_cost_usd}`);
  console.log(`  receiver requests: logs ${bySig.logs.length} (first at +${bySig.logs[0]?.at_ms} ms, last at +${bySig.logs.at(-1)?.at_ms}), metrics ${bySig.metrics.length} (first at +${bySig.metrics[0]?.at_ms}, last at +${bySig.metrics.at(-1)?.at_ms}); process exited at +${t1 - t0}`);
  console.log(`  requests carrying this session id: ${rows.filter((x) => x.hasSid).length}; collide.run values: ${[...new Set(rows.map((x) => x.run))].join(',') || 'none'}`);
  let events = 0, tokens = { input: 0, output: 0, cacheRead: 0, cacheCreation: 0 };
  for (const l of lines) {
    for (const rl of l.body.resourceLogs || []) for (const sl of rl.scopeLogs || []) for (const lr of sl.logRecords || []) {
      const a = Object.fromEntries(lr.attributes.map((x) => [x.key, x.value.stringValue ?? x.value.intValue ?? x.value.doubleValue]));
      if (a['event.name'] === 'api_request') events++;
    }
    for (const rm of l.body.resourceMetrics || []) for (const sm of rm.scopeMetrics || []) for (const m of sm.metrics || []) if (m.name === 'claude_code.token.usage') for (const p of m.sum.dataPoints) { const t = p.attributes.find((x) => x.key === 'type').value.stringValue; tokens[t] += Number(p.asInt ?? p.asDouble); }
  }
  console.log(`  api_request events ${events}; token.usage metric totals ${JSON.stringify(tokens)}; claude's json usage ${JSON.stringify({ input: j.usage?.input_tokens, output: j.usage?.output_tokens, cacheRead: j.usage?.cache_read_input_tokens, cacheCreation: j.usage?.cache_creation_input_tokens })}`);
}
rx.kill();
