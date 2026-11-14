---
id: ts-0007-ga
kind: task
status: ready
title: "Repo layout and workspace builds"
owners: []
links:
  - { type: blocked-by, to: ts-0006-ze }
---

## Goal

Create the empty workspace, so every later item has a place to land: the `core` and `host` crates (`host` holds the `collide` and `collide-daemon` binaries; the shim is `collide shim`), `app/src-tauri`, `app/` (React) and `packages/pack-sdk`. Create the built-in packs `packs/collide` and `packs/plan` (`collide/plan`), the fixture packs `fixtures/packs/loop` and `fixtures/packs/pr-ci`, and `fixtures/packs/orchard`, a fixture copy of the reference pack's manifest and hooks. Each of the five pack directories holds a placeholder `pack.yaml` (`apiVersion: collide/v0` and a `name`) that the pack contract item (`ts-0012-ua`) replaces. If the Orchard repo's pack is not to hand, write the fixture copy from the example manifest in 0.7.

## Success criteria

- `cargo build`, `tsc --noEmit` and the app build pass on the empty outlines.
- The directories above exist, with the names above. `packs/collide` and `packs/plan` are the two built-in packs; nothing under `fixtures/` is built into the app.
- No file outside `fixtures/packs/orchard` and the pack directories names the reference pack (the no-names check of `ts-0013-nz` will enforce this).

## Tests

Write first: a smoke test per crate and per package (each fails until its crate or package exists), and a test that lists the five pack directories and finds a `pack.yaml` in each.

## Notes

- IMPLEMENTATION_PLAN.md, Stage 2: Skeleton, Repo layout; 0.4 (the built-in packs); 0.7 (the fixture copy of the reference pack).
- Area: repo root, `core`, `host`, `app/`, `app/src-tauri`, `packages/pack-sdk`, `packs/`, `fixtures/packs/`.
- Done when the criteria hold and a fresh clone builds with the commands above.
