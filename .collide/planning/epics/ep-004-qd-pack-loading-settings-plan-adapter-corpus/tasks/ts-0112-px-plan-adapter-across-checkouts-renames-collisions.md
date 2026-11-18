---
id: ts-0112-px
kind: task
status: done
title: "Plan adapter across checkouts: renames, collisions and watch"
owners: []
links:
  - { type: blocked-by, to: ts-0111-vn }
---

## Goal

Make the `plan` adapter ref-scoped in fact: it reads any checkout it is asked about, tells a rename and a collision
apart from a change, and watches the checkouts it is given, so a `plan set` in one worktree reaches Collide as a
`task.changed` for that checkout.

What exists. After `ts-0111-vn`, `fixtures/packs/orchard/providers/plan.ts` reads one checkout, writes through the fake
plan CLI and previews. `core/src/provider.rs` has the notification types (`TaskChanged`, `Rename`, `TaskRemoved`,
`TaskInvalid`, `TaskCollision`, `SourceStale`, `ChangeCause`), and `defineProvider` passes a `notify` to a `watch`
handler. `packs/plan` has no `watch` (it declares `watch: false`). `host/src/workspace.rs` shows how the host runs git;
the adapter will run its own `git`.

What this adds.
- `list` and `get` take `checkout`: the one named, else `root`. Every task carries its `checkout` and its `path`.
- Identity by (shortId, path). The same short id at two paths in two checkouts is a collision, reported as
  `task.collision {shortId, locations}`, never as a divergence.
- Renames from the id map and from git's rename detection, as `task.renamed {from, to, checkout}`; never a remove plus
  an add guessed to be one.
- `watch({checkouts})`: watches each checkout's `arch/` and its HEAD, debounced, plus a recheck every 10 s. It sends
  `task.changed` (with `from`, `to`, `diff`, `cause` of `head` or `worktree`, and `checkout`), `task.removed`,
  `task.invalid`, `task.renamed`, `task.collision` and `source.stale`. `capabilities.watch` is true. A later `watch`
  replaces the set of checkouts.
- The fake plan CLI gains `renumber OLD NEW` (renames the file, rewrites the citations, adds an id-map row).

## Success criteria

- The same fixture tree in the main checkout and in a `git worktree`, with one task `doing` in the worktree and `ready`
  in the main checkout: `list` for each returns its own state, with the same key, and the task carries the checkout and
  path. (Core turns the two into a divergence, `ts-0115-ug`.)
- A renumbered task is reported as `task.renamed` with the old and new short id and the checkout, and not as a removal
  and a new task. Its key follows.
- The same short id at two different paths in two checkouts is reported as a collision with both locations. One checkout
  alone never collides. The same task at the same path in two checkouts is not one.
- After `plan set` in a worktree, `task.changed` arrives within the debounce, with `cause: worktree`, that worktree as
  `checkout`, the old and new state and the field diff. A checkout of another branch or a merge that changes the file
  gives `cause: head`.
- A change the watcher did not see (the watcher is off in the test) is found by the 10 s recheck.
- Ten writes in 100 ms give one event for each task that changed, with the last state.
- A file that becomes unparseable gives `task.invalid` with the path, the checkout and the reason, and the task is no
  longer listed there. Fixing it gives `task.changed`.
- A checkout that disappears gives `source.stale` naming it, and the other checkouts keep working.
- A file deleted gives `task.removed` with `cause: deleted`, and that cause is `unknown` when the adapter cannot tell.
- `watch` answers at once. No startup event is sent: core reconciles at start (`ts-0116-wh`).
- The adapter holds no handle on a checkout that was dropped from a later `watch`.

## Tests

Write these first. Helpers make a temp git repository and a second checkout with `git worktree add`.

- The stage's two-checkout tests on the fixture `arch/` tree: a renumbered task (rename) and a same-id, different-path
  pair (collision).
- Divergence input: `list` per checkout with one status changed.
- The stage's watch test: `watch` emits `task.changed` after `plan set` in a worktree.
- `cause: head` after `git checkout` of a branch with another status.
- Debounce coalescing with an injected timer, and the recheck with the watcher off.
- Unparseable file and its repair; a vanished checkout; a deleted file.
- A second `watch` with fewer checkouts drops the rest.
- One end-to-end test with a real `node` child on stdio: `initialize`, `watch`, then a `plan set` run by the test, and
  the notification read from stdout.
