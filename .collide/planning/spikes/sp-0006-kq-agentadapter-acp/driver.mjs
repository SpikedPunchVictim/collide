// S7 driver: a minimal ACP client over stdio against `opencode acp`.
// Usage: node driver.mjs [scenario ...]   (default: all, in order)
// Scenarios: init, allow, reject, cancel-perm, cancel-run, config, apierror, exit-pending, exit, load
// Every message in both directions goes to out/live-run.jsonl with a monotonic timestamp (ms since start).
import { spawn, execSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const WORK = path.join(HERE, 'work');
const OUT = path.join(HERE, 'out');
const SCRATCH = path.join(WORK, 'scratch');
const STATE = path.join(WORK, 'state.json');
const LOG = path.join(OUT, 'live-run.jsonl');
const OPENCODE = process.env.OPENCODE_BIN || path.join(process.env.HOME, '.opencode/bin/opencode');
const TURN_TIMEOUT_MS = Number(process.env.TURN_TIMEOUT_MS || 300000);

const scenarios = process.argv.slice(2);
const ALL = ['init', 'allow', 'cancel-perm', 'cancel-run', 'reject', 'config', 'apierror', 'exit-pending', 'exit', 'load'];
const todo = scenarios.length ? scenarios : ALL;

fs.mkdirSync(OUT, { recursive: true });
fs.mkdirSync(SCRATCH, { recursive: true });
if (!scenarios.length) fs.rmSync(LOG, { force: true });
const t0 = performance.now();
const now = () => Math.round(performance.now() - t0);
function log(dir, msg) {
  fs.appendFileSync(LOG, JSON.stringify({ t: now(), dir, msg }) + '\n');
}
const state = fs.existsSync(STATE) ? JSON.parse(fs.readFileSync(STATE, 'utf8')) : {};
const saveState = () => fs.writeFileSync(STATE, JSON.stringify(state, null, 2));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// ---- JSON-RPC peer over a child's stdio ----
class Peer {
  constructor(label) {
    this.label = label;
    this.nextId = 1;
    this.pending = new Map();
    this.onNotification = () => {};
    this.onRequest = async () => ({ error: { code: -32601, message: 'not handled' } });
    this.exit = null;
    this.exitP = null;
  }
  start(extraEnv = {}) {
    const env = {
      PATH: process.env.PATH,
      HOME: path.join(WORK, 'home'),
      XDG_CONFIG_HOME: path.join(WORK, 'config'),
      XDG_DATA_HOME: path.join(WORK, 'data'),
      XDG_STATE_HOME: path.join(WORK, 'state'),
      XDG_CACHE_HOME: path.join(WORK, 'cache'),
      OPENCODE_DISABLE_CLAUDE_CODE: '1',
      OPENCODE_DISABLE_AUTOUPDATE: '1',
      ...extraEnv,
    };
    this.child = spawn(OPENCODE, ['acp', '--cwd', SCRATCH, '--print-logs', '--log-level', 'INFO'], {
      cwd: SCRATCH,
      env,
      stdio: ['pipe', 'pipe', 'pipe'],
    });
    log('event', { spawn: this.label, pid: this.child.pid, argv: ['acp', '--cwd', SCRATCH] });
    this.spawnedAt = now();
    let buf = '';
    this.child.stdout.on('data', (d) => {
      buf += d;
      let i;
      while ((i = buf.indexOf('\n')) >= 0) {
        const line = buf.slice(0, i);
        buf = buf.slice(i + 1);
        if (line.trim()) this.#line(line);
      }
    });
    let ebuf = '';
    this.child.stderr.on('data', (d) => {
      ebuf += d;
      let i;
      while ((i = ebuf.indexOf('\n')) >= 0) {
        log('stderr', ebuf.slice(0, i));
        ebuf = ebuf.slice(i + 1);
      }
    });
    this.exitP = new Promise((res) =>
      this.child.on('exit', (code, signal) => {
        this.exit = { code, signal, at: now() };
        log('event', { exit: this.label, code, signal });
        for (const p of this.pending.values()) p.reject(new Error('child exited'));
        res(this.exit);
      }),
    );
  }
  #line(line) {
    let m;
    try {
      m = JSON.parse(line);
    } catch {
      log('stdout-nonjson', line);
      return;
    }
    log('in', m);
    if (m.method && m.id !== undefined) {
      Promise.resolve(this.onRequest(m)).then((r) => {
        const out = r && r.error ? { jsonrpc: '2.0', id: m.id, error: r.error } : { jsonrpc: '2.0', id: m.id, result: r };
        log('out', out);
        this.child.stdin.write(JSON.stringify(out) + '\n');
      });
    } else if (m.method) {
      this.onNotification(m);
    } else if (m.id !== undefined) {
      const p = this.pending.get(m.id);
      if (p) {
        this.pending.delete(m.id);
        m.error ? p.reject(Object.assign(new Error(m.error.message), { rpc: m.error })) : p.resolve(m.result);
      }
    }
  }
  request(method, params, timeoutMs = 60000) {
    const id = this.nextId++;
    const msg = { jsonrpc: '2.0', id, method, params };
    log('out', msg);
    this.child.stdin.write(JSON.stringify(msg) + '\n');
    return new Promise((resolve, reject) => {
      const to = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`timeout ${method}`));
      }, timeoutMs);
      this.pending.set(id, {
        resolve: (v) => (clearTimeout(to), resolve(v)),
        reject: (e) => (clearTimeout(to), reject(e)),
      });
    });
  }
  notify(method, params) {
    const msg = { jsonrpc: '2.0', method, params };
    log('out', msg);
    this.child.stdin.write(JSON.stringify(msg) + '\n');
  }
}

