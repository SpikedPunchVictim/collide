---
id: sp-0005-fv
alias: S5
kind: spike
status: results
title: "S5: schema and expressions"
owners: []
links: []
---
# S5: schema and expressions

- **Date:** 2026-11-13
- **Spike:** Stage 1, S5. Plan revision 10 (frozen); this record only proposes changes and does not edit
  `IMPLEMENTATION_PLAN.md`.
- **Reproduction:** `.collide/planning/spikes/sp-0005-fv-schema-expressions/` (its `README.md` lists the scripts). `scripts/run-all.sh` re-runs
  every measurement and prints the numbers. It was run from a copy of the directory without `work/`,
  `node_modules/` or `.pnpm-store/`.

Labels: **[M]** measured here, **[I]** inferred from measurements or documented behaviour and not observed,
**[U]** not measured.

## Versions and conditions

| Item | Version |
| --- | --- |
| macOS, rustc, cargo | 26.3.1 arm64; 1.98.0 in `~/.cargo/bin` (not on PATH) |
| Node, pnpm | v24.18.0; 11.17.0 |
| `schemars` | 1.2.3 |
| `ts-rs` | 12.0.1 (features `serde-compat`, `serde-json-impl`) |
| `specta` | `=2.0.0-rc.28` (still a release candidate), with `specta-typescript` 0.0.15 and `specta-serde` 0.0.15. The crate's highest non-RC version, 1.0.5, is the old line and does not pair with them |
| `serde_yaml` | 0.9.34+deprecated (archived) |
| `serde_yaml_ng`, `serde_norway` | 0.10.0, 0.9.42 |
| `serde-saphyr` | 2.1.1 (parser `granit-parser` 2.1.0); raw trees: `saphyr` 0.3.4, `yaml-rust2` 0.13.0 (not run through serde) |
| `cel` | 0.15.0 (the cel-rust project, formerly `cel-interpreter` 0.10.0) |
| npm | `yaml` 2.9.1, `js-yaml` 5.5.0, `gray-matter` 4.0.3 (bundles js-yaml 3.x), `ajv` 8.20.0, `json-schema-to-typescript` 16.0.0, `typescript` 7.0.2, `@types/node` 26.6.6 |
| YAML 1.1 oracle | Ruby 2.6.10, Psych 3.1.0 (libyaml 0.2.1) |

**The machine was loaded.** The load average was 7 to 17 during the CEL runs. Every timing below is an upper bound;
comparisons within one run are more reliable than absolute numbers. Crate and package pages were read as data only;
nothing in them was followed. The Orchard task corpus was read, never written.

## Summary

Recommendations first. Each is justified in its section.

1. **Schema pipeline: `schemars` to JSON Schema, then `json-schema-to-typescript`.** The schema is the contract that
   rejects unknown keys (0 of 13 invalid cases accepted, `additionalProperties: false` on every struct and tagged
   variant). Generated TypeScript is a discriminated union and accepts every valid case. `specta` is the closest
   second and the better fit only if the Tauri command types are wanted from the same derive. `ts-rs` is not
   recommended: it types `#[serde(default)]` fields as required and needs four per-field overrides plus still
   rejects `null`. Use `Wait {}` rather than a unit variant in internally tagged enums, because serde does not
   enforce `deny_unknown_fields` on a unit variant.
2. **YAML crate: `serde-saphyr` with a hardened option set, never its defaults.** Defaults accept YAML 1.1 booleans
   (`no` becomes `false`), which breaks the plan's "`no` stays a string" promise. With `strict_booleans`,
   `no_schema`, merge keys as errors, and a budget (no anchors, 100,000 nodes, depth 32), it passed every hazard:
   duplicate keys rejected everywhere (also inside maps, where the serde_yaml family silently keeps the last),
   unquoted `{{ }}` rejected at the right column, alias bombs and 100,000-deep nesting stopped in under 2 ms.
   Pure Rust, `unsafe` forbidden, active. Runner-up: `serde_yaml_ng`.
3. **YAML in TypeScript: the `yaml` package in its default (YAML 1.2 core) mode.** Do not use `gray-matter`'s
   default: it executes JavaScript from a `---js` fence. Do not set `version: '1.1'` (`yaml` turns `E-07` into NaN
   there). Set a size cap, keep `uniqueKeys` and the alias limit, and validate every field's type instead of
   coercing.
4. **Id rule: `^[a-z]{2,}-[0-9a-z]{4,}$`** (at least two lowercase letters, a hyphen, a base36 tail). `sp-` plus
   four base36 characters (1,679,616 ids) came out as a string in all five parsers with 0 exceptions. Bare 4-char
   base36 ids fail 0.73% of the time (12,322 of 1,679,616 in `yaml` core). A one-letter prefix is a trap (`e-`).
5. **CEL: `cel` 0.15.0 has no cost, step or time limit, and none of the other Rust candidates found does either.**
   The plan's S5 criterion ("stopped within the limit") is reachable only with a change. A cooperative step counter
   is a 36-line patch (one line in `resolve_val`); it costs under 5% on ordinary views and stopped every runaway
   case within a bound set by the limit. A thread with a deadline does not stop anything: the thread keeps burning
   CPU. A child process can be killed (127 ms at a 100 ms limit). Do not expose the corpus (`tasks`) to view
   expressions, limit source length to about 1 KB (a 4 KB chain of `+` overflows a 2 MB stack and aborts the
   process), and cap list-field length when loading.

Contradictions with the plan are in "Proposed plan changes".

