const { invoke, Channel } = window.__TAURI__.core;
const bar = document.getElementById('bar');
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const enc = new TextEncoder();
const dec = new TextDecoder();
const report = (name, data) => invoke('report', { name, data });

function stats(a) {
  const s = [...a].sort((x, y) => x - y);
  const q = (p) => s[Math.min(s.length - 1, Math.floor(p * s.length))];
  return { n: s.length, min: s[0], p50: q(0.5), p90: q(0.9), p95: q(0.95), max: s[s.length - 1], mean: s.reduce((x, y) => x + y, 0) / s.length };
}

async function main() {
  const cfg = await invoke('app_config');
  const info = await invoke('app_info');
  const term = new Terminal({ fontFamily: 'Menlo, monospace', fontSize: cfg.fontSize || 13, scrollback: 5000, allowProposedApi: true });
  const fit = new (FitAddon.FitAddon || FitAddon)();
  term.loadAddon(fit);
  term.open(document.getElementById('term'));
  let renderer = 'dom';
  if (cfg.renderer === 'webgl') {
    try { term.loadAddon(new (WebglAddon.WebglAddon || WebglAddon)()); renderer = 'webgl'; } catch (e) { renderer = 'dom (webgl failed: ' + e + ')'; }
  }
  fit.fit();

  let conn = null;
  let probe = null;
  let lastOut = performance.now();
  let bytesIn = 0, chunks = 0, exitInfo = null;
  const sendBytes = (u8) => invoke('write_input', u8, { headers: { conn: String(conn) } });
  const sendText = (s) => sendBytes(enc.encode(s));
  term.onData((d) => { if (conn) sendText(d); });
  let resizeSeen = null;
  term.onResize(({ rows, cols }) => {
    resizeSeen = { t: performance.now(), rows, cols };
    if (conn) invoke('resize', { conn, rows, cols });
  });
  new ResizeObserver(() => { try { fit.fit(); } catch (e) {} }).observe(document.getElementById('term'));

  const ch = new Channel();
  ch.onmessage = (m) => {
    const t = performance.now();
    const u8 = m instanceof ArrayBuffer ? new Uint8Array(m) : Array.isArray(m) ? Uint8Array.from(m) : m;
    const type = String.fromCharCode(u8[0]);
    const body = u8.subarray(1);
    if (type === 'O') {
      lastOut = t; bytesIn += body.length; chunks++;
      if (probe && probe.tArrive === null && dec.decode(body).includes(probe.ch)) {
        const p = probe; p.tArrive = t;
        term.write(body, () => { p.tParsed = performance.now(); requestAnimationFrame(() => { p.tFrame = performance.now(); p.resolve(); }); });
        return;
      }
      term.write(body);
    } else if (type === 'X') { exitInfo = JSON.parse(dec.decode(body)); bar.textContent += '  [exited]'; }
    else if (type === 'Z') { bar.textContent += '  [conn closed]'; }
  };

  const screenText = () => {
    const b = term.buffer.active; const out = [];
    for (let i = 0; i < b.length; i++) out.push(b.getLine(i).translateToString(true));
    while (out.length && out[out.length - 1] === '') out.pop();
    return out.join('\n');
  };
  const waitFor = async (text, timeout = 20000) => {
    const t0 = performance.now();
    while (performance.now() - t0 < timeout) { if (screenText().includes(text)) return true; await sleep(100); }
    return false;
  };
  const quiet = async (ms = 800, max = 15000) => {
    const t0 = performance.now();
    while (performance.now() - t0 < max) { if (performance.now() - lastOut > ms) return true; await sleep(50); }
    return false;
  };

  if (cfg.attach) {
    conn = await invoke('attach', { id: cfg.attach, rows: term.rows, cols: term.cols, onData: ch });
    bar.textContent = `attach ${cfg.attach} conn ${conn} app pid ${info.pid} renderer ${renderer} ${term.cols}x${term.rows}`;
  } else {
    bar.textContent = `no attach; app pid ${info.pid}`;
  }

  const results = {};
  for (const step of cfg.bench || []) {
    try {
      if (step.type === 'wait') await sleep(step.ms);
      else if (step.type === 'quiet') await quiet(step.ms, step.max);
      else if (step.type === 'waitfor') results['waitfor:' + step.text] = await waitFor(step.text, step.timeout);
      else if (step.type === 'send') term.input(step.text, true);
      else if (step.type === 'type') { for (const c of step.text) { term.input(c, true); await sleep(step.gap || 20); } }
      else if (step.type === 'dump') await report('screen-' + step.name, { text: screenText(), rows: term.rows, cols: term.cols });
      else if (step.type === 'snap') { await sleep(step.delay || 300); await invoke('snapshot', { name: step.name }); await sleep(300); }
      else if (step.type === 'mark') await report('mark-' + step.name, { t: Date.now(), rows: term.rows, cols: term.cols });
      else if (step.type === 'latency') {
        let pool = step.pool;
        if (!pool) { const scr = screenText(); pool = [...'QZXJKWVYGFDBPNRTMLqzxjkwvygfdbpnrtml~^&|{}=;+'].filter((c) => !scr.includes(c)).slice(0, 18).join(''); }
        const rows = [];
        const t0all = performance.now();
        let typed = 0;
        for (let i = 0; i < step.n; i++) {
          const c = pool[i % pool.length];
          if (i % pool.length === 0 && i > 0) { for (let k = 0; k < pool.length; k++) term.input('\x7f', true); await sleep(400); }
          probe = { ch: c, t0: performance.now(), tArrive: null, tParsed: null, tFrame: null };
          const done = new Promise((r) => { probe.resolve = r; });
          term.input(c, true);
          const to = sleep(2000).then(() => 'timeout');
          const r = await Promise.race([done, to]);
          if (r === 'timeout') rows.push({ c, timeout: true }); else rows.push({ c, echo_ms: probe.tArrive - probe.t0, parsed_ms: probe.tParsed - probe.t0, frame_ms: probe.tFrame - probe.t0 });
          probe = null;
          await sleep(step.gap || 120);
        }
        for (let k = 0; k < pool.length; k++) term.input('\x7f', true);
        const ok = rows.filter((r) => !r.timeout);
        results['latency:' + step.label] = { renderer, timeouts: rows.length - ok.length, echo: stats(ok.map((r) => r.echo_ms)), parsed: stats(ok.map((r) => r.parsed_ms)), frame: stats(ok.map((r) => r.frame_ms)), raw: rows };
        await report('latency-' + step.label, results['latency:' + step.label]);
      }
      else if (step.type === 'resize') {
        const { LogicalSize } = window.__TAURI__.dpi;
        const win = window.__TAURI__.window.getCurrentWindow();
        const seq = [];
        for (let i = 0; i < step.sizes.length; i++) {
          const [w, h] = step.sizes[i];
          const before = { rows: term.rows, cols: term.cols };
          const c0 = chunks; resizeSeen = null; lastOut = 0;
          const t0 = performance.now();
          await win.setSize(new LogicalSize(w, h));
          const tw = performance.now();
          while (!resizeSeen && performance.now() - tw < 3000) await sleep(5);
          const tResize = resizeSeen ? resizeSeen.t - t0 : null;
          // first output after resize = the child's redraw
          while (lastOut === 0 && performance.now() - tw < 5000) await sleep(5);
          const tRedraw = lastOut ? lastOut - t0 : null;
          await quiet(500, 8000);
          const tQuiet = performance.now() - t0 - 500;
          let stty = null;
          if (step.stty) { term.input('stty size\r', true); await sleep(600); const m = screenText().split('\n').filter((l) => /^\d+ \d+$/.test(l.trim())).pop(); stty = m ? m.trim() : null; }
          seq.push({ w, h, before, after: { rows: term.rows, cols: term.cols }, to_xterm_resize_ms: tResize, to_first_redraw_ms: tRedraw, to_quiet_ms: tQuiet, chunks: chunks - c0, stty });
          if (step.snapPrefix) { await invoke('snapshot', { name: step.snapPrefix + i }); await sleep(300); }
          await report('mark-resize-' + i, { t: Date.now(), rows: term.rows, cols: term.cols });
          await sleep(step.hold || 3500);
        }
        results['resize'] = seq;
        await report('resize', seq);
      }
    } catch (e) { results['error:' + step.type] = String(e); await report('error', { step, e: String(e) }); }
  }
  if (cfg.bench) await report('done', { results: Object.keys(results), exitInfo, renderer });
}
main().catch((e) => { document.getElementById('bar').textContent = 'ERR ' + e; invoke('report', { name: 'fatal', data: String(e) }); });
