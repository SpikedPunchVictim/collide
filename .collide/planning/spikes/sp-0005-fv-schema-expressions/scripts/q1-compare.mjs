// Q1: compares serde (ground truth), the schemars JSON Schema (ajv) and three generated TypeScript type sets
// (json-schema-to-typescript from the schema, ts-rs, specta) on the same cases. Needs work/q1-out/ from `emit`
// and `verdict` (scripts/q1.sh runs them).
import { execFileSync, spawnSync } from 'node:child_process';
import { readFileSync, writeFileSync, mkdirSync, existsSync } from 'node:fs';
import { compile } from 'json-schema-to-typescript';
import Ajv2020 from 'ajv/dist/2020.js';

const out = 'work/q1-out';
const cases = JSON.parse(readFileSync('fixtures/q1-cases.json', 'utf8'));
const verdicts = JSON.parse(readFileSync(`${out}/verdict.json`, 'utf8'));
const schema = JSON.parse(readFileSync(`${out}/schema.json`, 'utf8'));

// 1. schema -> TS with json-schema-to-typescript (default options).
const dts = await compile(schema, 'PackManifest', { bannerComment: '', additionalProperties: true });
writeFileSync(`${out}/jsts.ts`, dts);

// 2. ajv. Strict mode first, to record the friction, then the working configuration.
let strictNote;
try {
  new Ajv2020({ strict: true }).compile(schema);
  strictNote = 'ajv strict mode: compiles';
} catch (e) {
  strictNote = `ajv strict mode: ${String(e.message).split('\n')[0]}`;
}
console.log(strictNote);
const ajv = new Ajv2020({ strict: true, formats: Object.fromEntries(['int8','int16','int32','int64','uint8','uint16','uint32','uint64','float','double'].map((f) => [f, true])) });
const validate = ajv.compile(schema);

// 3. one TS file per generator, one declaration per line so that tsc line numbers map to cases.
const generators = {
  'json-schema-to-typescript': { import: `import type { PackManifest } from './jsts';` },
  'ts-rs': { import: `import type { PackManifest } from './ts-rs/PackManifest';` },
  'ts-rs+as': { import: `import type { PackManifest } from './ts-rs-optional/PackManifest';` },
  specta: { import: `import type { PackManifest } from './specta';` },
};
writeFileSync(`${out}/tsconfig.json`, JSON.stringify({
  compilerOptions: { strict: true, noEmit: true, module: 'esnext', moduleResolution: 'bundler', target: 'es2022', skipLibCheck: false, types: [] },
  include: ['check-*.ts'],
}, null, 2));

const rows = cases.map((c, i) => ({
  name: c.name,
  value: c.value,
  serde: verdicts[i].serde,
  schema: validate(c.value) ? 'ok' : 'err',
  reserialized: verdicts[i].reserialized,
  idempotent: verdicts[i].idempotent,
  schemaOnReserialized: verdicts[i].reserialized ? (validate(verdicts[i].reserialized) ? 'ok' : 'err') : '-',
}));

const tscBin = 'node_modules/.bin/tsc';
function tscErrors(file) {
  const r = spawnSync(tscBin, ['-p', `${out}/tsconfig.json`, '--pretty', 'false'], { encoding: 'utf8' });
  return r.stdout + r.stderr;
}
// Typecheck each generator in turn (tsconfig includes check-*.ts, so write one at a time).
const ts = {};
for (const [g, { import: imp }] of Object.entries(generators)) {
  const lines = [imp];
  const index = {};
  rows.forEach((r, i) => {
    lines.push(`export const c${i}: PackManifest = ${JSON.stringify(r.value)};`);
    index[lines.length] = { kind: 'input', i };
    if (r.reserialized) {
      lines.push(`export const r${i}: PackManifest = ${JSON.stringify(r.reserialized)};`);
      index[lines.length] = { kind: 'reserialized', i };
    }
  });
  for (const f of Object.keys(generators)) writeFileSync(`${out}/check-${f.replace(/[^a-z]/g, '')}.ts`, 'export {};\n');
  const file = `check-${g.replace(/[^a-z]/g, '')}.ts`;
  writeFileSync(`${out}/${file}`, lines.join('\n') + '\n');
  const text = tscErrors(file);
  const bad = { input: new Set(), reserialized: new Set() };
  let other = [];
  for (const l of text.split('\n')) {
    const m = l.match(/^(?:.*\/)?check-[a-z]+\.ts\((\d+),\d+\): error (TS\d+): (.*)/);
    if (!m) continue;
    const hit = index[Number(m[1])];
    if (hit) bad[hit.kind].add(hit.i);
    else other.push(l);
  }
  ts[g] = { bad, other };
  if (other.length) console.log(`  [${g}] tsc errors outside case lines: ${other.slice(0, 3).join(' | ')}`);
}