## Q1: Rust types to JSON Schema to TypeScript

**Slice** [M]: `q1-schema/src/lib.rs`, the pack-manifest shape: `PackManifest` (`rename_all = "camelCase"`,
`deny_unknown_fields`), `Workflow`, an internally tagged `Phase` (`tag = "kind"`; agent, gate, hook, review, wait and a
recursive fanout), `ProtectedPaths` (nested struct), an optional field, defaulted non-Option fields, a `BTreeMap`
(`config`), and a `u32`. 21 cases in `fixtures/q1-cases.json`: 8 valid by serde and 13 invalid. serde is the ground
truth for each case.

### Results

1. **The schema is exactly as strict as serde, and no stricter than needed** [M]. Invalid cases accepted by the
   schema: 0 of 13 (unknown top-level key, unknown key in a nested struct, unknown key in a tagged variant, a key
   of another variant, unknown or missing `kind`, snake_case key, wrong type, missing required, bad enum string,
   negative `u32`, bad nested phase, an extra key on an empty struct variant). The one disagreement is in the other
   direction: an extra key on a **unit** variant of an internally tagged enum is accepted by serde and rejected by
   the schema. That is a serde quirk (`deny_unknown_fields` is not enforced for a unit variant of an internally
   tagged enum). With `Wait {}` both reject. The schema is the stricter side, which is the safe one.
2. **Round trip** [M]: JSON to Rust to JSON on the 8 valid cases is idempotent in 8 of 8, the re-serialized value
   validates against the schema in 8 of 8, and it typechecks under all four type sets in 8 of 8.
3. **Generated TypeScript, checked as object literals** (`tsc` strict, 21 cases, both directions) [M]:

   | Generator | Valid case rejected | Invalid case accepted |
   | --- | --- | --- |
   | `json-schema-to-typescript` from the schema | 1 (the unit-variant quirk) | 1 (negative `u32`: TypeScript has no ranges) |
   | `specta` | 1 (same quirk) | 1 (same) |
   | `ts-rs`, as generated | 7 | 0 |
   | `ts-rs` with per-field overrides | 3 (`null` for `description` and `timeoutS`, and the quirk) | 1 |

   Excess-property checks catch unknown keys only for object literals; TypeScript cannot stop an extra key reaching
   a hook through a variable. The schema validates at the boundary, the type does not.
4. **`ts-rs` ignores `#[serde(default)]`** [M]. Four non-Option fields (`requires`, `config`, `maxRounds`,
   `readOnly`) come out required, so a `pack.yaml` that omits them fails the type. `#[ts(optional)]` is a compile
   error on a non-Option field; the workaround is `#[ts(as = "Option<T>")]` per field plus `optional_fields`. Even
   then `?: string` has no `| null`. It also prints `failed to parse serde attribute: deny_unknown_fields` during the
   build, twice. `u64` maps to `bigint`, so `ts(type = "number")` is needed.
5. **`specta` friction** [M]: it forbids BigInt, so `u64` needs `specta(type = Option<specta_typescript::Number>)` and
   a map of `serde_json::Value` needs `specta_typescript::Unknown`. The output is the most faithful to serde
   (optional `?` for defaults, `| null` for Options) but the pairing is an RC and the API moves.
6. **How tagged enums come out** [M]: schemars gives a `oneOf` whose branches carry
   `kind: {type: string, const: "agent"}`; `json-schema-to-typescript` turns it into a discriminated union
   (`kind: "agent"`); `specta` emits the same shape inline (`{ kind: "agent"; ... }`); `ts-rs` too, with a quoted
   key (`{ "kind": "agent", ... }`). All narrow on `kind`.
7. **Schema details the loader must handle** [M]: draft 2020-12 with `$defs`; `Option<T>` is `type: [T, "null"]`;
   integers carry `format: uint32` or `uint64` plus `minimum: 0`. `ajv` in strict mode rejects the unknown formats
   ("unknown format uint32 ignored"), so the validator registers those formats or runs with `strict: false`.
8. **Two contracts, not one** [M]: `schemars` can generate for deserialize or for serialize. They differ in
   `required[]`: deserializing, `Phase.agent` requires `kind, agent, prompt`; serializing it also requires
   `readOnly`; the top level differs likewise (`requires` and `config` become required on output). Types for what a
   user writes (manifest input) and for what Collide emits (hook inputs, normalized events) should come from
   different generations, or an author-facing type will demand fields the author may omit.
9. **Build friction** [M], dev profile, cold build in a fresh target directory with the crates already downloaded
   (the clean-copy run, the quieter of the two):

   | Setup | Cold build | Target dir | Edit, rebuild and emit |
   | --- | --- | --- | --- |
   | serde and serde_json only | 4.3 s | 61 MB | not applicable |
   | plus `schemars` | 4.4 s | 91 MB | 0.5 s |
   | plus `ts-rs` | 4.0 s | 100 MB | 0.7 s |
   | plus `specta` (three crates) | 4.8 s | 139 MB | 3.0 s |

   An earlier run on a machine at load average 13 to 19, including the first download of the crates, gave 11.3,
   27.2, 21.8 and 22.8 s cold and 9.2, 11.1 and 20.3 s for the edit loop. The ranking of the edit loop is the same in both
   runs: `specta` is 4 to 6 times slower than the other two; the cold builds are within noise of each other.

   Output sizes: `schema.json` 5,308 B, `jsts.ts` 1,334 B, `specta.ts` 1,323 B, `ts-rs` 1,930 B in five files.
   The schemars pipeline has one more step (a Node generator) and no per-type attributes: the schema derive is
   `#[derive(JsonSchema)]`, nothing else, because it reads serde attributes natively.

