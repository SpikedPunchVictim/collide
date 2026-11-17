---
id: ts-0115-ug
kind: task
status: ready
title: "Corpus core: categories, identity, collisions, invalid items and links"
owners: []
links:
  - { type: blocked-by, to: ts-0114-hr }
---

## Goal

The corpus as a pure `core` module: it takes what each source's `list` returned for each checkout, and answers what the
board shows. One task for each key, its copies by checkout, its divergence, its collisions, its links one hop out, and
the items that could not be read. It does no I/O, so the daemon (`ts-0116-wh`) and the tests feed it data.

What exists. `core/src/provider.rs` has the protocol types (`Task`, `ListResult`, `TaskInvalid`, `TaskCollision`,
`SourceStale`, `TaskChanged`, `Rename`). `core/src/domain.rs` has `CachedTask` (a record, not a store).
`core/src/hook.rs` has `HookLink`, the shape of a link one hop out. `core/src/owner/task.rs` has `TaskCopy`,
`TaskGetResult` and `TaskListResult`. `core/src/short_id.rs` has `resolve` for short ids and suffixes.
`core/src/ready.rs` has the ready rule. `core/src/testing/provider.rs` has `FakeProvider`. Nothing merges sources, maps
a status to a category, or finds a divergence, a collision or a link.

What this adds, in `core/src/corpus.rs` (or a module of that name):
- The input: a source's declared statuses (`InitializeResult`), its `list` result for each checkout, the kind of each
  checkout (a line root, a workspace forked from a root, or other), and whether the source is stale.
- Identity: a task's key is `source:shortId`. Within a checkout a task is (shortId, path). Aliases and long ids find a
  key.
- The category of each task from its source's declared statuses. A task whose status the source did not declare is not
  listed as a task: it joins the source's invalid items, with the status named.
- Copies by checkout, and which copy the board shows: the default line's root, else the first checkout, marked branch
  only.
- Divergence: a workspace's copy that differs from the copy at the root it was forked from. Other lines' states are
  copies, not divergence (0.10).
- Collisions: one short id at different paths in different checkouts. A collision is never a divergence, and the task is
  ambiguous: a lookup says so.
- Links one hop out, both directions, across sources, as `HookLink` (`type`, `dir`, `key`, `kind`, `status`, `category`,
  `stale`), from the corpus alone.
- Incremental updates from the notifications (`task.changed`, `task.renamed`, `task.removed`, `task.invalid`,
  `source.stale`), and `reconcile`: the diff of a cached corpus against fresh lists as the `task.changed` events with
  `cause: startup` that core emits at start.

## Success criteria

- A task `doing` in a workspace and `ready` at the root it was forked from: the corpus shows the root's state and
  reports a divergence that names the checkout and both states (the stage's first criterion, in fixture form).
- The same short id at two paths in two checkouts is a collision with both locations, and no divergence. The same path
  in two checkouts with two states is a divergence and no collision. A lookup of a collided id says it is ambiguous.
- A status is mapped to its category from the source's declared list, in the source's order. A status the source did not
  declare (the `list` gave it, or a `task.changed` brings it) makes the item an invalid entry with the status named in
  the reason, and the task is not listed. The item counts with the source's other invalid items.
- Links resolve one hop in both directions across two sources: each entry has the other end's kind, status and category,
  and `stale` is true when the other end's source is stale. A link to a key that is not in the corpus is reported as
  missing, never dropped. See Notes.
- `reconcile` of an unchanged source gives no event. A changed task gives one `task.changed` with `cause: startup` and
  the old and new state; a new task gives one with `from` null; a task that is gone gives one `task.removed` with cause
  `unknown`.
- A `task.renamed` moves the key, and the old key resolves to the new one.
- A `task.invalid` adds the item to the invalid list and removes the task of that path from the listing; a later
  `task.changed` for the path brings it back.
- The corpus holds no body text beyond what a source listed, and applying the same notification twice is the same as
  once.

## Tests

Write these first, over `FakeProvider` data in `core/tests/`.

- Divergence: root and workspace copies; two workspaces; a workspace of another line's root (a copy, no divergence).
- Collision against divergence, and the ambiguous lookup.
- Categories, in the source's order; an undeclared status in a `list` and in a notification.
- Links: out and in, across two sources, a stale source, a missing target, a link whose target is a feature (not
  resolved here).
- `reconcile`: unchanged, changed, new, gone; the events' shape against `TaskChanged`.
- Rename, and an invalid item that is then fixed.
- Idempotence of each notification.
- The corpus's output serialises to `TaskListResult` and `TaskGetResult` (the wire types that already exist).

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4b: Corpus (status categories, identity and collisions; "A task whose status its source did not declare is
    listed with the source's invalid items, with the status named"; links one hop out; the startup reconcile; an
    unparseable item emits `task.invalid` and counts on its view).
  - Success Criteria: "A task shows `doing` in one worktree and `ready` at the board root, with the divergence
    reported."
  - 0.10: Identity; Status categories; Ref-scoped sources (divergence chips compare a line's root with the workspaces
    forked from it). 0.5: `tasks[].links`. S11 (`sp-0008-bk-derived-point-coverage`): G5, G7, G8.
- **Owner's choice, with a recommendation: a link to something the corpus does not hold.** The hook payload carries
  `kind`, `status` and `category` for every link, and a gate such as "close only when every evidence item is done" would
  pass if a missing target were left out. Recommend adding `missing: bool` to the link entry (additive, default false;
  `kind`, `status` empty, `category` `backlog`), so a gate can refuse it. Alternative: drop it from the list, which is
  smaller and lets that gate pass. This changes the frozen hook payload (0.5), so it is the owner's.
- **Decision for the item, with a recommendation: what a link's target means.** A `to` with a `:` is a key. Any other
  `to` is a short id, an id or an alias in the same source as the item that holds the link. A link whose type targets a
  feature (`LinkTarget::Feature`) is not resolved here; features belong to the planning stage (4c).
- **Decision for the item, with a recommendation: `reconcile` emits only for what differs.** The plan says one change
  per item. An identical item gives no event, since a change event that changed nothing would light a chip for every
  task at every start. Alternative: one event for every item. Not recommended.
- **Decision for the item: the shown copy.** The copy at the default line's root; if there is none, the copy of the
  first checkout in path order, marked `branch only` (0.10, Planning sources). The drawer lists all copies.
- **Decision for the item: scale.** The structure indexes by key and by checkout so that one notification costs one
  task, not a pass over about 1,600. Say how in a comment.
- Out of scope: the store cache and the events' persistence (`ts-0116-wh`); views (`ts-0118-wp`); the `latest_run` and
  runs of `task.get` (`ts-0116-wh`).
- Area: `core/src/corpus.rs`, `core/src/lib.rs`, `core/src/hook.rs` (only `missing`, if chosen) and the regenerated
  schemas, `core/tests/`.
- The test rule: no test touches the real home, real launchd or a real daemon. A test that runs the `collide` binary
  clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as
  `host/tests/common/mod.rs` `TempRoot` does. This item's tests are pure `core` tests and need neither. Settings files
  are fixtures, never the owner's own `settings.json`.
- Stage 4b item (revision 23 scope, split 2026-11-17). Items of Stage 4b run one at a time, in id order, because they
  share `core`, `host` and the generated files.
- Nothing here runs a real `claude`.
- Owner decisions (2026-11-17, at the split): the recommendations above, on delegation: an additive `missing: bool` on a
  hook's link entry.
