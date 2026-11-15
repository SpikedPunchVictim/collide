---
id: ts-0072-sk
kind: task
status: ready
title: "Fix the cargo doc link warnings"
owners: []
links: []
---

## Goal

`cargo doc --workspace --no-deps` prints six warnings, all in doc comments. Fix each one so the command prints none. Change doc comments only: no code, no behavior, no tests.

The warnings, as `cargo doc` gives them on 2026-11-15:

- `core/src/owner/task.rs`, the module doc: unresolved link to `PreviewResult`.
- `core/src/replay.rs`, the module doc: unresolved link to `crate::testing` (the module exists only under the `testing` feature).
- `core/src/replay.rs`, the module doc: redundant explicit link target.
- `core/src/yaml.rs`, the module doc: public documentation for `yaml` links to the private item `hardened_options`.
- `host/src/launch.rs`, the doc of `ensure_daemon`: links to the private item `start_launchd_job`.
- `host/src/run_cli.rs`, the module doc: unresolved link to `EXIT_CODES`.

A link to an item that cannot be linked becomes plain code text (`` `name` ``) or points at the public item it means. A redundant target is dropped.

## Success criteria

- `cargo doc --workspace --no-deps` prints no warning.
- `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all --check` pass.
- Only doc comments changed (`git diff` shows `//!` and `///` lines only).

## Tests

None new: the change is to doc comments. The checks above are the test.

## Notes

- The dogfood task of Stage 3b (part L): `ts-0070-mk` runs it through `collide run` in a fresh worktree of Collide, with haiku and a $0.50 cap (owner's choice, 2026-11-15). The worktree and branch are left for the owner to review; nothing is merged or pushed by the run.
- Small on purpose: a first real phase should be cheap to check by eye.