### Recommendation

`schemars` then `json-schema-to-typescript`. Pros: one artifact validates `pack.yaml`, `board.yaml` and views in
every language, the strictest unknown-key behaviour, no attribute duplication, a fast edit loop (0.5 s, against 3.0 s for `specta`). Cons: a
Node generation step in the build; numeric ranges and the unit-variant quirk are invisible to the types. `specta`:
the best typed output with no Node step and the natural fit for Tauri commands, but a release candidate, BigInt
refusals and three crates; consider it for the desktop IPC types only, where the schema is not the contract.
`ts-rs`: skip.

## Q2: Rust YAML crate

### Maintenance (crates.io API and GitHub, 2026-11-13) [M]

| Crate | State |
| --- | --- |
| `serde_yaml` 0.9.34 | archived on GitHub 2024-03-25; the old RUSTSEC-2018-0005 is patched; works but frozen |
| `serde_yaml_ng` 0.10.0 | not archived, last push 2025-09-14, last publish 2024-05-26, 15.8M recent downloads; libyaml port (`unsafe-libyaml`) |
| `serde_norway` 0.9.42 | not archived, last push 2025-08-04, last publish 2024-12-21; libyaml port (`unsafe-libyaml-norway`) |
| `serde_yml` | archived; now a deprecated shim at 0.0.13; RUSTSEC-2025-0068 (unsound and unmaintained). **Excluded** |
| `unsafe-libyaml` | RUSTSEC-2024-0449 (unmaintained), RUSTSEC-2023-0075 (unsound on 32-bit, patched). Under both `_ng` and `norway` |
| `serde-saphyr` 2.1.1 | pushed 2026-11-12; pure Rust, `unsafe` forbidden, fuzzed, a `Budget` API |
| `saphyr` 0.3.4, `yaml-rust2` 0.13.0 | active (pushed 2026-11-11 and 2026-09-11); raw trees only, no serde layer, not tested |

### Results

Seven configurations run on the same documents: serde_yaml 0.9, serde_yaml_ng, serde_norway, and serde-saphyr as
default, with `strict_booleans`, with `strict_booleans` plus `no_schema` ("lint"), and hardened.

1. **`no`, `yes`, `on`, `off`, `y`, `n` into a String field** [M]: strings in all seven. Into a **bool** field the
   serde_yaml family errors on `no`. **`serde-saphyr` default accepts `no` as `false`** (and `y`, `n`), both into
   bool fields and untyped values. `strict_booleans: true` fixes it. This is the one place the plan's "YAML 1.2
   core" claim fails.
2. **`version: 0.10` into String** [M]: "0.10" in all seven, and `0x1f`, `0o17`, `1e10` arrive as their text.
   Typed String fields are the protection. Into an untyped `serde_json::Value` (the `config` map) it becomes the
   number 0.1. With `no_schema: true`, `serde-saphyr` refuses an unquoted ambiguous scalar in a String field:
   "The string value [0.10] must be quoted". That is a built-in quote-required lint.
3. **Null into String** [M]: `null`, `~`, an empty value, and an anchor or tag with no value arrive in the serde_yaml
   family as "null", "~" or "" with no error. `serde-saphyr` errors. So `name:` (empty) is a valid empty name unless
   the schema has `minLength: 1`. `Option<String>` gives `None` everywhere.
4. **Unquoted `{{ task.id }}`** [M]: an error at 1:4 in all seven ("invalid type: map, expected a string"; the
   saphyr text says "expected string scalar"). Quoted forms work. `x {{ }} y` and `${{ secrets.X }}` are plain
   scalars and work. `{% ... %}` is an error. `@x`, a backtick, `%x` and `*x` at the start of a plain scalar are
   errors. The error is correct but its text does not say "quote the template"; the loader can add that hint when
   the failing value starts at a `{{`.
5. **Duplicate keys** [M]: into derive structs every crate rejects, with the serde_yaml family pointing at the map
   start (1:1) and saphyr at the duplicate line and column (for example 2:1, and 1:8 inside a flow mapping). **Into a
   map or `Value`, `serde_yaml`, `_ng` and `_norway` accept and keep the last** (`{"lanePool": "y"}`), which is how a
   `workflows:` or `config:` map would silently drop a definition. `serde-saphyr` rejects in all positions by
   default.
6. **Merge keys `<<`** [M]: saphyr merges by default; the serde_yaml family fails in typed targets. The hardened
   set uses `MergeKeyPolicy::Error`, so there is one behaviour.
7. **Error positions** [M]: every crate gives line and column. `deny_unknown_fields` is located (`unknown field
   flavor, expected one of name, version, workflows` at 3:1), as are type mismatches (2:10) and missing fields
   (1:1, the map start). For an unterminated quote the serde_yaml family reports end of input (4:1) with the opening
   position only in the text; saphyr reports 3:1 "invalid indentation in multiline quoted scalar". Tab indentation:
   4:1 in serde_yaml, 4:2 with a clearer message in saphyr.
