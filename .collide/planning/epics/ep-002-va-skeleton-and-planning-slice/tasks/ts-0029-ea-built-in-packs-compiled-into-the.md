---
id: ts-0029-ea
kind: task
status: done
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
- Build step: `pnpm build:packs` runs `scripts/build-packs.ts` (plain Node, erasable TypeScript, no new tool). It builds the SDK (`pnpm --filter @collide/pack-sdk build`), then for each directory under `packs/` copies the pack (everything except `node_modules`, `package.json`, `tsconfig*.json`, `src`, `test`, `dist`) into a `<out>.staging` folder, compiles `src/` with the pack's own `tsc -p tsconfig.build.json --outDir <bundle>/src` (`packs/plan/tsconfig.build.json` extends its `tsconfig.json`, turns emit on and sets `rewriteRelativeImportExtensions`), and writes Collide's `package.json` (`{"private": true, "type": "module"}`, as in 0.6) so the `.js` files load as ES modules. It then replaces `<out>` with the staging folder and takes the write bits off every file and directory. `--out <dir>` picks another folder; the default is the git-ignored `app/src-tauri/resources/builtin-packs/` (`.gitignore` covers all of `app/src-tauri/resources/`). A rebuild unlocks (`chmod u+w`) and removes the previous output first; the rename happens before the lock because macOS refuses to rename a directory that has no write bit.
- SDK copy: not inlined. It sits in a bundle-local `plan/node_modules/@collide/pack-sdk/` with its `package.json` and built `dist/` (the `.d.ts` and `.map` files are left out, so the bundle holds no `.ts` file at all). `yaml` 2.9.1 gets there the same way: the script copies each name in the pack's `package.json` `dependencies` from the installed copy (`realpath` of `packs/plan/node_modules/<name>`, symlinks dereferenced, pnpm's `node_modules/.bin` shims inside it dropped) into `plan/node_modules/<name>/`. A dependency that has dependencies of its own fails the build (`yaml` has none), since no pack needs transitive copying yet. No bundler. `packs/collide` has no `package.json` or `src/` yet, so it stages as its `pack.yaml` alone.
- Loader: `host/src/builtin_packs.rs`. `BuiltinPacks::new(bundle_dir).provider("collide/plan", "plan", &Environment { path })` loads `<bundle>/plan/pack.yaml` through `core::yaml::load_yaml_file` into `PackManifest`, checks its `name`, reads `providers.plan.script` and the `node` entry of `requires.tools`, and returns `ProviderLaunch { pack_dir, program, args }` (`.command()` builds a `std::process::Command`). `program` is the first executable `node` in the `PATH` directories (empty entries skipped), and `args` is the compiled entry: the manifest says `src/main.ts` (what the pack runs as from source), and the bundle holds compiled code, so a `.ts` script maps to the `.js` beside it. The bundle directory is a parameter: the app passes its resource dir (`builtin-packs/`), and nothing in the loader knows the repo. Built-in names map to bundle folders by a two-entry table in the loader (`collide`, `collide/plan`).
- `PackManifest` (core) gained exactly `apiVersion`, `name`, `requires.tools` (`name`, optional `version`) and `providers.<name>.script`, all optional, still `deny_unknown_fields`; `requires.sdk` and every other key stay for `ts-0012-ua`, which should also make `apiVersion` and `name` required. `pnpm gen` regenerated `schemas/PackManifest.*.json` and the SDK's `PackManifest.*.ts`.
- `packs/plan/pack.yaml` gained `requires.tools: [{ name: node, version: ">=22.18" }]` (the plan's example line; 22.18 is where Node strips types without a flag, which `node src/main.ts` from source needs, and the repo runs 24.18) and `providers: { plan: { script: src/main.ts } }`.
- Fresh clone: Tauri's build script refuses a `bundle.resources` path that does not exist (`resource path ... doesn't exist`), so `app/src-tauri/build.rs` creates `resources/builtin-packs/` when it is absent (the same way it creates `app/dist`), and fails a release build while that folder is empty ("run `pnpm build:packs`"). `tauri.conf.json` has `bundle.resources: { "resources/builtin-packs/": "builtin-packs/" }`. No `beforeBuildCommand` is set, since the config has none for the front end either; whoever turns `bundle.active` on should add `pnpm build:packs` there. Tests do not use the staged folder: `host/tests/builtin_packs.rs` builds the bundle itself into `target/tmp/` through the same script (once per run, under a lock, since the script rebuilds the SDK's `dist/` in place), so nothing skips when the staged folder is missing. They need `node` and `pnpm` on `PATH` and the pnpm install.
- Tests (`host/tests/builtin_packs.rs`, 10): loader found and not found (exact message), a non-executable `node` is not found, an empty `PATH`, an unknown pack and a missing provider, bundle contents, no `.ts` file anywhere in the bundle, no write bit and a failed write into it, a rebuild over read-only output, and the bundled provider's `initialize` and `list` over `packs/plan/test/fixtures/basic`. Core: a manifest test for the new keys, and the key-set tests in `core/tests/planning_contracts.rs` and `host/tests/schema_command.rs` updated.
- Read-only side effects, and what handles them. (1) Tauri copies `bundle.resources` to `target/<profile>/builtin-packs/` keeping the 0444 modes and then cannot overwrite that copy, so `app/src-tauri/build.rs` makes the old copy writable and removes it before `tauri_build::build()` (the profile folder is found from `OUT_DIR`). (2) The host tests unlock and delete the bundles they build when they are done, also on a panic (`Drop` guards in `host/tests/builtin_packs.rs`), so `rm -rf target/tmp` and `cargo clean` work. (3) `git clean -fdx` and `rm -rf` cannot remove a staged bundle: run `chmod -R u+w app/src-tauri/resources` first. `pnpm build:packs` itself unlocks the previous output.
