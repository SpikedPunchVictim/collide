---
id: ts-0112-px
kind: task
status: ready
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
- Area: `fixtures/packs/orchard/providers/`, `fixtures/orchard-arch/plan`, `packs/orchard-fixture-tests/`.
- The test rule: no test touches the real home, real launchd or a real daemon. A test that runs the `collide` binary
  clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as
  `host/tests/common/mod.rs` `TempRoot` does. This item's tests are Node: each makes its own temp git repositories, sets
  `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_SYSTEM` to empty files and a temp `HOME`, and runs no daemon. Settings files are
  fixtures, never the owner's own `settings.json`. No test touches the owner's real worktrees.
- Stage 4b item (revision 23 scope, split 2026-11-17). Items of Stage 4b run one at a time, in id order, because they
  share `core`, `host` and the generated files.
- Nothing here runs a real `claude`.
- Owner decisions (2026-11-17, at the split): the recommendations above, on delegation.
