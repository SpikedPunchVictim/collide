---
id: ts-0109-qj
kind: task
status: done
title: "SDK hooks: defineHook and the hook context"
owners: []
links:
  - { type: blocked-by, to: ts-0108-dq }
---

## Goal

Make `@collide/pack-sdk` v0 complete for what Stage 4b names: `defineHook` and the parts of the hook context that
read, `ctx.state`, `ctx.config` and `ctx.dataDir`. A pack's hook written with the SDK then reads its input, runs, and
leaves a result file, the same as a shell or Python hook does by hand.

What exists. `packages/pack-sdk/src/provider.ts` `defineProvider` works (stdio, JSON-RPC, `notify`, the optional
`watch` and `preview`), and the package builds to `dist` with `tsc`. `src/context.ts` has the `HookContext` types and
`createHookContext`, and `src/hook.ts` has `defineHook` and the `HookHandler` type; both are `unimplemented` stubs
(`sdk: ctx`, `sdk: defineHook`). The generated types for the hook input and result come from `core`
(`core/src/hook.rs`, `HookInput` and `HookResult`). A provider needs no `ctx`: its settings arrive in `initialize` and
its data folder is in `COLLIDE_PACK_DATA`.

What this adds.
- `createHookContext(input)`: `state` and `config` from the input, `dataDir` from `COLLIDE_PACK_DATA`.
- `defineHook(handler)`: reads stdin to the end, parses it as the hook input, builds the context, awaits the handler,
  and writes what the handler returned to the file named by `COLLIDE_RESULT`. A handler that throws exits non-zero with
  its message on stderr, and writes no file (0.5: a missing file is `error`).
- `ctx.report` and `ctx.ok`, `ctx.fail`, `ctx.refuse` stay Stage 7's (`sdk: ctx.report`, `sdk: ctx.result`): a call to
  them throws the stub's tag, so nothing pretends to work.

## Success criteria

- A hook that returns `{ status: 'ok', outputs: { a: 1 } }` leaves exactly that in the result file, written whole (a
  temp file, then a rename), and exits 0.
- `ctx.state`, `ctx.config` and `ctx.dataDir` equal the input's `state` and `config` and the environment's
  `COLLIDE_PACK_DATA`. Reading `dataDir` with the variable unset throws a message that names the variable.
- A result whose `status` is not one of the six (`ok`, `fail`, `unverified`, `pending`, `refused`, `error`) is not
  written: the hook exits non-zero and says why. A `pending` result keeps its `retryAfter`.
- Input that is not one JSON object, an unset `COLLIDE_RESULT`, and a handler that throws each exit non-zero with a
  message on stderr and leave no result file.
- `ctx.ok(...)` and `ctx.report.metric(...)` still throw their stage-7 tags.
- The tags `sdk: ctx` and `sdk: defineHook` count zero stubs and move to `FILLED_TAGS` in `checks/tests/real_tree.rs`.
  `sdk: ctx.report`, `sdk: ctx.result` and `sdk: defineObserver` stay.
- The compiled `dist` exports `defineHook` and runs under `node` with no `.ts` in it (the cache supplies the SDK as
  compiled JS, 0.6).

## Tests

Write these first.

- A hook script run with `node` as a real child with piped stdin, a temp `COLLIDE_RESULT` and `COLLIDE_PACK_DATA`,
  in the way `test/provider-stdio.test.ts` runs a provider: ok, `pending` with `retryAfter`, `unverified`, a thrown
  error, malformed input, a missing variable.
- The context values (`state`, `config`, `dataDir`) from a sample input shaped like the generated type.
- A result file is written whole: the write goes to a temp file in the same folder and is renamed, checked by the
  folder's listing after the hook ends (no temp file left, one result file).
- The stub test in `test/stubs.test.ts` changes: `defineHook` is no longer in its list, and `ctx.ok` and `ctx.report`
  take its place.
- A type test that a handler's return type accepts `void` and a `HookResultInput`, and rejects a status outside the six.
- The compiled `dist` is loaded by a hook run with a plain `node` (not the workspace link).

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4b: "`@collide/pack-sdk` v0, compiled: `defineProvider`, `defineHook`, `ctx.config`, `ctx.dataDir`".
  - 0.6: The context object; Packs in TypeScript (compiled SDK, supplied by Collide). 0.5: Result (the file is the
    verdict; a missing or invalid file is `error`), Input.
  - Stub tags tagged 4b: `sdk: ctx`, `sdk: defineHook`.
- Carried from `ts-0011-at`: the SDK outline, with `ctx.state`, `ctx.config`, `ctx.dataDir` and `defineHook` left
  for 4b.
- **Decision for the item, with a recommendation: the result when the handler returns nothing.** With `ctx.ok` not built
  until Stage 7, a handler that returns `void` has written nothing, so the file is missing and the point is `error`.
  Recommend that: it is what the plan says for a missing file, and Stage 7 changes nothing here. Alternative: treat
  `void` as `ok` with no outputs. Not recommended, since a hook that forgot to answer would pass a gate.
