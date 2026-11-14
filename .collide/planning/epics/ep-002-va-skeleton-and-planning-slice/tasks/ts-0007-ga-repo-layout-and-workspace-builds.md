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

Create the `core` and `host` crates, `app/src-tauri`, `app/` (React), `packages/pack-sdk`, `packs/collide`, `packs/plan` (`collide/plan`), and the fixture packs `fixtures/packs/loop` and `fixtures/packs/pr-ci`, with a fixture copy of the reference pack's manifest and hooks.

## Success criteria

- `cargo build`, `tsc --noEmit` and the app build pass on the empty outlines.

## Tests

- A smoke test per crate and package.

## Notes

IMPLEMENTATION_PLAN.md, Stage 2: Skeleton, Repo layout.