- The fake CLI's `renumber` test.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4b: the `plan` adapter's bullets "reports renames from the id map and reports collisions" and "`watch`
    debounced, plus a 10 s recheck"; Tests: the adapter "in two checkouts" (rename, collision) and "`watch` emits
    `task.changed` after `plan set` in a worktree".
  - 0.10: Protocol (`watch` and its notifications); Identity (collisions are real; renames from the id map and git);
    Ref-scoped sources (a checkout's state, the filter, the divergence chips); the `plan` row of Sources (file watch on
    each checkout's `arch/` and HEAD, debounced, plus a 10 s recheck).
  - S11 (`sp-0008-bk-derived-point-coverage`): G7 (the shape of `task.changed`, `cause`), G8 (`task.invalid`).
- **Decision for the item, with a recommendation: what watches.** `node:fs.watch` with `recursive: true` on `arch/` and
  on the checkout's git HEAD file, and the 10 s recheck as the net (a watcher can miss events; that is why the plan has
  the recheck). No package such as `chokidar`, so there is no new dependency. Alternative: poll only, which means a full
  scan every few seconds for about 1,600 files (not measured here). The watcher is macOS-first (0.2); the recheck keeps
  it correct elsewhere.
- **Decision for the item, with a recommendation: debounce.** 300 ms, in one constant with the 10 s, both injectable for
  tests. The plan says "debounced" and fixes no number.
- **Decision for the item: collisions are hints.** An event is sent when a short id's set of locations changes and has
  two or more. Core recomputes collisions from each `list` (`ts-0115-ug`), so a resolved collision needs no event.
- **Decision for the item: HEAD.** `cause: head` when HEAD moved between two scans of a checkout, else `worktree`.
- **Decision for the item: where the id map is.** In the checkout's `arch/`, as the fixture tree has it. The real file's
  name and row format are the owner's sample (`ts-0110-ra`); until it exists the fixture's shape is invented.
- Out of scope: reporting a divergence (core's, `ts-0115-ug`); the line lock and the phase queue (`ts-0114-hr`); the
  daemon's use of the stream (`ts-0113-ck`, `ts-0116-wh`).
- Area: `fixtures/packs/orchard/providers/`, `fixtures/orchard-arch/plan`, `fixtures/arch-plan-tests/`.
- The test rule: no test touches the real home, real launchd or a real daemon. A test that runs the `collide` binary
  clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as
  `host/tests/common/mod.rs` `TempRoot` does. This item's tests are Node: each makes its own temp git repositories, sets
  `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_SYSTEM` to empty files and a temp `HOME`, and runs no daemon. Settings files are
  fixtures, never the owner's own `settings.json`. No test touches the owner's real worktrees.
- Stage 4b item (revision 23 scope, split 2026-11-17). Items of Stage 4b run one at a time, in id order, because they
  share `core`, `host` and the generated files.
- Nothing here runs a real `claude`.
- Owner decisions (2026-11-17, at the split): the recommendations above, on delegation.

### Built

Every Node test below runs against the fake plan CLI and temp git repositories (empty `GIT_CONFIG_GLOBAL` and
`GIT_CONFIG_SYSTEM`, a temp `HOME`). No Rust and no wire type changed, so `pnpm gen` had nothing to do.

- **The fake follows the owner's copy of the Orchard repo (2026-11-17).** Read only, never run; shapes only, every value
  invented. Taken from it: `renumber OLD NEW` with `--file`, `--force`, `--root`, `--format`, `--dry-run`; the JSON
  envelope keys and the report's keys; exit 0 done, exit 2 for a refusal, exit 1 only for the write gate; the id map
  as `ID-MAP.md` at the root of the folder, rows `| old id | new id | day | moved: <path>[; survivors: <paths>] |`,
  both ids with their slug; the file keeps its slug and its number moves; citations outside the generated region are
  rewritten; the epic tables are refreshed. Could not confirm (the copy was read, not run): the exact text of messages
  (invented), what the CLI does with a second `--force` on a loop of renumbers, the order of `survivors` when there are
  several, and whether exit 1 is ever used by `renumber` itself. The fake does not model the real `--incoming` flag,
  frozen spans, ids other than tasks, or the second gate.
- `fixtures/orchard-arch/plan`: `renumber` (`commandRenumber`, `markdownFiles`, `rewriteOutsideRegion`, `today`,
  `ID_MAP`); `regenerate` takes already-rewritten epic texts. `fixtures/orchard-arch/basic/arch/ID-MAP.md`: the real row
  shape, invented values.
- `fixtures/packs/orchard/providers/plan/`
  - `provider.ts`: `planHandlers` returns `PlanHandlers` (`stop` added). `checkoutFor` resolves `checkout`, default the
    root, and refuses a path that is not absolute or not a folder. `list`, `get`, `apply` and `preview` take it.
    `watch` calls `createWatcher(...).set(checkouts)` and returns `{}` at once. `servePlan` calls `stop` when the input
    ends.
  - `watcher.ts` (new): `createWatcher`, `DEBOUNCE_MS` (300), `RECHECK_MS` (10 000), `Timers`, `WatchFs`,
    `WatchOptions`. `node:fs.watch` on each checkout's `arch/` (recursive) and on the folder of its HEAD file (only
    `HEAD` counts, since git swaps it in by rename). A trailing debounce; a recheck interval; a baseline read before the
    reply; serialised flushes; a checkout that cannot be read is `source.stale` once until it reads again; watchers are
    reopened on a read when one failed or `arch/` came later.
  - `changes.ts` (new, pure): `compare` (changed, removed, invalid, renamed), `diffOf`, `idsChanged`, `collisionsOf`.
  - `id-map.ts` (new): `readIdMap`, `renamedTo` (follows a chain to its end, stops on a loop). `git.ts` (new):
    `gitHead`, `gitDir`, `gitRenames` (`git diff -M` under `arch/` between two HEADs). `files.ts`: `isFolder`.
  - `declaration.ts`: `capabilities.watch` is true.
