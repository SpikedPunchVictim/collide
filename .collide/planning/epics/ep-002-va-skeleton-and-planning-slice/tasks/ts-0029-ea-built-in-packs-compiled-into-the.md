---
id: ts-0029-ea
kind: task
status: ready
title: "Built-in packs compiled into the bundle"
owners: []
links:
  - { type: blocked-by, to: ts-0007-ga }
  - { type: blocked-by, to: ts-0014-yc }
---

## Goal

The build step and the loader for built-in packs. The step compiles `packs/collide` and `packs/plan` (TypeScript to JavaScript, with their own copy of the compiled SDK) into a read-only directory staged under `app/src-tauri/resources/builtin-packs/`, which the app build bundles. Built-in packs are not materialized into the cache, have no git tree id and ask for no trust. A loader in `host` returns, for a built-in pack, its directory and the command that starts its provider, using `node` from the environment it is given (the CLI's own until the daemon exists). For the slice the loader reads only the provider entry of `packs/plan/pack.yaml`. The full manifest check is `ts-0012-ua`. This item records whether the SDK copy is inlined or sits in a bundle-local `node_modules`.

## Success criteria

- The build produces the bundle directory with `collide/plan`'s provider compiled and its own SDK copy, and with no `.ts` file under the SDK.
- Started through the loader, the bundled `collide/plan` provider answers `initialize` and `list` over a fixture root.
- With no `node` on the given `PATH`, the loader's error names the built-in pack (`collide/plan`), says it needs `node` (the version `requires.tools` names), and lists where it looked.
- The bundle directory is not writable by the loader's process.

## Tests

Write first: the loader test with a fake `PATH` (found, and not found with the exact message); a bundle-contents test; an `initialize` and `list` round trip over the bundled copy.

## Notes

- IMPLEMENTATION_PLAN.md, 0.4 (built-in packs ship compiled in the app bundle), 0.6 (built-in packs are not materialized), Stage 2: Skeleton, Planning slice (`collide/plan` loads as a built-in pack).
- Area: `host` (the loader), the build scripts, `app/src-tauri` (the resources entry), `packs/`.
- Done when the criteria hold.