8. **Alias bomb** (9 aliases per level, so level N is 9^(N+1) leaves; limits 3000 MB and 25 s) [M]:
   - serde_yaml family into `Value`: level 2 (729 leaves) parses; level 4 (59,049) fails with "repetition limit
     exceeded" after about 6 ms and 6 MB. Into `IgnoredAny`, levels 5 to 7 (up to 43 million leaves) take 0 ms
     and 2 MB: they never expand.
   - `serde-saphyr` default: level 4 parses (74,733 nodes, 22 ms, 18 MB); level 5 fails "budget breached: Nodes
     250001" after 77 ms and 51 MB.
   - hardened (`max_anchors: 0`): any anchor is rejected at once (0 ms).
9. **Deep nesting** (`k: [[[...]]]` into `IgnoredAny`) [M]: the serde_yaml family accepts depth 100,000 in 19 to
   21 s (22 MB). Depth 1,000 takes 3 to 4 ms; the cost is quadratic, so a 200 KB file of brackets is a 20-second CPU
   burn. saphyr stops in under 1 ms ("recursion limit exceeded", or "budget breached: Depth" at 65; hardened at 33).
10. **Throughput**, 0.49 MB manifest-shaped document (3,000 workflows), median of 5, release, into `Value` [M]:
    serde_yaml 19.5 ms, `_ng` 19.0 ms, `_norway` 16.4 ms, saphyr default, `strict_booleans`, lint and hardened
    26.8 to 27.4 ms (an earlier run on the loaded machine gave 28 to 30 ms and 41 to 74 ms; the ratio held at 1.4 to
    1.8x). Manifests are kilobytes, so both are far below any budget.

### Recommendation

`serde-saphyr` 2.x with this option set (`saphyr_options` in `q2-yaml/src/main.rs`): `strict_booleans: true`,
`no_schema: true`, merge keys as errors, budget `max_aliases: 0, max_anchors: 0, max_nodes: 100_000, max_depth: 32`,
and the default `DuplicateKeyPolicy::Error`. Pros: maintained, no `unsafe`, every hazard that matters is a loader
option and not a convention, exact duplicate-key positions, bounded resource use. Cons: its defaults are YAML 1.1
for booleans, so the options are mandatory and need a test that pins them; it is about 1.4x slower than the libyaml
ports (irrelevant at this size); one maintainer, smaller user base than `serde_yaml_ng`. Runner-up `serde_yaml_ng`
(maintained but not updated since 2024-05-26, libyaml port on an unmaintained `unsafe-libyaml`): acceptable only with
a file-size cap, a depth pre-check, and a duplicate-key check on every map field.

**What the loader or schema must do, per hazard:**

| Hazard | With serde-saphyr (hardened) | Still the loader's job |
| --- | --- | --- |
| `no`, `yes`, `on`, `off` | strings, bool fields reject them | none |
| `0.10`, `1e10`, `0x1f` in string fields | rejected unless quoted (lint) | `config` and other `Value` maps: schema type check, or document that numbers arrive as numbers |
| unquoted `{{ }}` | error at the right column | add the hint "quote the template" |
| duplicate keys | rejected everywhere | none |
| empty or `null` value into a String | error | `minLength: 1` on names and ids |
| anchors, aliases, `<<` | rejected | tell authors in the error text |
| bombs and deep nesting | stopped in under 2 ms | cap the file size before parsing (for example 1 MiB) |
| error positions | line and column | none |

## Q3: YAML in TypeScript (provider frontmatter)

### Results

1. **Booleans** [M]: `yaml` 2.9.1 default (core), `js-yaml` 5.5.0 default and its failsafe schema keep
   `no`, `yes`, `on`, `off`, `y`, `n`, `NO` as strings. `yaml` with `version: '1.1'` and Psych (the 1.1 reference)
   give `no` as `false`; `y` and `n` become booleans in the 1.1 mode of `yaml` only. `gray-matter` (js-yaml 3)
   keeps them strings.
2. **Dates and numbers differ by parser** [M]: `2026-11-13` is a `Date` in `yaml` 1.1, `gray-matter` and Psych, a
   string in `yaml` core and `js-yaml` 5. `2026-1-3`, `12:30`, `1:30:00` (sexagesimal, 5400), `0b101` and `1_000`
   are numbers or dates in the 1.1 parsers only. `017` is 17 in core and `js-yaml` 5, 15 in 1.1, Psych and
   `gray-matter`. `0o17` is 15 in core and `js-yaml` 5, a string in 1.1. `1e10` and `12e3` are numbers in the JS
   parsers and strings in Psych. `0x1f`, `123`, `.5`, `+1` are numbers everywhere. `12345678901234567890` loses
   precision. `0.10` is the number 0.1 everywhere except the failsafe schema.
3. **A bug in `yaml` 2.9.1's 1.1 mode** [M]: `E-07`, `e-07`, `e-1`, `E-0`, `e+5` parse as the number NaN
   (`JSON.stringify` writes `null`). It hit 1,070 of 1,599 real Orchard tasks through `epic: E-07`. `e-4k7q`, `ee-07`,
   `sp-4k7q` and `E-` are strings. So a one-letter prefix followed by a hyphen and digits is hazardous in that mode.