// ---- client-side behaviour ----
let permissionPolicy = 'allow'; // allow | reject | hold
let onPermission = null; // hook for scenarios
const heldPermissions = [];
let updates = []; // updates of the current turn

function makePeer(label) {
  const p = new Peer(label);
  p.onNotification = (m) => {
    if (m.method === 'session/update') updates.push({ t: now(), u: m.params.update });
  };
  p.onRequest = async (m) => {
    switch (m.method) {
      case 'session/request_permission': {
        const opts = m.params.options;
        onPermission?.(m);
        if (permissionPolicy === 'hold') {
          return new Promise((res) => heldPermissions.push({ m, res }));
        }
        const want = permissionPolicy === 'allow' ? ['allow_once', 'allow_always'] : ['reject_once', 'reject_always'];
        const o = want.map((k) => opts.find((x) => x.kind === k)).find(Boolean);
        return o ? { outcome: { outcome: 'selected', optionId: o.optionId } } : { outcome: { outcome: 'cancelled' } };
      }
      case 'fs/read_text_file': {
        const f = m.params.path;
        if (!f.startsWith(SCRATCH)) return { error: { code: -32602, message: 'outside scratch' } };
        try {
          return { content: fs.readFileSync(f, 'utf8') };
        } catch (e) {
          return { error: { code: -32002, message: String(e.message) } };
        }
      }
      case 'fs/write_text_file': {
        const f = m.params.path;
        if (!f.startsWith(SCRATCH)) return { error: { code: -32602, message: 'outside scratch' } };
        fs.writeFileSync(f, m.params.content);
        return {};
      }
      default:
        return { error: { code: -32601, message: `client does not implement ${m.method}` } };
    }
  };
  return p;
}

const CLIENT_CAPS = {
  protocolVersion: 1,
  clientCapabilities: {
    fs: { readTextFile: true, writeTextFile: true },
    terminal: false,
    session: { compaction: {}, configOptions: {}, notices: {} },
  },
  clientInfo: { name: 'collide-s7-driver', title: 'Collide S7 spike', version: '0.0.0' },
};

