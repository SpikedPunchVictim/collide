#!/usr/bin/env node
// Trivial stdio MCP server for spike S1. One tool, `canary`, that returns MCP-CANARY-<name>.
// On start it appends "mcp-start name=<name> env=[...]" to $S1_MARKS (if set), listing the
// names (not values) of the CLAUDE* and ANTHROPIC_* variables it inherited.
import { appendFileSync } from 'node:fs';
import readline from 'node:readline';

const name = process.argv[2] ?? 'unnamed';
const envNames = Object.keys(process.env).filter((k) => /^(CLAUDE|ANTHROPIC_)/.test(k)).sort();
if (process.env.S1_MARKS) appendFileSync(process.env.S1_MARKS, `mcp-start name=${name} env=[${envNames.join(',')}]\n`);

const reply = (id, result) => process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id, result }) + '\n');
const replyError = (id, message) =>
  process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id, error: { code: -32601, message } }) + '\n');

readline.createInterface({ input: process.stdin }).on('line', (line) => {
  let req;
  try {
    req = JSON.parse(line);
  } catch {
    return;
  }
  if (req.id === undefined) return; // notification
  if (req.method === 'initialize') {
    reply(req.id, {
      protocolVersion: req.params?.protocolVersion ?? '2025-06-18',
      capabilities: { tools: {} },
      serverInfo: { name, version: '0.0.1' },
    });
  } else if (req.method === 'tools/list') {
    reply(req.id, {
      tools: [{ name: 'canary', description: `Returns the S1 canary word of server ${name}.`, inputSchema: { type: 'object', properties: {} } }],
    });
  } else if (req.method === 'tools/call') {
    reply(req.id, { content: [{ type: 'text', text: `MCP-CANARY-${name}` }] });
  } else if (req.method === 'ping') {
    reply(req.id, {});
  } else {
    replyError(req.id, `method not found: ${req.method}`);
  }
});
