---
id: ts-0013-nz
kind: task
status: active
title: "CI: builds, lint, typecheck and structural checks"
owners: []
links:
  - { type: blocked-by, to: ts-0007-ga }
---

## Goal

CI runs build, clippy, lint and typecheck, the stub count per tag, the dependency-direction check and the no-names check. The stub-tag registry (a file mapping each tag to the stage that fills it) is made here, and the outline items add their tags to it.

## Success criteria

- A test fails if `core` gains a Tauri, SQLite or process dependency.
- A seeded `orchard` string in `core`, and a seeded `CLAUDECODE` or `ANTHROPIC_` string outside the Claude adapter, each fail CI. The reference pack's name is allowed only under pack directories and test fixtures.
- CI prints the stub count per tag, and every tag belongs to a stage. A stub whose tag is missing from the registry fails CI.
- CI runs every test in the workspace, so the catalogue listing of `ts-0018-dd` prints in CI once that item lands.

## Tests

Write first: the no-names check with its positive controls (a seeded string in each forbidden place fails, a clean tree passes); the dependency-direction test with a seeded forbidden dependency; a stub with an unregistered tag.

## Notes

- IMPLEMENTATION_PLAN.md, 0.15 (Checks from the skeleton on; tagged stubs), Stage 2: Skeleton, CI and Success Criteria.
- Area: the CI configuration, a checks crate or script at the repo root, `core` (the dependency test).
- Done when the criteria hold and CI is green on the empty outlines.
- `ts-0011-at` made a pack-sdk-local registry first: `packages/pack-sdk/stub-tags.json` and `pnpm --filter @collide/pack-sdk stubs`. Fold it into the repo-wide registry here and retire the local script.

### Built (ts-0013-nz)

What exists:
- `checks/` (crate `collide-checks`, a workspace member; depends only on `collide-core` and `serde_json`, both already in the lock, so no new third-party dependency). `src/names.rs`, `src/deps.rs`, `src/stubs.rs`, a shared file walk in `src/walk.rs`, and the `collide-checks` binary (`names | deps | stubs | all [--root <dir>]`; exit 1 on a finding, 2 when it cannot run).
- Positive controls, written first, in `checks/tests/names.rs`, `deps.rs` and `stubs.rs`. They seed into scratch trees under cargo's `target/tmp` (removed on drop), never into the repository, and build every seeded name at run time so the controls themselves pass the checks. Each check was also broken once to see its controls fail on their own assertions.
- `checks/tests/real_tree.rs` runs the three checks over the repository inside `cargo test --workspace`, so the dependency-direction test of the success criteria fails in the ordinary test run.
- `stub-tags.json` at the repository root: the one registry, tag to stage id (`"4b"`, `"7"`, `"12"`; the ids of the `Stage` enum in `core/src/stage.rs`, checked by deserializing into it). The pack-sdk's local registry, `scripts/stubs.ts` and the `stubs` package script are removed; its test file keeps only the `unimplemented` and `define*` stub tests.
- `.github/workflows/ci.yml`, one job.
- Root `package.json`: scripts `check`, `check:names`, `check:deps`, `check:stubs`, and `packageManager: pnpm@11.17.0` (there was none; `pnpm/action-setup` reads the pnpm version from it). `Cargo.lock` gains the `collide-checks` entry only.

Run locally (from the repo root, with cargo on PATH):
- `pnpm check:names`, `pnpm check:deps`, `pnpm check:stubs` (prints the count per tag, `0` for a registered tag whose stubs are gone), or `pnpm check` for all three.
- `cargo test -p collide-checks` for the controls and the real-tree tests.

Rules as built:
- No-names: reads `.rs .ts .tsx .js .mjs .cjs .json .yaml .yml .toml .html .css .sh .py`. Not read: Markdown, `.collide/`, `docs/`, `private/`, `target/`, `node_modules/`, `dist/`, `app/src-tauri/resources` and `gen`. The reference pack's name (case-insensitive) is allowed only under a top-level `packs/` and under any `fixtures/` directory. `CLAUDECODE`, `CLAUDE_*` and `ANTHROPIC_*` (as words; `CLAUDE.md` is not a match) are allowed only in the Claude adapter, defined as `host/src/adapters/claude/`, `host/src/adapters/claude.rs` and `host/tests/adapters/claude/`. The adapter does not exist yet: `ts-0041-xf` must put it at one of these paths, or change the `ADAPTER_PATHS` constant in `checks/src/names.rs` in the same commit.
- The check found real hits: `core/tests/hook_protocol.rs` and `core/tests/catalogue.rs` used the reference pack's name in sample data. They now use `demo`; behaviour is unchanged.
- Dependency direction: reads `Cargo.lock` (offline, no cargo process), takes the closure of `collide-core` including dev-dependencies, and fails on `tauri`, `tauri-*`, `wry`, `tao`; `rusqlite`, `libsqlite3-sys`, `sqlite`, `sqlite3-src`, `sqlite3-sys`, `sqlx*`, `diesel`; `duct`, `subprocess`, `command-group`, `async-process`, `tokio-process`, `shared_child`, `portable-pty`, `pty-process`. It prints the chain that reaches the crate. It also fails on `std::process` in `core/src`, since the standard library can spawn a process with no dependency. The list is in `why_forbidden` in `checks/src/deps.rs`.
- Stubs: a stub is `todo!` or `unimplemented!` in `.rs`, or `unimplemented(` in `.ts` and `.tsx`, with the tag as one string literal (a call may span lines). Not counted: `tests/`, `test/` and `fixtures/` directories, `.d.ts`, comments, the declaration of the function. A stub with an unregistered or non-literal tag fails; a registered tag naming no `Stage` id fails.

CI choices:
- One job on `macos-latest`. Collide ships for macOS only, and `app/src-tauri` is a workspace member, so macOS builds it with no system packages; Ubuntu would need Tauri's Linux packages (webkit2gtk 4.1 and others) and would test a platform nothing ships on.
- Pinned: Rust 1.98.0 through rustup (no `rust-toolchain` file, which would make a local machine download that toolchain by name), Node 24.18.0, pnpm 11.17.0 from `packageManager`. Actions: `actions/checkout@v4`, `pnpm/action-setup@v4`, `actions/setup-node@v4` (pnpm store cache), `Swatinem/rust-cache@v2`. `permissions: contents: read`, no secrets.
- Steps are the local gates in order: install `--frozen-lockfile`, the `pnpm gen` stability check (shasum of `schemas/` and `packages/pack-sdk/src/generated` before and after), `pnpm test`, `pnpm typecheck`, the app build, `cargo build`, `cargo test` for the workspace, clippy `-D warnings`, `fmt --check`, `pnpm build:packs`, `touch app/src-tauri/build.rs && cargo build`, `cargo test` again with the bundle staged, then the three checks.

Not done by design:
- CI has not run: the main session pushes and watches it. The action major tags and the `pnpm/action-setup` reading `packageManager` are untested here.
- `tokio` is not on the forbidden list: the plan keeps `core` sans-IO but does not forbid the crate, and the lock cannot show its `process` feature. Owner decision if `core` should stay clear of it.
- The point-and-event listing ("CI also lists each catalogue point with the stage that makes it fire") is the `collide schema` catalogue test of `ts-0018-dd`, which runs under `cargo test --workspace`; this item adds nothing for it.
- Outline items add their tags to `stub-tags.json` with their stubs; a stage is done when its tags count zero.
