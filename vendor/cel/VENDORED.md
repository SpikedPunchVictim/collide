# Vendored `cel`

| | |
|---|---|
| Crate | `cel` |
| Version | 0.15.0 |
| Source | the crates.io package `cel-0.15.0` (upstream repository `https://github.com/cel-rust/cel-rust`, path `cel`, commit `57b2a7e9b835425be0a7320e6bc0461380843f2d`, from the package's `.cargo_vcs_info.json`) |
| License | MIT, text in `LICENSE` (copied from the upstream repository at the commit above) |
| Patch | `cel-step-limit.patch`, one patch: a step counter |
| Used by | `core` (`cel = { path = "../vendor/cel" }`), never the registry crate |

## What is here

The package's `src/`, `Cargo.toml`, `README.md` and `CHANGELOG.md`, with the patch applied. Not vendored: `tests/`,
`benches/`, `Cargo.lock` and the package's generated files (`.cargo_vcs_info.json`, `Cargo.toml.orig`). The manifest
lost its `[[test]]` and `[[bench]]` targets and its `[dev-dependencies]` for that reason, and gained a comment at the
top; nothing else in it changed. The published package ships no LICENSE file, so `LICENSE` is the upstream repository's file at the commit
above, which the MIT terms require to travel with the code.

## The patch

`cel-step-limit.patch` (S5, `.collide/planning/spikes/sp-0005-fv-schema-expressions/record.md`, finding 5) adds a
cooperative step counter:

- `src/steps.rs` (new): a thread-local counter and limit. `steps::set_limit(n)` resets the counter and sets the budget
  for the current thread; `steps::used()` reads the counter; the limit is `u64::MAX` until a caller sets one.
- `src/lib.rs`: `pub mod steps;`.
- `src/objects.rs`: `crate::steps::tick()?;` as the first line of `Value::resolve_val`, which every expression node
  passes through (macro bodies included). A tick past the limit returns an ordinary `ExecutionError::FunctionError`
  with function `cost` and message `step limit exceeded`.

Apply it to a pristine copy of the package, from the package root: `patch -p1 < cel-step-limit.patch`. It applies to
0.15.0 as published and reproduces `src/` here exactly (checked when the crate was vendored).

The patch is `steps.rs` as `rustfmt` writes it (37 lines; S5's 34-line original is the same code with one call on a
single line), because `cargo fmt --all` formats path dependencies and the repository's formatting check runs it.

### Offering it upstream

The patch is self-contained and has no Collide names in it. To offer it to `cel-rust/cel-rust`: apply it to
`cel/` in a checkout, and describe the counter as an opt-in limit with no change to ordinary results. S5 measured
the overhead as within noise on ordinary expressions (2.00 to 3.12 ms patched against 1.98 to 3.26 ms stock) and
the counter stopping every runaway case measured.

## Upgrading

1. Fetch the new package version into a scratch directory, copy its `src/`, `README.md` and `CHANGELOG.md` over
   this directory, and re-apply `cel-step-limit.patch` (regenerate it if it no longer applies).
2. Update the version and commit in the table above and run `cargo test --workspace`: the runaway-expression test in
   `core/tests/cel_step_budget.rs` fails (times out) if the tick is lost.

## Why a path dependency outside the workspace

The root `Cargo.toml` lists `vendor/cel` in `exclude`, so it is not a workspace member: `cargo clippy --workspace` and
`cargo test --workspace` skip it, and its dependencies resolve into the root `Cargo.lock`. `cargo fmt --all` still
reads it (see above).
