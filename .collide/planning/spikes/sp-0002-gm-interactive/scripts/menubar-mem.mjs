#!/usr/bin/env node
// Menu bar memory check: launches S2Spike.app with S2_MENUBAR=<destroy|hide>, follows the app's marks
// (out/menubar/marks.jsonl) and samples memory footprint of the app and of the WebKit processes in the
// app's coalition: window open, 5 s and 35 s after the window is closed, and after it is reopened.
// Usage: node scripts/menubar-mem.mjs destroy|hide
import { execFileSync, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

const SPIKE = path.resolve(path.dirname(new URL(import.meta.url).pathname), '..');
const APP = `${SPIKE}/target/debug/bundle/macos/S2Spike.app`;
const BIN = `${APP}/Contents/MacOS/s2-spike-app`;
const S2D = `${SPIKE}/target/debug/s2d`;
const OUT = `${SPIKE}/out/menubar`;
const MARKS = `${OUT}/marks.jsonl`;
const mode = process.argv[2];
if (mode !== 'destroy' && mode !== 'hide') throw new Error('usage: menubar-mem.mjs destroy|hide');

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const MiB = (b) => Math.round((b / 1048576) * 10) / 10;

function appPids() {
  const r = spawnSync('pgrep', ['-f', BIN], { encoding: 'utf8' });
  return r.stdout.split('\n').filter(Boolean).map(Number);
}

function marks() {
  if (!fs.existsSync(MARKS)) return {};
  const m = {};
  for (const line of fs.readFileSync(MARKS, 'utf8').split('\n').filter(Boolean)) {
    const e = JSON.parse(line);
    m[e.ev] = e.t_ms;
  }
  return m;
}

async function waitMark(ev, timeoutMs) {
  const t0 = Date.now();
  while (Date.now() - t0 < timeoutMs) {
    const m = marks();
    if (m[ev] !== undefined) return m[ev];
    const err = Object.keys(m).find((k) => k.startsWith('error:'));
    if (err) throw new Error(`app reported ${err}`);
    await sleep(100);
  }
  throw new Error(`timed out waiting for mark ${ev}`);
}

async function untilTime(tMs) {
  const d = tMs - Date.now();
  if (d > 0) await sleep(d);
}

function coalitionOf(pids) {
  if (!pids.length) return new Map();
  const out = execFileSync(S2D, ['coalition', ...pids.map(String)], { encoding: 'utf8' });
  return new Map(out.split('\n').filter(Boolean).map((l) => { const j = JSON.parse(l); return [j.pid, j.coalition]; }));
}

// The app plus every WebKit process of this user that shares the app's resource coalition.
function members(appPid) {
  const ps = execFileSync('ps', ['-x', '-o', 'pid=,rss=,comm='], { encoding: 'utf8' });
  const rows = ps.split('\n').filter(Boolean).map((l) => {
    const m = l.trim().match(/^(\d+)\s+(\d+)\s+(.*)$/);
    return { pid: Number(m[1]), rss_kb: Number(m[2]), comm: m[3] };
  });
  const webkit = rows.filter((r) => r.comm.includes('com.apple.WebKit'));
  const coal = coalitionOf([appPid, ...webkit.map((r) => r.pid)]);
  const mine = coal.get(appPid);
  const app = rows.find((r) => r.pid === appPid);
  return [app, ...webkit.filter((r) => coal.get(r.pid) === mine)].filter(Boolean).map((r) => ({ ...r, coalition: coal.get(r.pid) }));
}

function footprints(pids) {
  const tmp = `${OUT}/fp-${process.pid}.json`;
  execFileSync('footprint', ['-f', 'bytes', '-j', tmp, ...pids.flatMap((p) => ['-p', String(p)])], { stdio: 'ignore' });
  const j = JSON.parse(fs.readFileSync(tmp, 'utf8'));
  fs.rmSync(tmp, { force: true });
  return new Map(j.processes.map((p) => [p.pid, p.footprint]));
}

function sample(label, appPid) {
  const procs = members(appPid);
  const fp = footprints(procs.map((p) => p.pid));
  const rows = procs.map((p) => ({ pid: p.pid, name: path.basename(p.comm), footprint_mib: MiB(fp.get(p.pid) ?? 0), rss_mib: MiB(p.rss_kb * 1024) }));
  const total = rows.reduce((s, r) => s + r.footprint_mib, 0);
  const rss = rows.reduce((s, r) => s + r.rss_mib, 0);
  return { label, t_ms: Date.now(), total_footprint_mib: Math.round(total * 10) / 10, total_rss_mib: Math.round(rss * 10) / 10, procs: rows };
}

async function main() {
  fs.mkdirSync(OUT, { recursive: true });
  if (appPids().length) throw new Error('S2Spike.app is already running; quit it first');
  fs.rmSync(MARKS, { force: true });
  const r = spawnSync('open', ['-n', '--env', `S2_MENUBAR=${mode}`, APP], { encoding: 'utf8' });
  if (r.status !== 0) throw new Error(`open failed: ${r.stderr}`);
  let appPid;
  for (let i = 0; i < 100 && !appPid; i++) { appPid = appPids()[0]; if (!appPid) await sleep(100); }
  if (!appPid) throw new Error('app did not start');

  const samples = [];
  try {
    const loaded = await waitMark('loaded:main', 30000);
    await untilTime(loaded + 10000);
    samples.push(sample('window open (idle xterm page)', appPid));

    const closed = await waitMark('closed', 30000);
    await untilTime(closed + 5000);
    samples.push(sample('window closed +5 s', appPid));
    await untilTime(closed + 35000);
    samples.push(sample('window closed +35 s', appPid));

    const reopenBegin = await waitMark('reopen-begin', 30000);
    const reopenReturned = await waitMark('reopen-returned', 30000);
    let reopenLoaded = null;
    if (mode === 'destroy') reopenLoaded = await waitMark('loaded:main2', 30000);
    await untilTime((reopenLoaded ?? reopenReturned) + 10000);
    samples.push(sample('window reopened', appPid));

    const m = marks();
    const result = {
      mode,
      app_pid: appPid,
      reopen_ms: {
        begin_to_returned: Math.round(reopenReturned - reopenBegin),
        begin_to_page_loaded: reopenLoaded ? Math.round(reopenLoaded - reopenBegin) : null,
      },
      first_load_ms_after_tray: m[`tray-ready:${mode}`] ? Math.round(m['loaded:main'] - m[`tray-ready:${mode}`]) : null,
      samples,
    };
    await waitMark('exit', 60000);
    for (let i = 0; i < 100 && appPids().length; i++) await sleep(100);
    result.app_exited = appPids().length === 0;
    await sleep(3000);
    const ps = execFileSync('ps', ['-x', '-o', 'pid='], { encoding: 'utf8' });
    const seen = new Set(samples.flatMap((s) => s.procs.map((p) => p.pid)));
    result.webkit_still_alive_3s_after_exit = ps.split('\n').filter(Boolean).map((l) => Number(l.trim())).filter((p) => seen.has(p) && p !== appPid);
    fs.writeFileSync(`${OUT}/menubar-${mode}.json`, JSON.stringify(result, null, 2));
    console.log(JSON.stringify(result, null, 2));
  } finally {
    // only ever signal the app this script launched
    if (appPids().includes(appPid)) { try { process.kill(appPid, 'SIGTERM'); } catch {} }
  }
}

main().catch((e) => { console.error(String(e)); process.exit(1); });
