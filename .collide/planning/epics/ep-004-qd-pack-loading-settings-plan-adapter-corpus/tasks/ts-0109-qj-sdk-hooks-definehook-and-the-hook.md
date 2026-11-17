---
id: ts-0109-qj
kind: task
status: ready
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
