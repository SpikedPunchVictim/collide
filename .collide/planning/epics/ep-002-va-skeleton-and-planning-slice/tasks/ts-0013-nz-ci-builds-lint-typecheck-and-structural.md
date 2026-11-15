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
