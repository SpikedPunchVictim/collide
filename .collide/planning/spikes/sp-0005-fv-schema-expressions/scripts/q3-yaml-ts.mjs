// Q3: YAML in TypeScript for provider frontmatter (untrusted data). Parsers compared:
//   yaml 2.9.1 (core schema, the default), yaml with version 1.1, js-yaml 5.5.0, gray-matter 4.0.3 (bundles js-yaml 3.x),
//   and Ruby Psych (libyaml, YAML 1.1) as an outside oracle for what other tools would read.
import { readFileSync, writeFileSync, mkdirSync, readdirSync, statSync, existsSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';
import YAML from 'yaml';
import * as jsyaml from 'js-yaml';
import matter from 'gray-matter';

mkdirSync('work/q3', { recursive: true });
const pkgVersion = (n) => JSON.parse(readFileSync(`node_modules/${n}/package.json`, 'utf8')).version;
console.log(`versions: yaml ${pkgVersion('yaml')}, js-yaml ${pkgVersion('js-yaml')}, gray-matter ${pkgVersion('gray-matter')}, node ${process.version}`);
const ruby = spawnSync('/usr/bin/ruby', ['-ryaml', '-e', 'print Psych::VERSION + " (libyaml " + Psych::LIBYAML_VERSION + ", ruby " + RUBY_VERSION + ")"'], { encoding: 'utf8' });
console.log(`oracle: Psych ${ruby.stdout}`);

const kind = (v) => {
  if (v === null) return 'null';
  if (v instanceof Date) return `Date(${v.toISOString().slice(0, 10)})`;
  if (typeof v === 'bigint') return `bigint(${v})`;
  if (typeof v === 'object' && v && v.__t) return `${v.__t}(${v.v})`;
  if (typeof v === 'object') return Array.isArray(v) ? `array` : `object(${JSON.stringify(v)})`;
  if (typeof v === 'string') return `str(${JSON.stringify(v)})`;
  return `${typeof v}(${v})`;
};

const PARSERS = {
  'yaml core (default)': (s) => YAML.parse(s),
  'yaml v1.1': (s) => YAML.parse(s, { version: '1.1' }),
  'yaml failsafe': (s) => YAML.parse(s, { schema: 'failsafe' }),
  'js-yaml 5 default': (s) => jsyaml.load(s),
  'gray-matter (js-yaml 3)': (s) => matter(`---\n${s}\n---\n`).data,
};
function psych(docs) {
  const r = spawnSync('/usr/bin/ruby', ['scripts/psych.rb', '-json'], { input: JSON.stringify(docs), encoding: 'utf8', maxBuffer: 1 << 28 });
  if (r.status !== 0) throw new Error(r.stderr);
  return JSON.parse(r.stdout);
}
const tryParse = (f, s) => { try { return { v: f(s) }; } catch (e) { return { err: String(e.message).split('\n')[0].slice(0, 100) }; } };
const field = (r, key) => ('err' in r ? `ERR(${r.err})` : r.v && typeof r.v === 'object' && key in r.v ? kind(r.v[key]) : kind(r.v));

function groupPrint(rows, indent = '  ') {
  const groups = new Map();
  for (const [p, t] of rows) groups.set(t, [...(groups.get(t) || []), p]);
  if (groups.size === 1) return console.log(`${indent}all parsers: ${[...groups.keys()][0]}`);
  for (const [t, ps] of groups) console.log(`${indent}[${ps.join(' | ')}] ${t}`);
}

function table(title, items, mk = (x) => `k: ${x}\n`) {
  console.log(`\n== ${title}`);
  const docs = items.map(mk);
  const ps = psych(docs);
  items.forEach((it, i) => {
    const rows = Object.entries(PARSERS).map(([n, f]) => [n, field(tryParse(f, docs[i]), 'k')]);
    rows.push(['psych 1.1', 'k' in (ps[i] || {}) ? kind(ps[i].k) : `ERR/${JSON.stringify(ps[i]).slice(0, 80)}`]);
    console.log(`\`${it}\``);
    groupPrint(rows);
  });
}

const which = process.argv[2] || 'all';
const run = (n) => which === 'all' || which === n;

if (run('corpus') && !process.env.ORCHARD_ARCH) {
  console.error('ORCHARD_ARCH is not set: set it to the root of the Orchard task corpus (read-only) to run the corpus part');
  process.exit(1);
}

if (run('scalars')) {
  table('hazard scalars, unquoted (`k: <x>`)', ['no', 'yes', 'on', 'off', 'y', 'n', 'NO', 'Null', '~', '0.10', '1.10', '1.10.0', '2026-11-13', '2026-1-3', '2026-11-13T10:00:00Z', '12:30', '1:30:00', '0o17', '017', '0x1f', '0b101', '1e10', '12e3', '123', '1_000', '.5', '+1', '.inf', '.nan', '12345678901234567890', '=', '<<']);
}

// ---- frontmatter fields seen as the plan writes them
if (run('fields')) {
  console.log('\n== planning.md frontmatter shaped examples');
  const doc = `id: sp-4k7q\nalias: S10\nkind: spike\nstatus: no\ntitle: Planning seams\ncreated: 2026-11-13\nowners: [lead]\nlinks:\n  - { type: informs, to: dc-2m8x }\npreregistered: 3f1c2a9\nversion: 0.10\nspend: { usd: 0.85, cap_usd: 4 }\n`;
  const ps = psych([doc])[0];
  const rows = Object.entries(PARSERS).map(([n, f]) => {
    const r = tryParse(f, doc);
    return [n, 'err' in r ? `ERR ${r.err}` : ['id', 'status', 'created', 'preregistered', 'version'].map((k) => `${k}=${kind(r.v[k])}`).join(' ')];
  });
  rows.push(['psych 1.1', ['id', 'status', 'created', 'preregistered', 'version'].map((k) => `${k}=${kind(ps[k])}`).join(' ')]);
  groupPrint(rows);
  console.log('  note: `preregistered: 3f1c2a9` is a git short sha; `3e10` would be a float, `1234567` an int. Same hazard as ids.');
}

// ---- id alphabet, exhaustive
function exhaustive(label, make, count) {
  const lines = [];
  for (let i = 0; i < count; i++) lines.push(`- ${make(i)}`);
  const text = lines.join('\n') + '\n';
  const file = `work/q3/ids-${label}.yaml`;
  writeFileSync(file, text);
  const res = {};
  const check = (name, arr) => {
    const bad = {};
    let nbad = 0;
    arr.forEach((v, i) => {
      const want = make(i);
      if (typeof v !== 'string' || v !== want) {
        nbad++;
        const k = kind(v).replace(/\(.*$/, '');
        bad[k] = (bad[k] || 0) + 1;
        (bad.__examples ||= []).length < 6 && bad.__examples.push(`${want}->${kind(v)}`);
      }
    });
    res[name] = { nbad, bad };
  };
  for (const [name, f] of Object.entries(PARSERS)) {
    if (name === 'gray-matter (js-yaml 3)' || name === 'yaml failsafe') continue;
    const t0 = performance.now();
    const r = tryParse(f, text);
    if ('err' in r) { res[name] = { nbad: 'ERR ' + r.err }; continue; }
    check(name, r.v);
    res[name].ms = Math.round(performance.now() - t0);
  }
  {
    const t0 = performance.now();
    const gm = matter(`---\nids:\n${lines.map((l) => '  ' + l).join('\n')}\n---\n`).data.ids;
    check('gray-matter (js-yaml 3)', gm);
    res['gray-matter (js-yaml 3)'].ms = Math.round(performance.now() - t0);
  }
  {
    const t0 = performance.now();
    const r = spawnSync('/usr/bin/ruby', ['scripts/psych.rb', file], { encoding: 'utf8', maxBuffer: 1 << 29 });
    if (r.status !== 0) res['psych 1.1'] = { nbad: 'ERR ' + r.stderr.slice(0, 100) };
    else { check('psych 1.1', JSON.parse(r.stdout)); res['psych 1.1'].ms = Math.round(performance.now() - t0); }
  }
  console.log(`\n${label}: ${count} ids, e.g. ${make(0)}, ${make(Math.floor(count / 2))}, ${make(count - 1)}`);
  for (const [n, r] of Object.entries(res)) {
    console.log(`  ${n.padEnd(26)} not-a-string: ${String(r.nbad).padStart(8)}  ${r.nbad ? JSON.stringify(r.bad) : ''}  ${r.ms ?? ''} ms`);
  }
}
const B36 = '0123456789abcdefghijklmnopqrstuvwxyz';
const fromIndex = (i, len, alpha) => { let s = ''; for (let k = 0; k < len; k++) { s = alpha[i % alpha.length] + s; i = Math.floor(i / alpha.length); } return s; };
if (run('ids')) {
  console.log('\n== id alphabets: every id of the shape, written as an unquoted list item, per parser');
  // The 4-character runs cover 1.68M ids per shape and take about 4 minutes in all; S5_FULL=1 enables them.
  // Without it the same shapes run with a 3-character suffix (46,656 ids each).
  const L = process.env.S5_FULL ? 4 : 3;
  exhaustive(`bare-base36-len${L}`, (i) => fromIndex(i, L, B36), 36 ** L);
  exhaustive(`prefixed-sp-base36-len${L}`, (i) => 'sp-' + fromIndex(i, L, B36), 36 ** L);
  // other prefixes, shorter suffix lengths: still exhaustive over the suffix
  for (const pre of ['e-', 't-', 'dc-', 'x-']) exhaustive(`prefixed-${pre}base36-len3`, (i) => pre + fromIndex(i, 3, B36), 36 ** 3);
  // bare ids that begin with a letter, and Crockford-style alphabets
  exhaustive(`bare-letterfirst-len${L}`, (i) => 'abcdefghijklmnopqrstuvwxyz'[Math.floor(i / 36 ** (L - 1))] + fromIndex(i % 36 ** (L - 1), L - 1, B36), 26 * 36 ** (L - 1));
}

// ---- stringify hazards: what the TS provider writes, and what other readers make of it
if (run('stringify')) {
  console.log('\n== writing frontmatter: yaml.stringify output for risky strings, and what each parser reads back');
  const items = ['no', 'yes', 'on', 'off', 'y', 'n', 'null', '~', '', '1e10', '0x1f', '0o17', '123', '12e3', '1_000', '.5', '2026-11-13', '1:30', '0.10', 'sp-4k7q', 'a: b', '#x', '{{ x }}'];
  for (const [label, opts] of [['default (core)', {}], ['version 1.1', { version: '1.1' }]]) {
    const outs = items.map((x) => YAML.stringify({ k: x }, opts).trimEnd());
    const ps = psych(outs);
    const bad = [];
    items.forEach((x, i) => {
      const back = {
        'yaml core': tryParse((s) => YAML.parse(s), outs[i]),
        'js-yaml 5': tryParse((s) => jsyaml.load(s), outs[i]),
        'gray-matter': tryParse((s) => matter(`---\n${s}\n---\n`).data, outs[i]),
      };
      const wrong = Object.entries(back).filter(([, r]) => !('v' in r) || r.v.k !== x).map(([n]) => n);
      if (!(typeof ps[i].k === 'string' && ps[i].k === x)) wrong.push('psych 1.1');
      if (wrong.length) bad.push(`${JSON.stringify(x)} written as \`${outs[i].replace('k: ', '')}\` misread by ${wrong.join(', ')}`);
    });
    console.log(`  yaml.stringify ${label}: ${items.length} strings, ${bad.length} misread somewhere${bad.length ? ':' : ''}`);
    bad.forEach((b) => console.log(`    ${b}`));
  }
}

// ---- hazards in untrusted frontmatter
if (run('hazards')) {
  console.log('\n== duplicate keys');
  for (const [n, f] of Object.entries(PARSERS)) {
    if (n === 'yaml failsafe') continue;
    const r = tryParse(f, 'a: x\na: y\n');
    console.log(`  ${n.padEnd(26)} ${'err' in r ? 'rejected: ' + r.err : 'accepted -> ' + JSON.stringify(r.v)}`);
  }
  const pd = psych(['a: x\na: y\n'])[0];
  console.log(`  ${'psych 1.1'.padEnd(26)} accepted -> ${JSON.stringify(pd)}`);

  console.log('\n== error position (unterminated quote on line 3)');
  const bad = 'id: sp-4k7q\nstatus: ok\ntitle: "oops\nz: 1\n';
  try { YAML.parse(bad); } catch (e) { console.log(`  yaml: ${e.message.split('\n')[0]} | linePos=${JSON.stringify(e.linePos)} code=${e.code}`); }
  try { jsyaml.load(bad); } catch (e) { console.log(`  js-yaml 5: ${e.message.split('\n')[0]} | mark.line=${e.mark?.line} column=${e.mark?.column}`); }
  try { matter(`---\n${bad}---\n`); } catch (e) { console.log(`  gray-matter: ${e.message.split('\n')[0]} | mark.line=${e.mark?.line}`); }

  console.log('\n== alias bomb (levels of 9 aliases, then JSON.stringify the result) and depth');
  const bomb = (levels) => {
    let s = 'l0: &l0 [x, x, x, x, x, x, x, x, x]\n';
    for (let i = 1; i <= levels; i++) s += `l${i}: &l${i} [${Array(9).fill(`*l${i - 1}`).join(', ')}]\n`;
    return s;
  };
  for (const levels of [3, 5, 7, 9]) {
    const doc = bomb(levels);
    for (const [n, f] of [['yaml core', (s) => YAML.parse(s)], ['yaml maxAliasCount=-1', (s) => YAML.parse(s, { maxAliasCount: -1 })], ['js-yaml 5', (s) => jsyaml.load(s)]]) {
      const t0 = performance.now();
      const r = tryParse(f, doc);
      const t1 = performance.now();
      let tail = '';
      if ('v' in r && levels <= 7) {
        const t2 = performance.now();
        try { tail = ` | JSON.stringify -> ${JSON.stringify(r.v).length} bytes in ${(performance.now() - t2).toFixed(0)} ms`; } catch (e) { tail = ` | JSON.stringify threw ${String(e.message).slice(0, 60)}`; }
      }
      console.log(`  level ${levels} (9^${levels + 1} = ${9 ** (levels + 1)} leaves) ${n.padEnd(24)} parse ${(t1 - t0).toFixed(1)} ms: ${'err' in r ? 'ERR ' + r.err : 'ok'}${tail}`);
    }
  }
  for (const depth of [1000, 100000]) {
    const doc = `k: ${'['.repeat(depth)}${']'.repeat(depth)}\n`;
    for (const [n, f] of [['yaml core', (s) => YAML.parse(s)], ['js-yaml 5', (s) => jsyaml.load(s)]]) {
      const t0 = performance.now();
      const r = tryParse(f, doc);
      console.log(`  nesting depth ${depth} ${n.padEnd(12)} ${(performance.now() - t0).toFixed(0)} ms: ${'err' in r ? 'ERR ' + r.err : 'ok'}`);
    }
  }

  console.log('\n== `__proto__` key and prototype pollution');
  for (const [n, f] of [['yaml core', (s) => YAML.parse(s)], ['js-yaml 5', (s) => jsyaml.load(s)], ['gray-matter', (s) => matter(`---\n${s}\n---\n`).data]]) {
    const r = tryParse(f, '__proto__:\n  polluted: yes\nconstructor: {prototype: {p2: 1}}\n');
    const v = r.v;
    console.log(`  ${n.padEnd(12)} ${'err' in r ? 'ERR ' + r.err : `own __proto__ key: ${Object.prototype.hasOwnProperty.call(v, '__proto__')}; Object.prototype.polluted: ${({}).polluted}; Object.getPrototypeOf(result) === Object.prototype: ${Object.getPrototypeOf(v) === Object.prototype}`}`);
  }

  console.log('\n== gray-matter language engines (the `---js` fence)');
  globalThis.__s5_ran = false;
  let ran = 'no';
  try {
    matter('---js\n({ id: (globalThis.__s5_ran = true, "x") })\n---\nbody\n');
    ran = globalThis.__s5_ran ? 'YES: JavaScript in the frontmatter was executed' : 'no';
  } catch (e) { ran = `threw ${String(e.message).slice(0, 80)} (ran=${globalThis.__s5_ran})`; }
  console.log(`  matter('---js ...') default options: ${ran}`);
  let ran2 = 'no';
  globalThis.__s5_ran = false;
  try {
    matter('---js\n({ id: (globalThis.__s5_ran = true, "x") })\n---\nbody\n', { engines: { js: () => { throw new Error('js engine disabled'); } } });
  } catch (e) { ran2 = `threw "${e.message}"; code ran: ${globalThis.__s5_ran}`; }
  console.log(`  with engines.js overridden: ${ran2}`);
}

// ---- real corpus: 1604 task files in the Orchard repo, read-only
if (run('corpus')) {
  const root = process.env.ORCHARD_ARCH;
  const files = [];
  const walk = (d) => { for (const e of readdirSync(d, { withFileTypes: true })) { const p = join(d, e.name); if (e.isDirectory()) walk(p); else if (/\/tasks\/T-.*\.md$/.test(p)) files.push(p); } };
  if (!existsSync(root)) { console.log(`\n== corpus: ${root} not found; skipped`); process.exit(0); }
  walk(root);
  const split = (t) => { const m = t.match(/^---\r?\n([\s\S]*?)\r?\n---\r?\n/); return m ? m[1] : null; };
  const fms = files.map((f) => split(readFileSync(f, 'utf8')));
  console.log(`\n== corpus: ${files.length} task files under ${root.replace(process.env.HOME, '~')} (read-only); with frontmatter: ${fms.filter(Boolean).length}`);
  const docs = fms.filter(Boolean);
  const timeParser = (name, f) => {
    const times = [];
    let out;
    for (let i = 0; i < 5; i++) { const t0 = performance.now(); out = docs.map((d) => f(d)); times.push(performance.now() - t0); }
    times.sort((a, b) => a - b);
    console.log(`  ${name.padEnd(24)} median ${times[2].toFixed(0)} ms for ${docs.length} files (${(times[2] / docs.length * 1000).toFixed(0)} us/file)`);
    return out;
  };
  const outCore = timeParser('yaml core', (s) => YAML.parse(s));
  const out11 = timeParser('yaml v1.1', (s) => YAML.parse(s, { version: '1.1' }));
  const outJs = timeParser('js-yaml 5', (s) => jsyaml.load(s));
  // an options object bypasses gray-matter's content cache, which would otherwise make runs 2-5 near free
  const outGm = timeParser('gray-matter (js-yaml 3)', (s) => matter(`---\n${s}\n---\n`, {}).data);
  const t0 = performance.now();
  const outPs = psych(docs);
  console.log(`  ${'psych 1.1 (one ruby run)'.padEnd(24)} ${(performance.now() - t0).toFixed(0)} ms for ${docs.length} files`);
  const norm = (v) => JSON.stringify(v, (k, x) => (x instanceof Date ? { __t: 'Date', v: x.toISOString().slice(0, 10) } : x));
  const normPs = (v) => JSON.stringify(v, (k, x) => (x && x.__t === 'Date' ? { __t: 'Date', v: x.v.slice(0, 10) } : x));
  const diffs = (a, b, na) => a.reduce((n, x, i) => n + (na(x) !== na(b[i]) ? 1 : 0), 0);
  console.log(`  files whose parse differs from yaml core: yaml v1.1 ${diffs(outCore, out11, norm)}, js-yaml 5 ${diffs(outCore, outJs, norm)}, gray-matter ${diffs(outCore, outGm, norm)}`);
  console.log(`  files whose parse differs from yaml core: psych 1.1 ${outCore.reduce((n, x, i) => n + (norm(x) !== normPs(outPs[i]) ? 1 : 0), 0)}`);
  const census = {};
  for (const o of outCore) for (const [k, v] of Object.entries(o || {})) {
    const t = Array.isArray(v) ? `array<${[...new Set(v.map((x) => typeof x))].join('|') || 'empty'}>` : v === null ? 'null' : typeof v;
    (census[k] ||= {})[t] = (census[k][t] || 0) + 1;
  }
  console.log('  frontmatter key -> value types (yaml core):');
  for (const [k, v] of Object.entries(census)) console.log(`    ${k.padEnd(16)} ${JSON.stringify(v)}`);
  const statusVals = {};
  for (const o of outCore) statusVals[o.status] = (statusVals[o.status] || 0) + 1;
  console.log(`  status values: ${JSON.stringify(statusVals)}`);
  const ids = new Set();
  for (const f of files) ids.add(f.split('/').pop().replace(/\.md$/, ''));
  console.log(`  file stems that parse as non-strings when written as plain scalars: ${[...ids].filter((s) => typeof YAML.parse(s) !== 'string').length} of ${ids.size}`);
}
