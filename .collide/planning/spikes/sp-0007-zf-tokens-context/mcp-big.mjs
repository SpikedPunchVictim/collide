#!/usr/bin/env node
// Trivial stdio MCP server for spike S9 (adapted from S1's mcp-canary.mjs). One tool, `big`, that
// returns `chars` characters of numbered words ("w1 w2 ..."), to see how MAX_MCP_OUTPUT_TOKENS cuts it.
import readline from 'node:readline';

const reply = (id, result) => process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id, result }) + '\n');
const replyError = (id, message) => process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id, error: { code: -32601, message } }) + '\n');

readline.createInterface({ input: process.stdin }).on('line', (line) => {
  let req;
  try { req = JSON.parse(line); } catch { return; }
  if (req.id === undefined) return;
  if (req.method === 'initialize') {
    reply(req.id, { protocolVersion: req.params?.protocolVersion ?? '2025-06-18', capabilities: { tools: {} }, serverInfo: { name: 's9big', version: '0.0.1' } });
  } else if (req.method === 'tools/list') {
    reply(req.id, { tools: [{ name: 'big', description: 'Returns a long text of the requested length in characters.', inputSchema: { type: 'object', properties: { chars: { type: 'number' } }, required: ['chars'] } }] });
  } else if (req.method === 'tools/call') {
    const n = Number(req.params?.arguments?.chars ?? 1000);
    let s = '';
    for (let i = 1; s.length < n; i++) s += `w${i} `;
    reply(req.id, { content: [{ type: 'text', text: s.slice(0, n) }] });
  } else if (req.method === 'ping') reply(req.id, {});
  else replyError(req.id, `method not found: ${req.method}`);
});
