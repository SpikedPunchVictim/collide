#!/usr/bin/env node
// Q3 extra: asks a live session for its context breakdown with a get_context_usage control request
// (a subtype found in the 2.1.323 binary's strings). Sends one prompt, waits for the result, sends the request,
// prints the control_response. Usage: node context-usage.mjs <cwd> -- <claude args>
import { spawn } from 'node:child_process';
import readline from 'node:readline';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
const here = dirname(fileURLToPath(import.meta.url));
const a = process.argv.slice(2); const sep = a.indexOf('--');
const cwd = resolve(a[0]); const args = a.slice(sep + 1);
const child = spawn(resolve(here, 'claude-clean.sh'), args, { cwd, stdio: ['pipe', 'pipe', 'pipe'] });
const send = (o) => child.stdin.write(JSON.stringify(o) + '\n');
send({ type: 'user', message: { role: 'user', content: [{ type: 'text', text: 'Reply with exactly: PONG' }] }, parent_tool_use_id: null, session_id: '' });
let asked = false;
readline.createInterface({ input: child.stdout }).on('line', (l) => {
  let m; try { m = JSON.parse(l); } catch { return; }
  if (m.type === 'result' && !asked) { asked = true; send({ type: 'control_request', request_id: 'ctx-1', request: { subtype: 'get_context_usage' } }); }
  if (m.type === 'control_response') { console.log(JSON.stringify(m)); child.stdin.end(); }
});
readline.createInterface({ input: child.stderr }).on('line', (l) => console.error('stderr: ' + l));
setTimeout(() => { console.error('timeout'); child.kill('SIGKILL'); }, 90000);
child.on('exit', () => process.exit(0));