- **Decision for the item: input validation.** The SDK parses the JSON and checks it is an object, and does not validate
  it against the generated schema, as `defineProvider` does not for its params. A hook that needs more checks it.
- **Decision for the item: the size of stdin.** Read it all with no cap here. The hook input is Collide's own, and the
  task text in it was already capped by the corpus (0.9). Say so in a comment.
- Out of scope: `ctx.report`, `ctx.ok`, `ctx.fail`, `ctx.refuse` (Stage 7), `defineObserver` (Stage 12), the Python and
  shell paths, which need nothing.
- Area: `packages/pack-sdk/src/{context,hook}.ts`, `packages/pack-sdk/test/`, `checks/tests/real_tree.rs`.
- The test rule: no test touches the real home, real launchd or a real daemon. A test that runs the `collide` binary
  clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as
  `host/tests/common/mod.rs` `TempRoot` does. This item's tests are Node and run no daemon. Settings files are
  fixtures, never the owner's own `settings.json`.
- Stage 4b item (revision 23 scope, split 2026-11-17). Items of Stage 4b run one at a time, in id order, because
  they share `core`, `host` and the generated files.
- Nothing here runs a real `claude`.
- Owner decisions (2026-11-17, at the split): Collide supplies `yaml` with the SDK (the owner's choice of option 3 in
  `ts-0110-ra`; plan 0.6 changed), and this item adds it: `yaml` 2.9.1 as a dependency of `@collide/pack-sdk` and a
  subpath export `@collide/pack-sdk/yaml` that re-exports `parse` and `YAMLParseError`, nothing more. The SDK copy that
  `pnpm build:packs` stages and `ensure_sdk` copies into the cache carries `yaml` in its own `node_modules`, so a
  materialized repo pack resolves it through the SDK link. The SDK's version is raised so a pack can require the export.
  Tests: a materialized fixture pack imports `@collide/pack-sdk/yaml` and parses a document; a pack that holds its own
  `node_modules` is still refused. The rest: the recommendations above (a hook that returns nothing is `error`).

### Built

