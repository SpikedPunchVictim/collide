---
id: ts-0011-at
kind: task
status: done
title: "pack-sdk outline"
owners: []
links:
  - { type: blocked-by, to: ts-0010-yb }
  - { type: blocked-by, to: ts-0027-ar }
---

## Goal

`@collide/pack-sdk`: `defineHook`, `defineProvider`, `defineObserver`, the `ctx` object (`ctx.state`, `ctx.config`, `ctx.dataDir`, `ctx.report.span`, `.metric` and `.progress`, `ctx.ok`, `ctx.fail`, `ctx.refuse`) and the generated types, with stubbed bodies tagged `unimplemented("sdk: ...")`. The SDK ships as compiled JavaScript with `.d.ts` files and zero runtime dependencies, because Node refuses to strip types under `node_modules`.

## Success criteria

- `tsc --noEmit` passes, and the types are the generated ones, not a hand-written copy.
- The compiled output has no runtime dependency and uses erasable syntax only.
- Every stub carries a tag that belongs to a stage.

## Tests

Write first: type tests against the schema (a handler with a wrong result type fails to compile); a test that the compiled package's `package.json` lists no `dependencies`.

## Notes

- IMPLEMENTATION_PLAN.md, 0.6 (the SDK ships compiled; the context object), 0.15 (tagged stubs), Stage 2's `@collide/pack-sdk` bullet.
- Area: `packages/pack-sdk`.
- Done when the criteria hold. `defineProvider` becomes real in `ts-0028-hq`.
- Build: `pnpm --filter @collide/pack-sdk build` runs `tsc -p tsconfig.build.json` (TypeScript 7.0.2, which emits JavaScript and declarations) into `dist/`, which `.gitignore` already covers. `main`, `types` and `exports` point at `dist/`, and `files` is `["dist"]`. `erasableSyntaxOnly` is on in the package's `tsconfig.json`, so `typecheck` and `build` both enforce it. `rewriteRelativeImportExtensions` turns the `.ts` imports in `src/` into `.js` in the output. No bundler. The package is `version` 0.1.0, for `requires.sdk` (0.7). `pnpm test` builds into a temp directory itself, so it needs no `dist/`.
- The generator (`scripts/generate.ts`) now writes `.ts` extensions in the generated `index.ts` re-exports, and `src/index.ts` uses `export type * from './generated/index.ts'`. Extensionless re-exports in a `.d.ts` fail to resolve for a consumer under `moduleResolution: nodenext` in an ES module package (measured: TS2835, and the types are silently `any` under `skipLibCheck`). The `.ts` form resolves there (measured with a nodenext consumer of the built package). Generated files changed only in those lines.
- Zero runtime dependencies is tested twice: `package.json` has no `dependencies`, `peerDependencies` or `optionalDependencies`, and a scan of the compiled JavaScript finds only `./`, `../` and `node:` specifiers (`test/package.test.ts`, with a positive control for the scanner).
- Stub tags: `sdk: <name>`, written as `unimplemented("sdk: <name>")` (`src/unimplemented.ts`), which throws `unimplemented: <tag>`. The plan (0.15) says the skeleton keeps a table of tags and the stage that fills each, and that CI counts stubs per tag, but the registry file is `ts-0013-nz`'s and does not exist yet. So the SDK keeps its own: `packages/pack-sdk/stub-tags.json` (tag to stage heading) and `scripts/stubs.ts` (`pnpm --filter @collide/pack-sdk stubs` prints the count per tag). `test/stubs.test.ts` fails if a stub's tag is not registered, a registered tag has no stub, a tag is not a string literal, or a stage is not `Stage N: ...`. `ts-0013-nz` should fold `stub-tags.json` and the counter into the repo registry. `ts-0028-hq` shows `sdk: defineProvider` gone by removing its line from `stub-tags.json` with the stub; the test `defineProvider is counted once` then goes with it.
- Tags and stages, from the plan: `sdk: defineProvider` is Stage 2 (the planning slice, ts-0028-hq); `sdk: defineHook` and `sdk: ctx` (state, config, dataDir: Stage 4b's "`ctx.config`, `ctx.dataDir`") are Stage 4b; `sdk: ctx.report` and `sdk: ctx.result` are Stage 7 (Report files: "the SDK's `ctx.report`"; the result file is 0.5's); `sdk: defineObserver` is Stage 12 (the plan moves the first observer onto the SDK there; it names no earlier stage).
- Shape I chose where the plan names none. Collide-made messages (a handler's params, the hook input, an observer event) use the generated `...Output` types; what a pack writes (results) uses `...Input`, so a pack may leave out a defaulted key. `HookHandler` is `(input, ctx) => Promise<HookResultInput | void>`: it returns a result (the only way to say `pending` or `unverified`, which `ctx` has no method for) or nothing after `ctx.ok`, `ctx.fail` or `ctx.refuse`, which write the result file and return what they wrote. `ctx.report.span(name, run, attrs?)` wraps a function. `ProviderHandlers` has one function per method, `watch` and `preview` optional; `watch` gets a `notify(method, params)` typed by `ProviderNotifications` (the six `watch` notifications). `defineProvider`, `defineHook` and `defineObserver` return `Promise<void>`. `ts-0028-hq` may add an options argument for the streams.
- Type tests (`test/sdk-types.test.ts`, run by `pnpm typecheck`): each handler's parameter and result type is proved equal to the generated type, and a wrong result type, a missing required handler and wrong `ctx` arguments each fail to compile with `@ts-expect-error`. TypeScript does not flag an unknown key in an object literal returned from an async arrow, so the tests do not claim it.
- Left for later items: every stub body (`defineHook`, `ctx` and the observer loop are not in ts-0028-hq, which is `defineProvider` only), the JSON-RPC server and its framing code (ts-0028-hq), built-in packs compiled into the bundle with this SDK (ts-0029-ea), and the repo-wide registry and CI count (ts-0013-nz). The item's wording "Every stub carries a tag that belongs to a stage" is met by the SDK's own registry above.