async function turn(peer, sessionId, text, { cancelAfter } = {}) {
  updates = [];
  const started = now();
  const prom = peer.request('session/prompt', { sessionId, prompt: [{ type: 'text', text }] }, TURN_TIMEOUT_MS);
  const res = await prom;
  const ended = now();
  summarize(`turn "${text.slice(0, 40)}"`, started, ended, res);
  return { res, started, ended, updates: updates.slice() };
}
function summarize(label, a, b, res) {
  const kinds = {};
  for (const x of updates) kinds[x.u.sessionUpdate] = (kinds[x.u.sessionUpdate] || 0) + 1;
  console.log(`${label}: ${b - a} ms, result=${JSON.stringify(res)}, updates=${JSON.stringify(kinds)}`);
}
function toolSummary(ups) {
  const by = {};
  for (const { t, u } of ups) {
    if (u.sessionUpdate !== 'tool_call' && u.sessionUpdate !== 'tool_call_update') continue;
    const e = (by[u.toolCallId] ||= { id: u.toolCallId, events: [] });
    e.events.push({ t, kind: u.sessionUpdate, status: u.status, title: u.title, toolKind: u.kind, hasRawInput: u.rawInput !== undefined, hasRawOutput: u.rawOutput !== undefined });
  }
  return Object.values(by);
}
function psGrep(pattern) {
  try {
    return execSync(`ps -axo pid,ppid,etime,command | grep -F ${JSON.stringify(pattern)} | grep -v grep`, { encoding: 'utf8' }).trim();
  } catch {
    return '';
  }
}

// ---- scenarios ----
const results = {};
let peer;
let sessionId;

let peerEnv = {};
async function ensurePeer() {
  if (peer && !peer.exit) return;
  peer = makePeer('main');
  peer.start(peerEnv);
  results.initialize = await peer.request('initialize', CLIENT_CAPS, 120000);
}
async function ensureSession() {
  await ensurePeer();
  if (sessionId) return;
  const r = await peer.request('session/new', { cwd: SCRATCH, mcpServers: [] }, 120000);
  results.sessionNew = r;
  sessionId = r.sessionId;
  state.sessionId = sessionId;
  saveState();
}

async function freshSession() {
  await ensurePeer();
  const r = await peer.request('session/new', { cwd: SCRATCH, mcpServers: [] }, 120000);
  return r.sessionId;
}