- `packages/pack-sdk/src/hook.ts`: `defineHook` is real. It reads stdin to the end (no cap, said in a comment),
  parses one JSON object (`parseInput`), checks `COLLIDE_RESULT`, builds the context, awaits the handler, checks the
  result (`checkResult`: an object whose `status` is one of the six) and writes it (`writeResult`: a temp file in the
  result's folder with `wx`, then a rename; the temp file is removed when either step fails). Any failure writes its
  message to stderr, sets `process.exitCode = 1` and leaves no result file. A handler that returns nothing and wrote no
  file fails the same way; one that returned nothing after writing the file itself (Stage 7's `ctx.ok`) exits 0.
- `packages/pack-sdk/src/context.ts`: `createHookContext` gives `state` and `config` from the input and `dataDir` as a
  getter that reads `COLLIDE_PACK_DATA` when it is read (unset or empty throws a message naming it). `ctx.report` and
  `ctx.ok`, `ctx.fail`, `ctx.refuse` are thin delegates that call `createReport` and `createResultWriter` on first use,
  so a call still throws `unimplemented: sdk: ctx.report` or `sdk: ctx.result`. Those two stubs are unchanged.
- `packages/pack-sdk/src/yaml.ts` (new): `export { parse, YAMLParseError } from 'yaml'`, nothing more.
  `package.json`: version 0.1.0 to 0.2.0, `dependencies: { yaml: "2.9.1" }`, and the `./yaml` entry in `exports`.
  `pnpm-lock.yaml` gains only that importer dependency (installed with `pnpm install --offline`).
- `scripts/build-packs.ts`: the staged SDK copy now carries its own dependencies in its own `node_modules`
  (`copyOwnDependencies`, one rule for packs and the SDK; a dependency with dependencies of its own is still refused).
  `ensure_sdk` needed no change: it copies the folder as it is, so `<cache>/sdk/0.2.0/node_modules/yaml` is there.
- `checks/tests/real_tree.rs`: `sdk: ctx` and `sdk: defineHook` moved to `FILLED_TAGS` (47 entries).
- Tests. `test/define-hook.test.ts` (25) with `test/hook-script.ts`: a real `node` child with a cleared environment
  and piped stdin: ok, pending with `retryAfter`, unverified, thrown, returns nothing, bad status, non-object result,
  six kinds of bad input, unset `COLLIDE_RESULT`, context values, `dataDir` unset, whole write (folder listing; a
  symlink at the path is replaced and not written through; a result that cannot be put in place leaves no temp file),
  and `createHookContext` in process. `test/stubs.test.ts` now lists `ctx.ok` and `ctx.report`. `test/package.test.ts`:
  one dependency (yaml 2.9.1), `yaml.js` is the only file that imports it, the `./yaml` export, the version is at least
  0.2.0, and three tests of a hook run by plain node through a link to a staged copy of the compiled `dist` (no
  workspace link): it leaves its result, `@collide/pack-sdk/yaml` parses a document, and it exports exactly
  `YAMLParseError` and `parse`. `host/tests/pack_cache.rs` (3): a materialized pack script imports
  `@collide/pack-sdk/yaml` from the real built bundle and parses a document; the SDK copy carries its `node_modules`
  read-only and the entry link reaches it; a pack holding `node_modules/yaml` is refused. `host/tests/builtin_packs.rs`
  (1): the bundle's SDK copy holds yaml 2.9.1 as real files. The type test that a handler may return `void` or a
  `HookResultInput` and not a status outside the six already exists (`sdk-types.test.ts`, `hook-types.test.ts`) and
  was not changed.
- Runtime red. Before the code, all 24 hook tests and the 2 stub tests failed at runtime against the stubs
  (`unimplemented: sdk: defineHook`, `sdk: ctx`). `package.test.ts` first failed in its setup (no `yaml` installed); I
  then saw four of its tests fail with `src/yaml.ts` removed and the version at 0.1.0, and five with `dependencies`
  and the `./yaml` export removed. Mutations, each seen red then restored: a direct `writeFile` instead of temp and
  rename (the symlink test); no temp cleanup (the cannot-be-put-in-place test); no SDK own-dependency copy in
  `build-packs.ts` (`the_sdk_copy_in_the_bundle_carries_yaml...` and
  `a_pack_script_that_imports_the_sdk_yaml_export_parses_a_document`, `Cannot find package 'yaml'`); a stub under
  `sdk: ctx` (`every_filled_tag_stays_registered...`). Not red before the change, because the code already did it:
  `the_sdk_copy_carries_its_own_node_modules...` and `a_pack_that_holds_its_own_yaml_in_node_modules_is_still_refused`
  (they pin the rule).
- Checks: `cargo build --workspace`, `cargo test --workspace` (no failure), clippy `-D warnings`, `cargo fmt --check`,
  `pnpm test` (pack-sdk 96, plan 283, app 231), `pnpm typecheck`, app build, `pnpm build:packs` and `pnpm check` pass.

### Decisions for the owner

1. A handler that returns nothing: `error`, as recommended. The hook also exits non-zero with a message, unless the
   result file already exists (so Stage 7's `ctx.ok` followed by a bare return needs no change here). A stale file at
   `COLLIDE_RESULT` would hide a forgotten return, so the executor (Stage 7) should hand each hook a fresh path.
2. Failure is an exit code, not a rejected promise: `defineHook` prints the message and sets `process.exitCode`, so
   `await defineHook(...)` always resolves and the hook ends by itself. Alternative: reject and let Node print a
   stack. Recommend keeping this: the message is the whole output.
3. Result checks are the status and the object only. `message`, `outputs` and `retryAfter` pass through unchecked;
   core's `effective_result` checks them and the outputs cap. Alternative: check them in the SDK too. Not recommended
   until a second reader needs it.
4. The SDK's version is 0.2.0, so a pack requires `>=0.2 <0.3`. `fixtures/packs/orchard` and `loop` still say
   `>=0.1 <0.2`: no test loads them against the real SDK, and `ts-0110-ra` should move the Orchard fixture to
   `>=0.2 <0.3`.
5. `packs/plan` keeps its own `yaml`, so the bundle holds two copies (one beside the pack, one in the SDK copy).
   Alternative: have it import `@collide/pack-sdk/yaml`. Not now, as the item says.
6. Plan text to propose (not applied): 0.6 still says `@collide/pack-sdk` ships "with zero runtime dependencies".
   Suggest: "with one runtime dependency, `yaml` 2.9.1, which the compiled SDK copy carries in its own
   `node_modules`".

- Owner decisions (2026-11-17, at the build): decisions 1 to 5 as recommended, on delegation; `ts-0110-ra` moves the
  Orchard fixture's `requires.sdk` to `>=0.2 <0.3` because it imports `@collide/pack-sdk/yaml`, and
  `fixtures/packs/loop` keeps `>=0.1 <0.2` (its tests use a stand-in SDK). Decision 6 is applied: plan 0.6 now says the
  SDK has one runtime dependency, `yaml` 2.9.1.

### Not done, by design

- `ctx.report`, `ctx.ok`, `ctx.fail`, `ctx.refuse` (Stage 7, tags `sdk: ctx.report` and `sdk: ctx.result`);
  `defineObserver` (Stage 12).
- The `.d.ts` files are not in the staged SDK copy (`build-packs.ts` drops every `.ts`), so `tsc` in a pack sees no
  types for `@collide/pack-sdk/yaml`. The plan leaves `collide pack test` and its types to the second-pack stage.
- Moving the Orchard fixture to `>=0.2 <0.3` and importing `@collide/pack-sdk/yaml` there: `ts-0110-ra`.