4. **Id alphabets, exhaustively** (every id of the shape written as an unquoted list item, five parsers) [M]:

   | Shape | Ids | Not a string: `yaml` core / `yaml` 1.1 / `js-yaml` 5 / `gray-matter` / Psych |
   | --- | --- | --- |
   | bare base36, 4 chars | 1,679,616 | 12,322 / 13,262 / 12,322 / 11,674 / 9,774 (about 0.73%) |
   | `sp-` plus base36, 4 chars | 1,679,616 | 0 / 0 / 0 / 0 / 0 |
   | `e-` plus base36, 3 chars | 46,656 | 0 / 1,000 / 0 / 0 / 0 |
   | `t-`, `dc-`, `x-` plus base36, 3 chars | 46,656 each | 0 in every parser |
   | bare, letter first, 4 chars | 1,213,056 | 2 (`null`, `true`) / 1,002 / 2 / 2 / 2 |

   The failures of bare ids are digit-only ids (`1234`), exponent forms (`1e10`, `12e3`), hex (`0x1f`), `null` and
   `true`. **Rule: `^[a-z]{2,}-[0-9a-z]{4,}$`.** A hyphen inside the scalar rules out every number, boolean and null
   form in every parser, so no blocklist is needed; two letters avoid the `e-` NaN bug. The bare 4-character form
   `4k7q` stays a display alias (planning.md section 4.2 `alias`), never the stored value. A git short sha written
   unquoted (`preregistered: 3f1c2a9`) has the same hazard (`3e10`, `1234567`), so fields that hold hashes must be
   quoted by Collide or declared as strings and rejected otherwise.
5. **Readback of what a TS provider writes** [M]: `yaml.stringify` with defaults quotes safely for itself but 7 of 23
   risky strings are misread by another parser: `no`, `yes`, `on`, `off` by Psych; `1_000`, `2026-11-13`, `1:30` by
   `gray-matter` and Psych. With `version: '1.1'` only `0o17` is misread. If frontmatter has to survive a 1.1 reader
   (Ruby, an old tool), the provider should quote every string scalar itself.
6. **Duplicate keys** [M]: `yaml` (both modes), `js-yaml` 5 and `gray-matter` reject; Psych accepts, last wins.
7. **Error positions** [M]: `yaml` gives `linePos`, `js-yaml` gives `mark.line` and `column`. For an unterminated
   quote on line 3, `yaml` reports 5:1 and `js-yaml` 4:1: both point at the end of input.
8. **Alias bomb** [M]: `yaml` core rejects it ("Excessive alias count indicates a resource exhaustion attack",
   `maxAliasCount` 100). `js-yaml` 5 has no alias limit: parsing takes 0.3 to 5 ms but `JSON.stringify` of a
   level-7 result is 205 MB in 1.6 s, and level 9 is 3.5 billion leaves. `yaml` with `maxAliasCount: -1` behaves like
   `js-yaml`.
9. **Depth** [M]: `yaml` throws a catchable `RangeError` (17 ms at depth 1,000, 597 ms at depth 100,000); `js-yaml` 5
   has `maxDepth` 100.
10. **`__proto__` key** [M]: becomes an own key in `yaml`, `js-yaml` and `gray-matter`; no prototype pollution.
11. **`gray-matter` executes code** [M]: a frontmatter fence of `---js` runs the JavaScript by default (the test code
    ran). It is disabled only by overriding `engines.js`. Task files are untrusted data (plan section 0.9), so this
    alone rules out `gray-matter` defaults. Its content cache also hides its parse time unless an options object is
    passed (the script does).
12. **Real corpus** [M] (1,599 Orchard task files, read-only): every file has frontmatter; keys `title`, `status`,
    `epic`, `backlog_id`, `reviewed_by`, `awaiting` are strings; `depends_on` and `blocked_by` are arrays;
    `self_review` and `migrated` are booleans; status values are `done` 829, `specified` 593, `dropped` 141,
    `in_review` 18, `ready` 13,
    `doing` 5. No file stem parses as a non-string. Parse time, median of 5, 1,599 files: `yaml` core 67 ms
    (42 us per file), `yaml` 1.1 63 ms, `js-yaml` 5 13 ms (8 us per file), `gray-matter` 11 ms (cache bypassed),
    Psych one run 163 ms (an earlier run on the loaded machine: 201, 120, 19, 3 (cached) and 561 ms). Parses differing from `yaml`
    core: `yaml` 1.1 on 1,070 files (the NaN bug), `js-yaml` 5, `gray-matter` and Psych on 0. At 42 us per file a
    full re-read of the corpus is about 70 ms, so the markdown provider needs no cache for parsing cost.

### Recommendation

`yaml` 2.9.1, default (core) mode, a size cap before parsing (for example 64 KiB per file), default
`uniqueKeys`, default `maxAliasCount`, and a schema that checks every field's JavaScript type (no coercion, no `Date`).
Pros: the safest defaults of the four (alias limit, no code execution, strings stay strings), `linePos` for errors.
Cons: slower than `js-yaml` (42 against 8 us per file, irrelevant here); the 1.1 mode is a trap. `js-yaml` 5 is
acceptable only if the caller adds its own alias and output-size limits; `gray-matter` is out. Ids follow the rule
in result 4.

## Q4: CEL crate for views

### Candidates [M, source grep and crates.io]

| Crate | State | Cost or step limit |
| --- | --- | --- |
| `cel` 0.15.0 | updated 2026-10-06, repo pushed 2026-11-13, 718 stars, 5.2M recent downloads | none; only parser recursion limits and `MAX_RANGE_SIZE` 1,000,000 for `lists.range` |
| `cel-interpreter` 0.10.0 | the old name of the same project | none |
| `cel-core` 0.5.1 | 5 stars, last push 2026-02 | none found |
| `cellang` 0.7.0 | 190 recent downloads | none found |
| `rscel` 1.0.8 | bytecode VM | none found |
| `cel-cxx` | bindings to cel-cpp | [I] cel-cpp is documented to track cost; not tried (adds a C++ build) |
| `ferricel` | CEL to Wasm | [U] a Wasm runtime such as wasmtime can meter fuel; not tried |