- `fixtures/arch-plan-tests/test/`: `checkouts.test.ts` (13: list, get and write per checkout, the id map, git helpers),
  `watch.test.ts` (40), `fake-plan.test.ts` (`describe('renumber')`, 26), `fakes.ts` (fake clock, fake file watcher),
  `helpers.ts` (`notifications`, `pending`, `DEADLINE_MS` 20 s), `repo.ts` (`worktree`). `initialize.test.ts` and
  `list.test.ts`: two expectations updated (`watch: true`; a checkout that is not a folder is refused).
- **Seen red at runtime.** The 26 `renumber` tests of the fake: I did not record a red run for them, so treat them as
  not seen red. For the adapter I wrote the tests after the code, so I stubbed it afterwards: `checkoutFor` that
  ignores `checkout`, and a `watch` handler that does not start the watcher. Result: 42 of the 53 tests in
  `checkouts.test.ts` and `watch.test.ts` failed at runtime (all the checkout, change, cause, removed, invalid,
  rename, collision, recheck, stale, set and real-watcher tests); the 11 that passed are the id map, git helper and
  "sends nothing" tests. The stub was removed. That is a weaker check than red-before-code; say so at review.
- Real `fs.watch` is used in three tests, each waiting with a 20 s deadline; the debounce, the recheck and the
  closing of watchers use the injected clock and file watcher.

### Decisions for the owner

- **Renames are keys.** `task.renamed {from, to}` carries `plan:<short id>` for both, which is what `ts-0115-ug` and
  `ts-0116-wh` resolve ("the old key resolves to the new one"). Alternative: paths. Recommend keys.
- **Rename only on evidence.** From the id map, else from git's rename detection between the two HEADs. A file that went
  with another that came and no evidence is a `task.removed` and a new `task.changed`, never a guess. Alternative:
  guess from equal titles. Recommend no guess.
- **`task.removed` cause.** `deleted` when HEAD did not move, `unknown` when it did (the adapter does not read history).
  Recommend this until a use needs more.
- **Baseline before the reply.** `watch` reads each checkout before it answers, so a write right after the reply is a
  change, not part of the baseline. Cost: `watch` is as slow as one scan per new checkout. Recommend keeping it.
- **Collision locations.** `{checkout, path}` with the path from the checkout root, the checkout's absolute path as the
  checkout. Sent when the set of locations of a short id changes and has two or more; nothing for what exists at
  `watch` (core lists), nothing when it resolves.
- **A worktree's HEAD.** Its HEAD file sits in `.git/worktrees/<name>` of the main repository, so the adapter asks git
  (`--absolute-git-dir`) and watches that folder, not `<checkout>/.git`. Plan text for 0.10 Sources, `plan` row: "the
  folder of each checkout's own HEAD file, as git reports it".
- **Same short id twice in one checkout** is two `task.invalid` files, not a collision (a collision is across paths in
  different checkouts). Recommend.
- **`source.stale` has only a reason**, so the checkout is named in the text. If core should act per checkout, the wire
  type needs a `checkout` field; that is a change to `core/src/provider.rs`, for `ts-0113-ck` to ask for.

- Owner decisions (2026-11-17, at the build): every decision above as recommended, taken on delegation; no plan text
  (0.10's `plan` row says HEAD, which still holds). At review: the id map's header and slugged cells were checked
  against the plan CLI's `appendIdMap` in the copy, which writes `| old id | new id | renamed | detail |` for a new file
  (older rows there are bare). One fix, seen red first: `compare` took a gone task's git rename as matching a new task
  with no path when git reported no rename for it; it now needs git's rename (test `with no rename from git for the gone
  file, a new task without a path is not taken for its new name`). The `renumber` tests had no red run, so three
  mutations of the fake were tried (no citation rewrite, bare ids in the row, no --force check): each was caught (4, 2
  and 1 tests failed). `source.stale` with a `checkout` field is carried to `ts-0116-wh`, which watches sources.

### Not done, by design

- Divergence reporting and collision chips: `ts-0115-ug`. The daemon's use of the notifications, `task.get`, `apply` and
  `preview` through the daemon, and sources: `ts-0113-ck`. Reconcile at start: `ts-0116-wh`.
- The line lock, root busy and the phase queue: `ts-0114-hr`. The adapter does not know of them.
- `watch` on a platform without recursive `fs.watch`: the recheck is the only net there; not tested beyond the
  `noFs` case.
- The real plan CLI's `--incoming` and other id kinds in the fake's `renumber`.
