// Node client for the prototype daemon/shim frame protocol (see crates/s2lib/src/frame.rs).
import net from 'node:net';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const SPIKE = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
export const STATE = process.env.S2_STATE || path.join(SPIKE, 'work/state');
export const S2D = path.join(SPIKE, 'target/debug/s2d');
export const CLEAN = path.join(SPIKE, 'claude-clean.sh');
export const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
export const nowMs = () => Date.now() + (performance.now() % 1);

export function frame(type, payload) {
  const p = Buffer.isBuffer(payload) ? payload : Buffer.from(payload);
  const h = Buffer.alloc(5);
  h[0] = type.charCodeAt(0);
  h.writeUInt32BE(p.length, 1);
  return Buffer.concat([h, p]);
}

export class Conn {
  constructor(sockPath) {
    this.sock = net.connect(sockPath);
    this.buf = Buffer.alloc(0);
    this.handlers = [];
    this.closed = false;
    this.ready = new Promise((res, rej) => { this.sock.once('connect', res); this.sock.once('error', rej); });
    this.sock.on('data', (d) => {
      this.buf = Buffer.concat([this.buf, d]);
      while (this.buf.length >= 5) {
        const n = this.buf.readUInt32BE(1);
        if (this.buf.length < 5 + n) break;
        const t = String.fromCharCode(this.buf[0]);
        const p = this.buf.subarray(5, 5 + n);
        this.buf = this.buf.subarray(5 + n);
        const at = performance.now();
        for (const h of this.handlers) h(t, p, at);
      }
    });
    this.sock.on('close', () => { this.closed = true; for (const h of this.handlers) h('Z', Buffer.alloc(0), performance.now()); });
    this.sock.on('error', () => {});
  }
  send(type, payload) { this.sock.write(frame(type, payload)); }
  next(types) {
    return new Promise((res) => {
      const h = (t, p) => { if (types.includes(t)) { this.handlers = this.handlers.filter((x) => x !== h); res([t, p]); } };
      this.handlers.push(h);
    });
  }
  close() { this.sock.destroy(); }
}

export async function rpc(cmd, state = STATE) {
  const c = new Conn(path.join(state, 'daemon.sock'));
  await c.ready;
  const w = c.next(['J', 'Z']);
  c.send('C', JSON.stringify(cmd));
  const [t, p] = await w;
  c.close();
  if (t === 'Z') throw new Error('daemon closed');
  return JSON.parse(p.toString());
}

export async function spawnRun(spec, state = STATE) {
  const r = await rpc({ cmd: 'spawn', spec }, state);
  if (!r.ok) throw new Error('spawn failed: ' + JSON.stringify(r));
  return r;
}

// Attach through the daemon. Collects output; `text()` is everything seen so far (lossy utf8).
export async function attach(id, { rows = 40, cols = 120, state = STATE, direct = false } = {}) {
  const sockPath = direct ? path.join(state, 'runs', id, 'shim.sock') : path.join(state, 'daemon.sock');
  const c = new Conn(sockPath);
  await c.ready;
  if (!direct) {
    const w = c.next(['J', 'Z']);
    c.send('C', JSON.stringify({ cmd: 'attach', id }));
    const [t, p] = await w;
    if (t === 'Z' || !JSON.parse(p.toString()).ok) throw new Error('attach failed ' + p.toString());
  }
  const a = { c, chunks: [], total: 0, exit: null, hello: null, closed: false, listeners: [] };
  c.handlers.push((t, p, at) => {
    if (t === 'H') a.hello = JSON.parse(p.toString());
    else if (t === 'O') { a.chunks.push({ at, buf: Buffer.from(p) }); a.total += p.length; for (const l of a.listeners) l(p, at); }
    else if (t === 'X') a.exit = JSON.parse(p.toString());
    else if (t === 'Z') a.closed = true;
  });
  a.text = () => Buffer.concat(a.chunks.map((x) => x.buf)).toString('utf8');
  a.input = (s) => c.send('I', s);
  a.resize = (r, co) => { const b = Buffer.alloc(4); b.writeUInt16BE(r, 0); b.writeUInt16BE(co, 2); c.send('R', b); };
  a.closeStdin = () => c.send('E', '');
  a.waitFor = async (re, ms = 10000) => {
    const t0 = Date.now();
    while (Date.now() - t0 < ms) { if (re instanceof RegExp ? re.test(a.text()) : a.text().includes(re)) return true; await sleep(50); }
    return false;
  };
  a.close = () => c.close();
  if (!direct) a.resize(rows, cols); else a.resize(rows, cols);
  return a;
}

export async function listRuns(state = STATE) { return (await rpc({ cmd: 'list' }, state)).runs; }
export const strip = (s) => s.replace(/\x1b\[(\d*)C/g, (_, n) => ' '.repeat(+n || 1)).replace(/\x1b\[[0-9;?<>]*[ -\/]*[@-~]/g, '').replace(/\x1b\][^\x07]*\x07/g, '').replace(/\r/g, '');