A grep of the sources of `cel`, `cel-core`, `cellang` and `rscel` for cost, step, iteration and limit found none.

### Corpus

`q4-cel/src/corpus.rs`: 1,567 synthetic tasks (the plan's count; the real tree has 1,599) with the real status mix
and the real `depends_on` length histogram (0: 1,139, 1: 290, 2: 78, 3: 29, 4: 18, 5: 14, 6: 9, 7: 10, 8 to 24:
rest), mean title length 83, and 11 variables bound per task (`id`, `key`, `source`, `title`, `status`, `category`,
`kind`, `labels`, `collections`, `depends_on`, `author`). Release builds throughout.

### Results

1. **Ordinary view expressions** [M], per task over 1,567 tasks, bind plus evaluate (median):

   | Expression | Total ms | Steps per task (patched build) |
   | --- | --- | --- |
   | binding the 11 variables only | 1.9 | not applicable |
   | E1 `status == 'ready'` | 2.0 | 3.0 |
   | E2 `'plan:E-07' in collections \|\| 'mvp' in labels` | 2.1 | 6.2 |
   | E3 `size(depends_on) == 0 && labels.exists(l, l.startsWith('p'))` | 3.3 | 15.9 |
   | E4 `title.contains('edge') && author == 'me'` | 2.2 | 5.3 |
   | **E5 `id.matches('^T-0[0-9]-1.*') && category != 'dropped'`** | **40.5** | 4.8 |
   | E6 `depends_on.all(d, d.startsWith('T-')) && !(kind in ['bug','spike'])` | 2.9 | 15.1 |

   Ordinary views cost 1 to 2 us per task; a whole-corpus pass is 2 to 3 ms. **`matches` recompiles the regex on
   every call** (about 26 us per task): 20 times the others. Not a danger, but a view author should see it. The first
   `compile` costs 13.7 ms (one-time initialisation), later ones 0.1 to 0.7 ms. The heaviest ordinary view uses
   about 16 steps per task, about 25,000 steps for the whole corpus.
2. **Nested macros over the corpus** (`limits`, each in a child process, killed at 4,000 MB or 30 s) [M]:

   | Expression | Stock result | Peak RSS |
   | --- | --- | --- |
   | Q1 `tasks.filter(t, tasks.exists(u, t.id in u.depends_on))` (the example from the S5 brief) | 3.4 s (3.4 s patched, 16,031,772 steps) | 18 MB |
   | Q2 `tasks.filter(t, tasks.exists(u, tasks.exists(v, ...)))` | killed at 30 s | 18 MB |
   | Q3 `tasks.map(t, tasks.map(u, t.id + u.id))` (n squared strings) | 7.2 s (12,285,283 steps) | 390 MB |
   | Q4 nested map of maps (n cubed) | killed at 30 s | 730 to 970 MB at 30 s, still growing |

   Growth with corpus size (stock, in one process): Q1 x3.6 to x4.0 for each doubling of n (quadratic: 15.5, 55.9,
   221.5, 852.0 ms at n = 100, 200, 400, 800); Q2 x8.1 to x8.2 (cubic: 203.4, 1,648.2, 13,591.0 ms at n = 50, 100,
   200; n = 1,567 extrapolates to about 2 hours); Q3 x3.2 to x5.2 (quadratic, noisier because of allocation).
   **The brief's example expression takes 3.4 s at the real corpus size, and a triple nesting does not finish.**
3. **A hostile single task** [M]: no `tasks` variable at all, one task whose `labels` list has M entries, and the
   innocent `labels.all(a, labels.exists(b, a == b))`: M = 1,000 takes 160 ms, M = 5,000 takes 4.2 s (100,075,001
   steps), M = 20,000 takes 66.9 s in-process (unbounded: 12 MB). So hiding the corpus is not enough; list length in
   the data matters (plan section 0.9: task data is untrusted).
4. **Is there a built-in limit?** [M] No (see candidates). Nothing in `cel` 0.15 stops evaluation, and a macro cannot
   be interrupted from outside.
5. **Cooperative step counter** (patch `q4-cel/patches/cel-step-limit.patch`: a new 34-line `src/steps.rs` holding a
   thread-local counter and limit, `pub mod steps;` in `lib.rs`, and `crate::steps::tick()?;` at the top of
   `Value::resolve_val`, which every expression node passes through) [M]:
   - **Overhead:** unmeasurable: ordinary views 2.00 to 3.12 ms patched against 1.98 to 3.26 ms stock; E5 40.9 against
     40.5 ms.
   - **It stops every runaway case.** With a limit of 10,000,000 steps: Q1 stopped at 2.1 s (it needs 16.0 M steps to finish, so
     a limit of 10 M is below it), Q2 at 1.7 s, Q3 at 3.5 s (144 MB), Q4 at 4.2 s (397 MB), hostile
     M = 5,000 and 20,000 at 0.43 s and 0.45 s. At 50,000,000 steps: Q2 at 8.5 s, Q4 at 23.5 s with 1,725 MB (the
     memory is not bounded by steps alone).
   - **Steps are not seconds.** Cost per step in these runs: hostile list scan about 45 ns, Q2 170 ns, Q1 210 ns, Q3
     370 ns (string allocation). So a step limit bounds time within a factor of about 8 for these shapes, and memory
     only loosely. The error text is `Error executing function 'cost': step limit exceeded` and the process survives.
   - Counted per node, not per macro iteration: the counter sees `tasks.filter` iterate because every body node
     passes through `resolve_val`.
6. **Thread with a deadline** [M]: a 300 ms deadline fired at 306 to 310 ms, the caller moved on, and the abandoned
   thread **kept running**: process CPU rose 1.1 s, 2.1 s, 3.1 s at 1 s, 2 s, 3 s later (about 1 core), Q3's RSS grew
   from 28 MB to 66 MB, and it ended only when the expression finished (4.4 s here, for Q2 or the hostile case: minutes
   to hours). Rust cannot cancel a thread. A thread deadline gives the caller its answer but not its CPU or memory
   back.
7. **Child process with a deadline** [M]: Q2 in a child with a 100 ms wall limit was killed at 114 to 127 ms (peak 18 MB);
   with 300 ms at 314 to 321 ms. Isolation costs 13 to 17 ms wall end to end for a child that builds 10 tasks and runs Q1. It
   works, but the corpus must be sent to each child, and it is heavy for per-keystroke filter-bar previews.
8. **Static cost estimate** [M] (an AST walk over `Program::expression()`; multiplies the iteration count of each
   comprehension by the size of the list it ranges over, using `tasks` = 200 and other lists at most 64; 0.1 to
   2.6 us to compute). Node evaluations estimated against steps measured:

   | Expression | Estimate | Measured |
   | --- | --- | --- |
   | E1, E4, E5 (no macros) | 3, 7, 7 | 3.0, 5.2, 7.0 per task |
   | E2, E3, E6 | 135, 521, 461 | 6.2, 15.8, 14.2 per task (assumes 64-entry lists) |
   | Q1 | 2,962,204 | 277,040 at n = 200 |
   | Q2 | 144,362,204 | 80,481,203 |
   | Q3 | 361,604 | 201,003 |
   | Q4 | 88,321,604 | not run at this size |

   It never underestimated, and overestimated by 1.1x (no macros) to 11x (when `&&` or `in` short-circuits or a list
   is shorter than the assumed bound). It is the right check at the moment a view is saved or loaded ("this view is
   cubic in the corpus: refused"), and cheap enough to run on every keystroke. It does not replace the runtime limit:
   it bounds only expressions whose list sizes it knows.
9. **Source size and stack** [M] (`parse`, each in a child process, 8 MB main stack unless stated):
   - a chain of `1+1+1...` compiles and runs at 6,000 terms (12 KB); at 7,000 terms the process **aborts with a stack
     overflow** (not catchable). With a 2 MB stack (`ulimit -s 2048`): 1,500 terms (3 KB) fine, 2,000 terms (4 KB)
     abort;
   - parentheses and list literals nest only to a small parser recursion limit (60 deep works, 100 is a syntax
     error), but at 100,000 levels the parser **panics inside its error formatter** ("Formatting argument out of
     range", `parser.rs`), so a hostile expression can panic the caller unless `catch_unwind` is used or the source
     length is capped;
   - `!` chains of 100,000 (100 KB) compile in 24 ms; `&&` chains of 100,000 terms (800 KB) compile in 495 ms with
     330 MB peak.
   A cap of 1 KB on view expression source avoids all of these with a factor of 3 on a 2 MB stack.

### Recommendation

Use `cel` 0.15.0 and add a cost bound ourselves. The options, in the order I would take them:

1. **Fork or patch `cel` with the step counter (recommended).** Pros: 36 lines, under 5% overhead, stops everything
   measured, error is an ordinary `ExecutionError`, the evaluation thread survives. Cons: a fork to carry or an
   upstream PR to land; steps are a proxy for time (about 8x spread) and memory.
2. **Own evaluator over the public AST.** `Program::expression()` is public and the `Expr` enum is small
   (Call, Comprehension, Ident, List, Literal, Map, Select, Struct), so Collide could interpret it with its own
   accounting, including a memory estimate. Pros: no fork; exact accounting of allocations. Cons: reimplementing the
   standard functions (`size`, `startsWith`, `matches`, `in`, macros); more code to own. [U] Not built.
3. **Child process per evaluation with a kill deadline.** Pros: stops anything, including memory. Cons: 13 to 17 ms spawn,
   the corpus has to cross a process boundary, not suitable for interactive previews.
4. **Never use a thread deadline alone.** The thread keeps running (result 6).
5. A static estimate (result 8) in addition to either of the first two, never instead.

And regardless of the option, the data and expression limits: do not bind `tasks` or any corpus-wide variable (plan
section 0.10 binds per-task normalized fields, so keep it that way); cap `labels`, `collections` and `depends_on` at load (1,000 entries is
160 ms for the hostile nest, 5,000 is 4.2 s); cap string fields at about 4 KiB; cap view
expression source at 1 KB; and run compile under `catch_unwind`.

**Suggested limits** [I] (derived from the numbers above, not tested as a set): 10,000 steps per task evaluation
(the heaviest ordinary view uses about 16, so 600x headroom) and 2,000,000 steps per view pass (80x headroom over
25,000, about 0.3 to 0.8 s worst case at 170 to 370 ns per step). An evaluation error excludes the task and is counted
(plan section 0.10), so a runaway view degrades to "tasks excluded, N errors" rather than a hang.

## Recommendations

| # | Recommendation | Core or pack | Plan section it would change |
| --- | --- | --- | --- |
| 1 | `schemars` to JSON Schema to `json-schema-to-typescript`; `Wait {}` for variants with no fields; register or relax `uint*` formats in the validator; separate input and output type generations | core | 0.7 ("Types:" bullet), Stage 2 (schemas and generated types) |
| 2 | `serde-saphyr` with the hardened option set, pinned by a test; reject `no_schema` violations and anchors; file-size cap | core | 0.7 (the `serde_yaml` bullet), 0.9 (add YAML to the untrusted list) |
| 3 | Loader hints: "quote the template" for unquoted `{{ }}`; `minLength: 1` on names; schema-check `config` maps | core | 0.7 |
| 4 | Frontmatter parsing with `yaml` in core mode, size cap, no `gray-matter`, no 1.1 mode | pack (the markdown provider) and any core TS | 0.6, planning.md section 4.2 |
| 5 | Id rule `^[a-z]{2,}-[0-9a-z]{4,}$`, bare short id as display alias only; quote hashes | core (id allocation) and pack | planning.md sections 2.4 and 4.2, 0.8 |
| 6 | `cel` 0.15.0 plus a step counter (fork or upstream PR), per-task and per-pass limits, 1 KB source cap, list caps at load, `catch_unwind` around compile, no corpus variable | core | 0.9, 0.10, Stage 1 S5 bullet, 0.15 (dependencies) |
| 7 | Static cost estimate at view save and load | core | 0.10 |

## Proposed plan changes (proposals only; the plan was not edited)

1. **0.7, first bullet.** Replace "YAML 1.2 core schema with typed fields; that is what keeps `no` a string" with: typed
   String fields keep `no`, `0.10` and `1e10` strings in every crate tested, but the chosen crate's defaults are YAML
   1.1 for booleans, so `strict_booleans` is part of the loader and pinned by a test. Add: ambiguous unquoted scalars
   in string fields are an error (the `no_schema` lint), duplicate keys are an error everywhere, anchors and merge
   keys are rejected, files are size-capped before parsing.
2. **0.7, `serde_yaml` bullet.** Name `serde-saphyr` 2.x and drop "S5 picks".
3. **0.7, Types bullet and Stage 2.** Add "`Wait {}`, not a unit variant, in internally tagged enums"; add the
   input-versus-output contract split; add "`json-schema-to-typescript` is a Node build step".
4. **0.9, the CEL bullet.** Replace "with a cost limit and an expression size limit (S5)" with: `cel` has no limit; Collide
   adds a step counter, a per-task and a per-pass budget, a 1 KB source cap and list-length caps; a thread deadline
   is not a limit. Note that a 4 KB expression can overflow a 2 MB stack and abort the process, so the source cap is
   a safety property, not a style one.
5. **0.10.** State that views bind per-task fields only and never the corpus, and that the static estimate runs when
   a view is saved or loaded.
6. **Stage 1, S5 bullet.** Mark done with: "`cel` has no limit; the step-counter patch stops the plan's nested
   example within the limit".
7. **planning.md section 4.2.** Require ids to match `^[a-z]{2,}-[0-9a-z]{4,}$` (`sp-4k7q` passes); name the `e-` hazard;
   say the bare form is an alias.
8. **0.15.** Add `schemars`, `serde-saphyr` and `cel` (patched) to the dependency list.

## Left unmeasured

- **[U]** `specta` through a real Tauri command, and the JSON Schema validator in Rust (for example the `jsonschema`
  crate): the plan says the same schema validates `pack.yaml`, `board.yaml` and views, but the Rust side checked here
  is serde's `deny_unknown_fields`, not a schema validator.
- **[U]** `saphyr` and `yaml-rust2` raw trees through a typed layer, and `serde-saphyr`'s error text for every
  position (only the cases listed were run). A real fuzz run on any crate.
- **[U]** `cel-cxx` (cel-cpp's cost estimator) and `ferricel` (Wasm fuel): no build tried. `cellang`, `rscel` and
  `cel-core` were only searched for limits, not run.
- **[U]** Regex cost: `matches` recompiles per call; a huge or pathological pattern was not run.
- **[U]** The step counter in a multi-threaded evaluator: the counter is thread-local, so a view pass spread over
  rayon threads needs a per-thread or shared atomic budget.
- **[U]** Memory bound: steps do not bound memory (Q4 reached 1,725 MB at 50 M steps). A string-size and list-size
  cap on values built during evaluation was not tried.
- **[U]** The real Orchard corpus through CEL; only its shape (status mix, dependency histogram, key types) was read.
- **[U]** `yaml` 1.1-mode NaN bug upstream status; the 1.1 mode is not recommended so it was not chased.
- **[U]** Windows and Linux: all timings are macOS arm64, on a loaded machine.
- The suggested step limits are an inference, not a tested set.

## Reproduction

```
cd .collide/planning/spikes/sp-0005-fv-schema-expressions
scripts/run-all.sh                  # about 20-25 minutes; prints every number and saves it under work/results/
S5_FULL=1 scripts/run-all.sh        # adds the 4-character exhaustive id sweeps (about 4 more minutes)
S5_SKIP_NET=1 scripts/run-all.sh    # skips the crates.io metadata fetch
```

`README.md` in that directory lists each script, what it prints and what it needs. Cargo output is in
`work/target`, the pnpm store in `.pnpm-store`, and nothing is installed globally. The Q3 4-character sweeps
(1.68M ids per shape) take about 40 s for `yaml` core and 90 s for `yaml` 1.1 per shape; the default run uses the
3-character variants of the same shapes, and the 4-character numbers in this record come from `S5_FULL=1`.