// 4. report
const pad = (s, n) => String(s).padEnd(n);
console.log('\nPer case: serde | schema (ajv) | typecheck of the value as an object literal per generator (ok/err)');
console.log(pad('case', 58), pad('serde', 6), pad('schema', 7), Object.keys(generators).map((g) => pad(g.slice(0, 8), 9)).join(''));
const mism = Object.fromEntries(Object.keys(generators).map((g) => [g, { 'valid-by-serde rejected by TS': [], 'invalid-by-serde accepted by TS': [] }]));
const schemaMism = { 'valid-by-serde rejected by schema': [], 'invalid-by-serde accepted by schema': [] };
rows.forEach((r, i) => {
  const cells = Object.keys(generators).map((g) => (ts[g].bad.input.has(i) ? 'err' : 'ok'));
  console.log(pad(r.name, 58), pad(r.serde, 6), pad(r.schema, 7), cells.map((c) => pad(c, 9)).join(''));
  Object.keys(generators).forEach((g, k) => {
    if (r.serde === 'ok' && cells[k] === 'err') mism[g]['valid-by-serde rejected by TS'].push(r.name);
    if (r.serde === 'err' && cells[k] === 'ok') mism[g]['invalid-by-serde accepted by TS'].push(r.name);
  });
  if (r.serde === 'ok' && r.schema === 'err') schemaMism['valid-by-serde rejected by schema'].push(r.name);
  if (r.serde === 'err' && r.schema === 'ok') schemaMism['invalid-by-serde accepted by schema'].push(r.name);
});

console.log('\nSchema against serde:');
for (const [k, v] of Object.entries(schemaMism)) console.log(`  ${k}: ${v.length}${v.length ? '  -> ' + v.join('; ') : ''}`);
console.log('\nTypeScript against serde (object-literal check, so excess properties are caught):');
for (const g of Object.keys(generators)) {
  console.log(`  ${g}`);
  for (const [k, v] of Object.entries(mism[g])) console.log(`    ${k}: ${v.length}${v.length ? '  -> ' + v.join('; ') : ''}`);
}

console.log('\nRound trip (JSON -> Rust -> JSON):');
const okRows = rows.filter((r) => r.serde === 'ok');
console.log(`  values accepted by serde: ${okRows.length}; Rust->JSON->Rust idempotent: ${okRows.filter((r) => r.idempotent).length}`);
console.log(`  re-serialized value valid per schema: ${okRows.filter((r) => r.schemaOnReserialized === 'ok').length}/${okRows.length}`);
for (const g of Object.keys(generators)) {
  const reBad = [...ts[g].bad.reserialized].filter((i) => rows[i].serde === 'ok');
  console.log(`  re-serialized value typechecks under ${g}: ${okRows.length - reBad.length}/${okRows.length}${reBad.length ? '  fails: ' + reBad.map((i) => rows[i].name).join('; ') : ''}`);
}

console.log('\nPhase as generated:');
for (const [label, file] of [['json-schema-to-typescript', 'jsts.ts'], ['ts-rs', 'ts-rs/Phase.ts'], ['ts-rs+as', 'ts-rs-optional/Phase.ts'], ['specta', 'specta.ts']]) {
  const t = readFileSync(`${out}/${file}`, 'utf8');
  const m = t.match(/export (?:type|interface) Phase\b[\s\S]*?(?=\nexport |\n*$)/);
  console.log(`--- ${label}\n${m ? m[0].trim() : '(not found)'}`);
}
const ser = JSON.parse(readFileSync(`${out}/schema.serialize.json`, 'utf8'));
const agentOf = (sc) => sc.$defs.Phase.oneOf.find((v) => v.properties.kind.const === 'agent');
console.log('\nschemars contract: Phase.agent required[] when deserializing:', JSON.stringify(agentOf(schema).required),
  ' when serializing:', JSON.stringify(agentOf(ser).required));
console.log('Top-level required[] deserialize:', JSON.stringify(schema.required), ' serialize:', JSON.stringify(ser.required));
console.log('\nSizes (bytes): schema.json', readFileSync(`${out}/schema.json`).length,
  ' jsts.ts', readFileSync(`${out}/jsts.ts`).length,
  ' specta.ts', readFileSync(`${out}/specta.ts`).length,
  ' ts-rs total', ['PackManifest', 'Phase', 'ProtectedPaths', 'When', 'Workflow'].map((n) => readFileSync(`${out}/ts-rs/${n}.ts`).length).reduce((a, b) => a + b, 0));
