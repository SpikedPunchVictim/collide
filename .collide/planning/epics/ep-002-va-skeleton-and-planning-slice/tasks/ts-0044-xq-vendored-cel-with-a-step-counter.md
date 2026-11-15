---
id: ts-0044-xq
kind: task
status: active
title: "Vendored cel with a step counter"
owners: []
links:
  - { type: blocked-by, to: ts-0007-ga }
---

## Goal

Vendor the `cel` crate at 0.15 into the repo with S5's step-counter patch (one tick at the top of `Value::resolve_val`, about 34 lines), exposed as a step budget a caller sets, and use the vendored copy from `core`. The per-task and per-pass budgets, the source cap, the list caps, `catch_unwind` around compile, and the static cost estimate belong to Stage 4b. Here the patch exists, is pinned and stops a runaway expression.

## Success criteria

- A runaway expression (a triple-nested macro over a few thousand elements) stops with a step-budget error instead of running on.
- An ordinary expression gives the same answer as with the unpatched crate, within the step budget.
- The vendored crate's version and the patch are recorded in a file beside it, so the patch can be offered upstream.
- `core` depends on the vendored copy, not the registry crate.

## Tests

Write first: the runaway-expression test (observed hanging or failing to stop against the unpatched crate, with a short timeout); an ordinary-view equality test.

## Notes

- IMPLEMENTATION_PLAN.md, 0.9 (CEL runs on untrusted task data; the step counter), 0.15 (Chosen crates), Stage 2: Skeleton, Tests (the step counter's patch, vendored, stops a runaway expression). S5's record: `.collide/planning/spikes/sp-0005-fv-schema-expressions/record.md`.
- Area: `vendor/cel` (or the repo's vendor directory), `core` (the dependency).
- Done when the criteria hold.
- Built:
  - `vendor/cel/`: `cel` 0.15.0 as published (`src/`, `Cargo.toml`, `README.md`, `CHANGELOG.md`) with S5's patch applied: `src/steps.rs` (`steps::set_limit(n)`, `steps::used()`, the crate-private `tick()`; a thread-local counter and limit, `u64::MAX` until set), `pub mod steps;` in `src/lib.rs`, and `crate::steps::tick()?;` at the top of `Value::resolve_val` in `src/objects.rs`. A tick past the limit is an ordinary `ExecutionError::FunctionError` (function `cost`, message `step limit exceeded`). `vendor/cel/cel-step-limit.patch` is the patch against the pristine package (`patch -p1`; checked to reproduce `src/` exactly) and `vendor/cel/VENDORED.md` records the version, the upstream commit, what was and was not vendored, the patch, how to offer it upstream and how to upgrade.
  - Wiring: root `Cargo.toml` `exclude = ["vendor/cel"]`; `core/Cargo.toml` `cel = { path = "../vendor/cel" }` (default features, as S5 measured). `checks/src/stubs.rs`: `vendor` joins `NOT_SKELETON`, because the vendored crate holds its own `todo!()` calls.
  - Tests, written first: `core/tests/cel_step_budget.rs` (4): `a_runaway_expression_stops_with_a_step_budget_error` (a triple-nested `exists` macro over 3,000 elements, a budget of 1,000,000 steps, run on a thread with a 30 s deadline; it returns in about 1.6 s patched and times out with the tick commented out), `the_steps_a_stopped_expression_used_are_just_over_the_budget`, `ordinary_expressions_answer_as_the_unpatched_crate_within_the_budget` (10 expressions; the expected answers were checked against the registry crate 0.15.0 in a scratch crate, all equal), `with_no_budget_set_nothing_is_limited`. One checks test extended: `tests_fixtures_vendored_code_comments_and_the_declaration_of_unimplemented_are_not_stubs` (red before the `stubs.rs` change).
  - New in `Cargo.lock` (only additions): `cel` 0.15.0 (path), `antlr4rust` 0.6.0, `base64` 0.22.1, `borsh` 1.8.1, `borsh-derive` 1.8.1, `cfg_aliases` 0.2.2, `chrono-tz` 0.10.4, `lazy_static` 1.5.1, `minimal-lexical` 0.2.1, `murmur3` 0.4.1, `nom` 7.1.3, `pastey` 0.2.3, `phf` 0.12.1, `phf_shared` 0.12.1, `typed-arena` 2.0.2. The dependency-direction check stays clean.
- Decisions for the owner:
  1. `vendor/cel` is a path dependency listed in the workspace `exclude`, not a member. A member would put 1.5 MB of third-party code under `clippy -D warnings` and `cargo test --workspace` (its tests are not vendored). Passed on: a member, which would need lint allows on the crate. Note `cargo fmt --all` formats path dependencies even when excluded, so the vendored `src/` is under the repository's `rustfmt`; the upstream files were already clean, and the patch's `steps.rs` is the rustfmt-formatted version (37 lines against S5's 34, the same code).
  2. The patch is S5's unchanged apart from rustfmt and the first doc comment (it said "Spike patch"). The budget error is the existing `FunctionError` with function `cost`, not a new error variant, so the patch stays small and upstreamable; Stage 4b can match on the message or add a typed error then. Passed on: a new `ExecutionError` variant (a public-enum change upstream would have to accept).
  3. The published `cel` package ships no LICENSE file (the manifest says MIT; the repository holds the text), and this build had no network to fetch it. `VENDORED.md` says so. Please add the upstream `LICENSE` text to `vendor/cel/` before relying on the vendored copy in a release. Passed on: writing an MIT text with a guessed copyright line. Resolved before commit: `vendor/cel/LICENSE` is the upstream file at the vendored commit.
  4. Tests, benches, `Cargo.lock` and the dev-dependencies of the package are not vendored (the manifest's `[[test]]`, `[[bench]]` and `[dev-dependencies]` are removed), to keep the copy to what is built. Passed on: vendoring upstream's `tests/macros.rs`, which would need `criterion`, `dhat` and `serde_bytes` resolved into `Cargo.lock` for a crate that is not a member.
  5. `vendor` is skipped by the stub count only (the vendored code has its own `todo!()` calls); the names check still reads it. Passed on: skipping `vendor` in the shared file walk, which would take third-party code out of the names check too.
  6. `cel` is used with its default features (regex, chrono, ext_encoders), as S5 measured, which brings `chrono-tz` and `base64` into `core`'s closure. Passed on: `default-features = false`, which would drop `matches` and timestamps from views before Stage 4b says which are wanted.
- Not done by design: per-task and per-pass budgets, the 1 KB source cap, list caps, `catch_unwind` around compile and the static cost estimate (Stage 4b); a multi-thread budget (the counter is thread-local, as S5 noted); a memory bound (steps do not bound memory); no `core` wrapper over `cel::steps` (nothing in `core/src` evaluates an expression until Stage 4b, so the tests call the vendored crate directly); the upstream pull request itself.