const runners = {
  async init() {
    await ensurePeer();
    console.log('initialize result:', JSON.stringify(results.initialize));
    await ensureSession();
    console.log('session/new result:', JSON.stringify(results.sessionNew));
    try {
      const l = await peer.request('session/list', { cwd: SCRATCH });
      console.log('session/list:', JSON.stringify(l));
    } catch (e) {
      console.log('session/list error:', e.message, JSON.stringify(e.rpc || null));
    }
  },
  async allow() {
    await ensureSession();
    permissionPolicy = 'allow';
    const r = await turn(peer, sessionId, 'Create a file named hello.txt in the current directory containing exactly the word collide. Use your write tool. Then reply with the single word done.');
    console.log('tools:', JSON.stringify(toolSummary(r.updates), null, 1));
    const f = path.join(SCRATCH, 'hello.txt');
    console.log('hello.txt exists:', fs.existsSync(f), fs.existsSync(f) ? JSON.stringify(fs.readFileSync(f, 'utf8')) : '');
  },
  async reject() {
    await ensureSession();
    permissionPolicy = 'reject';
    const r = await turn(peer, sessionId, 'Create a file named nope.txt in the current directory containing the word no. Use your write tool. If it is refused, say refused.');
    console.log('tools:', JSON.stringify(toolSummary(r.updates), null, 1));
    console.log('nope.txt exists:', fs.existsSync(path.join(SCRATCH, 'nope.txt')));
  },
  async 'cancel-perm'() {
    await ensurePeer();
    for (let attempt = 1; attempt <= 3; attempt++) {
      const sid = await freshSession();
      permissionPolicy = 'hold';
      heldPermissions.length = 0;
      let cancelledAt = null;
      onPermission = () => {
        // cancel as soon as the request arrives, without answering it first
        setTimeout(() => {
          cancelledAt = now();
          peer.notify('session/cancel', { sessionId: sid });
          // the client MUST answer pending permission requests with the cancelled outcome
          setTimeout(() => {
            for (const h of heldPermissions.splice(0)) h.res({ outcome: { outcome: 'cancelled' } });
          }, 50);
        }, 200);
      };
      const r = await turn(peer, sid, 'Call the write tool with filePath "cancelme.txt" and content "x". Do not answer with text first, call the tool now.');
      onPermission = null;
      console.log(`attempt ${attempt}: cancel sent at ${cancelledAt}, turn ended at ${r.ended}, delta ${cancelledAt ? r.ended - cancelledAt : 'n/a (no permission request seen)'} ms; cancelme.txt exists: ${fs.existsSync(path.join(SCRATCH, 'cancelme.txt'))}`);
      console.log('tools:', JSON.stringify(toolSummary(r.updates), null, 1));
      if (cancelledAt) break;
    }
  },
  async 'cancel-run'() {
    await ensurePeer();
    for (let attempt = 1; attempt <= 3; attempt++) {
      const sid = await freshSession();
      permissionPolicy = 'allow';
      let cancelAt = null;
      const iv = setInterval(() => {
        const running = toolSummary(updates).find((e) => e.events.some((x) => x.status === 'in_progress') && !e.events.some((x) => x.status === 'completed' || x.status === 'failed'));
        if (running && !cancelAt) {
          cancelAt = now() + 3000;
          setTimeout(() => {
            log('event', { cancelSentFor: running.id, psBefore: psGrep('sleep 47') });
            peer.notify('session/cancel', { sessionId: sid });
          }, 3000);
        }
      }, 100);
      const r = await turn(peer, sid, 'Call the bash tool with command "sleep 47" and description "wait 47 seconds". Do not answer with text first, call the tool now.');
      clearInterval(iv);
      await sleep(1500);
      const left = psGrep('sleep 47');
      console.log(`attempt ${attempt}: cancel sent ~${cancelAt}, turn ended ${r.ended}, delta ${cancelAt ? r.ended - cancelAt : 'n/a (no tool seen running)'} ms`);
      console.log('sleep 47 still alive after cancel:', left || 'no');
      log('event', { psAfter: left });
      if (left) {
        for (const l of left.split('\n')) {
          const pid = Number(l.trim().split(/\s+/)[0]);
          if (pid) { try { process.kill(pid); } catch {} }
        }
      }
      console.log('tools:', JSON.stringify(toolSummary(r.updates), null, 1));
      if (cancelAt) {
        // is the same session usable after a cancel?
        const r2 = await turn(peer, sid, 'Reply with the single word ready. Do not use tools.');
        const text = r2.updates.filter((x) => x.u.sessionUpdate === 'agent_message_chunk').map((x) => x.u.content?.text).join('');
        console.log('same session after cancel answers:', JSON.stringify(text));
        break;
      }
    }
  },
  async config() {
    await ensureSession();
    const s = results.sessionNew;
    console.log('modes:', JSON.stringify(s.modes));
    console.log('configOptions:', JSON.stringify(s.configOptions));
    if (s.modes?.availableModes?.length > 1) {
      const other = s.modes.availableModes.find((m) => m.id !== s.modes.currentModeId);
      try {
        const r = await peer.request('session/set_mode', { sessionId, modeId: other.id });
        console.log('set_mode ->', other.id, JSON.stringify(r));
      } catch (e) {
        console.log('set_mode error', e.message, JSON.stringify(e.rpc));
      }
    }
    const model = (s.configOptions || []).find((o) => o.category === 'model' || o.id === 'model');
    if (model) {
      console.log('model option:', JSON.stringify(model).slice(0, 600));
    }
    // an unknown method, to see how errors look
    try {
      await peer.request('session/set_model', { sessionId, modelId: 'ollama/llama3.1:8b' });
      console.log('session/set_model accepted');
    } catch (e) {
      console.log('session/set_model:', e.message, JSON.stringify(e.rpc));
    }
  },
  async apierror() {
    // The model does not exist on the Ollama server (an API 404, nothing is pulled): how does ACP report an API failure?
    if (peer && !peer.exit) { peer.child.stdin.end(); await peer.exitP; }
    peer = null; sessionId = null;
    // a copy of the config dir whose default model is one that Ollama does not have
    const badRoot = path.join(WORK, 'config-bad');
    fs.rmSync(badRoot, { recursive: true, force: true });
    fs.cpSync(path.join(WORK, 'config'), badRoot, { recursive: true });
    const cfgFile = path.join(badRoot, 'opencode', 'opencode.json');
    const cfg = JSON.parse(fs.readFileSync(cfgFile, 'utf8'));
    cfg.model = 'ollama/nosuch:1b';
    cfg.provider.ollama.models['nosuch:1b'] = { name: 'nosuch:1b', tools: true };
    fs.writeFileSync(cfgFile, JSON.stringify(cfg, null, 2));
    peerEnv = { XDG_CONFIG_HOME: badRoot };
    try {
      await ensurePeer();
      const created = await peer.request('session/new', { cwd: SCRATCH, mcpServers: [] }, 120000);
      const sid = created.sessionId;
      console.log('session/new currentModelId:', created.models?.currentModelId);
      const wanted = 'ollama/nosuch:1b';
      console.log('available:', JSON.stringify((created.models?.availableModels || []).map((m) => m.modelId).filter((m) => m.startsWith('ollama/'))));
      if (created.models?.currentModelId !== wanted) {
        try {
          console.log('session/set_model ->', wanted, JSON.stringify(await peer.request('session/set_model', { sessionId: sid, modelId: wanted })));
        } catch (e) { console.log('session/set_model error', e.message, JSON.stringify(e.rpc || null)); }
      }
      updates = [];
      const a = now();
      try {
        const res = await peer.request('session/prompt', { sessionId: sid, prompt: [{ type: 'text', text: 'Say hi.' }] }, TURN_TIMEOUT_MS);
        console.log(`prompt result after ${now() - a} ms:`, JSON.stringify(res));
      } catch (e) {
        console.log(`prompt error after ${now() - a} ms:`, e.message, JSON.stringify(e.rpc || null));
      }
      const kinds = {};
      for (const x of updates) kinds[x.u.sessionUpdate] = (kinds[x.u.sessionUpdate] || 0) + 1;
      console.log('updates during failed turn:', JSON.stringify(kinds), JSON.stringify(updates.map((x) => x.u).filter((u) => u.sessionUpdate !== 'available_commands_update')).slice(0, 800));
    } finally {
      peerEnv = {};
      if (peer && !peer.exit) { peer.child.stdin.end(); await peer.exitP; }
      peer = null; sessionId = null;
    }
  },
  async 'exit-pending'() {
    // stdin closes while a permission request is pending: what does the agent do?
    await ensurePeer();
    const sid = await freshSession();
    permissionPolicy = 'hold';
    heldPermissions.length = 0;
    let closedAt = null;
    onPermission = () => {
      setTimeout(() => {
        closedAt = now();
        log('event', { stdinClosedWithPendingPermission: true });
        peer.child.stdin.end();
      }, 300);
    };
    updates = [];
    const a = now();
    let outcome;
    try {
      const res = await peer.request('session/prompt', { sessionId: sid, prompt: [{ type: 'text', text: 'Call the write tool with filePath "pending.txt" and content "x". Do not answer with text first, call the tool now.' }] }, TURN_TIMEOUT_MS);
      outcome = 'prompt result ' + JSON.stringify(res);
    } catch (e) {
      outcome = 'prompt rejected: ' + e.message;
    }
    onPermission = null;
    const ex = await Promise.race([peer.exitP, sleep(10000).then(() => null)]);
    console.log(`stdin closed at ${closedAt}; ${outcome}; child: ${ex ? `exited code=${ex.code} signal=${ex.signal} at ${ex.at}` : 'still running 10 s later'}`);
    console.log('pending.txt exists:', fs.existsSync(path.join(SCRATCH, 'pending.txt')));
    if (!ex) peer.child.kill('SIGTERM');
    peer = null; sessionId = null;
  },
  async exit() {
    // termination semantics: close stdin and see what the child does
    await ensurePeer();
    const a = now();
    peer.child.stdin.end();
    const ex = await Promise.race([peer.exitP, sleep(15000).then(() => null)]);
    console.log('after stdin EOF:', ex ? `exited code=${ex.code} signal=${ex.signal} after ${ex.at - a} ms` : 'still running after 15 s');
    if (!ex) {
      peer.child.kill('SIGTERM');
      const e2 = await Promise.race([peer.exitP, sleep(10000).then(() => null)]);
      console.log('after SIGTERM:', e2 ? `code=${e2.code} signal=${e2.signal}` : 'still running');
      if (!e2) peer.child.kill('SIGKILL');
    }
    peer = null;
    sessionId = null;
  },
  async load() {
    const id = state.sessionId;
    if (!id) throw new Error('no saved sessionId; run init first');
    if (peer && !peer.exit) { peer.child.kill('SIGTERM'); await peer.exitP; }
    peer = null;
    await ensurePeer();
    const caps = results.initialize.agentCapabilities || {};
    console.log('loadSession:', caps.loadSession, 'sessionCapabilities:', JSON.stringify(caps.sessionCapabilities));
    updates = [];
    const a = now();
    try {
      const r = await peer.request('session/load', { sessionId: id, cwd: SCRATCH, mcpServers: [] }, 120000);
      const kinds = {};
      for (const x of updates) kinds[x.u.sessionUpdate] = (kinds[x.u.sessionUpdate] || 0) + 1;
      console.log(`session/load ok in ${now() - a} ms: ${JSON.stringify(r)}; replayed updates: ${JSON.stringify(kinds)}`);
      sessionId = id;
    } catch (e) {
      console.log('session/load error:', e.message, JSON.stringify(e.rpc || null));
    }
    if (caps.sessionCapabilities?.resume) {
      try {
        const r = await peer.request('session/resume', { sessionId: id, cwd: SCRATCH, mcpServers: [] });
        console.log('session/resume ok', JSON.stringify(r));
      } catch (e) { console.log('session/resume error', e.message); }
    }
    if (sessionId) {
      permissionPolicy = 'allow';
      const r = await turn(peer, sessionId, 'What was the exact content of hello.txt that you created earlier in this conversation? Answer with that one word only, do not use tools.');
      const text = r.updates.filter((x) => x.u.sessionUpdate === 'agent_message_chunk').map((x) => x.u.content?.text).join('');
      console.log('post-load answer:', JSON.stringify(text));
    }
  },
};

try {
  for (const s of todo) {
    console.log(`\n=== ${s} ===`);
    log('event', { scenario: s });
    try {
      await runners[s]();
    } catch (e) {
      console.log(`scenario ${s} failed:`, e.message, e.rpc ? JSON.stringify(e.rpc) : '');
      log('event', { scenarioError: s, message: e.message, rpc: e.rpc });
    }
  }
} finally {
  if (peer && !peer.exit) {
    peer.child.stdin.end();
    await Promise.race([peer.exitP, sleep(5000)]);
    if (!peer.exit) peer.child.kill('SIGTERM');
  }
}
