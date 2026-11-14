# S5 schema and expressions: reproduction scripts

Throwaway spike code. The decision record is `record.md`.

One command re-runs everything from a clean checkout and prints the numbers (saved under `work/results/`):

```
scripts/run-all.sh                  # about 20-25 minutes
S5_FULL=1 scripts/run-all.sh        # adds the 4-character exhaustive id sweeps in Q3 (about 4 more minutes)
S5_SKIP_NET=1 scripts/run-all.sh    # skips the crates.io metadata fetch
```

Needs macOS, Rust via `~/.cargo/bin/cargo` (not on PATH; override with `CARGO=`), Node at `/opt/homebrew/bin/node`
(override with `NODE=`), `pnpm`, and `/usr/bin/ruby` (Psych 3.1, the YAML 1.1 oracle). Network is used for crates,
npm packages and the crates.io API. Cargo output goes to `work/target` (`.cargo/config.toml`) and the pnpm store to
`.pnpm-store` (`.npmrc`). Nothing is installed globally. No model calls. The Orchard task corpus is read
read-only: set `ORCHARD_ARCH` to its root (Q3 stops with a message if it is unset).
Timings are wall-clock on a possibly busy machine: run on an idle one for clean numbers.

| Script | Measures | Prints | Needs |
|---|---|---|---|
| `scripts/crate-status.mjs <crate>...` | Maintenance signals for each candidate crate | Latest version, publish date, yanked flag, downloads, repository | network |
| `scripts/q1.sh` | Q1: cold build size and time per generator, the edit-rebuild loop, then the comparison | Build seconds and target-dir MB; ts-rs warnings; the comparison below | cargo, pnpm install |
| `scripts/q1-compare.mjs` | Q1: serde (ground truth) against the schemars JSON Schema (ajv) and three generated TS sets (json-schema-to-typescript, ts-rs, ts-rs with `as` workarounds, specta), on 21 cases in `fixtures/q1-cases.json` | Per-case verdicts, mismatch lists, round-trip counts, the generated `Phase`, `required[]` for deserialize vs serialize contracts, sizes | `work/q1-out/` from `q1.sh` |
| `work/target/release/s5-yaml all` (subcommands `scalars templates duplicates errors merge bombs perf`) | Q2: seven parser configurations (serde_yaml 0.9, serde_yaml_ng, serde_norway, and serde-saphyr default / strict booleans / lint / hardened) on the YAML hazards | Per-hazard parse results with line:column, alias-bomb and deep-nesting runs under an RSS and wall limit (child process, `wait4` rusage), parse throughput | `cargo build --release -p s5-yaml` |
| `scripts/q3-yaml-ts.mjs [scalars\|fields\|ids\|stringify\|hazards\|corpus]` | Q3: `yaml` (core and 1.1), js-yaml 5, gray-matter and Psych on scalars, frontmatter fields, exhaustive id alphabets, written-frontmatter readback, bombs, depth, the `---js` engine, and the real corpus | Counts of ids that are not strings per parser, differences, parse times | `S5_FULL=1` for 4-character sweeps; `/usr/bin/ruby` |
| `scripts/q4-vendor.sh` | Copies cel 0.15.0 from the cargo registry to `work/vendor/cel-patched` and applies `q4-cel/patches/cel-step-limit.patch` | Path of the patched copy | `cargo fetch` |
| `work/target/release/cel-bench ordinary` | Q4: six ordinary view expressions over 1567 tasks | Compile time, bind+eval time, matches, steps per task | `cargo build --release -p s5-cel` |
| `cel-bench estimate [n]` | Q4: static AST cost estimate against measured steps | Node count, estimate, steps, estimate time | same |
| `cel-bench limits` | Q4: four nested expressions and a hostile single-task list, each in a child process killed at 4000 MB RSS or 30 s | Wall time, peak RSS, result, steps | same |
| `cel-bench scaling` | Q4: growth exponent of nested expressions with corpus size | Time per size and the factor for doubling n | same |
| `cel-bench hostile` | Q4: one task with a big `labels` list and an innocent-looking nested expression | Time per list size | same |
| `cel-bench threads` | Q4: evaluation on a thread with a deadline, and what the thread does after the caller gives up | CPU time and RSS after the deadline | same |
| `cel-bench parse` | Q4: deep and large expressions: parse and evaluate | Time and peak RSS per shape | same |
| `cel-bench-patched <same subcommands>` | Q4: the same with the cooperative step counter, which shows counter overhead and stops runaway evaluations | The stock columns plus steps; `limits` adds runs at 10M and 50M steps | `cd q4-cel-patched && cargo build --release` after `q4-vendor.sh` |

Supporting code:

- `Cargo.toml` is the workspace of `q1-schema`, `q2-yaml` and `q4-cel`. `q4-cel-patched` is a separate workspace
  because it depends on the patched copy of cel; it includes `q4-cel/src/bench.rs` and `corpus.rs` by path.
- `q1-schema/src/lib.rs` is the pack-manifest slice: an internally tagged enum `Phase`, optional and defaulted
  fields, `rename_all`, `deny_unknown_fields`, a map, a nested struct and a recursive variant. The generator
  derives are behind the features `schemars`, `ts`, `ts-fix` and `specta`. `bin/emit.rs` writes the artifacts and
  `bin/verdict.rs` prints serde's verdict and re-serialization per case.
- `q2-yaml/src/main.rs` holds the seven parser backends behind one `parse` function. `saphyr_options` has the
  hardened serde-saphyr configuration.
- `q4-cel/src/corpus.rs` builds the synthetic 1567-task corpus from the measured Orchard status and
  dependency-count distributions. `q4-cel/src/bench.rs` has the expression sets, the static estimator and the
  child-process runner.
- `q4-cel/patches/cel-step-limit.patch` adds a thread-local step counter (`src/steps.rs`) and one `tick()` call at
  the top of `Value::resolve_val`, the evaluator's single entry point.
- `scripts/psych.rb` is the Ruby Psych oracle. `scripts/timeit.mjs` times a command. `scripts/crate-search.mjs`
  searches crates.io.
- `work/` is disposable: build output, vendored sources, generated files and saved results.
