// Minimal OTLP/HTTP-JSON receiver, no dependencies. Usage: node otlp-receiver.mjs <port> <out.jsonl>
// Every POST (/v1/metrics, /v1/logs, /v1/traces) is appended to <out.jsonl> as one line:
//   {recv_ms, path, bytes, encoding, content_type, body}   (body = parsed JSON, or {raw: "<n bytes>"} if not JSON)
// GET /_count returns the number of requests per path.
import http from 'node:http';
import fs from 'node:fs';
import zlib from 'node:zlib';

const port = +process.argv[2] || 14318;
const out = process.argv[3] || 'otlp.jsonl';
const counts = {};
const server = http.createServer((req, res) => {
  if (req.method === 'GET') {
    res.writeHead(200, { 'content-type': 'application/json' });
    return res.end(JSON.stringify(counts));
  }
  const parts = [];
  req.on('data', (d) => parts.push(d));
  req.on('end', () => {
    let buf = Buffer.concat(parts);
    const enc = req.headers['content-encoding'] || '';
    try { if (enc === 'gzip') buf = zlib.gunzipSync(buf); } catch {}
    let body;
    try { body = JSON.parse(buf.toString('utf8')); } catch { body = { raw: `${buf.length} bytes` }; }
    counts[req.url] = (counts[req.url] || 0) + 1;
    fs.appendFileSync(out, JSON.stringify({ recv_ms: Date.now(), path: req.url, bytes: buf.length, encoding: enc, content_type: req.headers['content-type'], body }) + '\n');
    res.writeHead(200, { 'content-type': 'application/json' });
    res.end('{}');
  });
});
server.listen(port, '127.0.0.1', () => console.log(`otlp receiver on 127.0.0.1:${port} -> ${out}`));
