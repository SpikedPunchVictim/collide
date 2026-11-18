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

### Built

A pure `core` module. No I/O, no daemon, no store, no new crate or package. `pnpm gen` ran and a second run left no
diff.

- `core/src/hook.rs`: `HookLink.missing: bool` (`#[serde(default)]`, always written). `schemas/HookInput.*.json` and
  `packages/pack-sdk/src/generated/HookInput.*.ts` are regenerated. Existing fixtures that build a link
  (`core/tests/hook_protocol.rs`, `host/tests/schema_command.rs`, `packages/pack-sdk/test/hook-types.test.ts`) gain the
  key.
- `core/src/corpus.rs` (new, in `core/src/lib.rs`).
  - Input: `Corpus::set_checkouts` (`CheckoutInfo`, `CheckoutKind::{Line { default }, Workspace { forked_from },
    Other}`), `declare_source` (an `InitializeResult`), `load(source, checkout, ListResult)` (replaces one checkout's
    list; `None` for a source that is not ref-scoped).
  - Notifications, one method each, each touching one entry: `apply_changed` (returns `Applied::{Done, NeedsTask}`),
    `apply_renamed`, `apply_removed`, `apply_invalid`, `mark_stale` and `mark_fresh`. An undeclared source is
    `CorpusError::UnknownSource`.
  - Answers: `rows` (`Row`: shown task, `branch_only`, `divergence`, `collided`, `stale`), `divergences` (`Divergence`),
    `collisions`, `invalid` (`Invalid`, with the source), `statuses`, `lookup` (`Found::{One, Collided, Ambiguous,
    None}`), `get` (a `TaskGetResult` with no runs), `list_result` (a `TaskListResult`, no body), `links_of`
    (`Vec<HookLink>`), `stale_sources`.
  - `reconcile(cached, fresh)` returns `ProviderEvent`s (`Changed` with `cause: startup`, `Removed` with `unknown`,
    `Invalid`).
  - Scale: indexes by key, by name, by link target and by (source, checkout, path). An entry records what it indexed,
    so a notification reindexes that entry only. The module comment says so.
- `core/src/short_id.rs`: `matching_ids`, for the number forms (`12`, `12-ua`) over bare ids. `resolve` is unchanged.
- Tests. `core/tests/corpus.rs` (58 tests), `core/tests/hook_protocol.rs` (2 new, 4 updated),
  `core/tests/short_ids.rs` (1 new).
- **Seen red at runtime.**
  - `missing`: with the field stubbed `#[serde(skip)]`, 6 tests of `hook_protocol.rs` failed (the link entry, the two
    new tests and the round trips).
  - Corpus: with every method stubbed to answer empty, 51 of the first 56 tests failed at runtime. Five passed against
    the stub:
    - `a_workspace_that_agrees_with_its_root_is_not_divergent`
    - `reconcile_of_an_unchanged_source_gives_no_event`
    - `reconcile_says_nothing_of_what_it_was_not_given_a_list_for`
    - `a_link_whose_type_targets_a_feature_is_not_resolved_here`
    - `a_lookup_of_nothing_is_an_answer_with_no_task`

    Each of the first four was then mutation-checked and failed. The fifth was not seen red.
  - Then 29 mutations of `corpus.rs`, each put back after. Misses led to tests: a rename whose old key resolves only
    through the redirect, a shown copy where the default root is not first in path order, and a moved copy that must
    not answer for its old path; all three now fail under their mutation. Caught by the first set: divergence rule
    (equality, path), feature links, reconcile (unchanged, listed, stale, invalid), category, collision rule, `missing`,
    rename (checkout), invalid (removal, clearing), branch only, link stale, stale clear, body, duplicate in a
    checkout, `NeedsTask`, ambiguous names, inbound index.
  - Not seen red: `matching_ids_reads_a_short_form_over_bare_ids` (a new function; the corpus tests that use it did go
    red under the ambiguous-names mutation).

### Decisions for the owner

- **`missing` is always written** (`"missing": false` too), as `stale` is. Alternative: leave it out when false, so
  old payloads stay byte-identical. Recommend always: a gate reads `link.missing` without a default.
- **A key has one copy for each checkout.** A task listed twice in one checkout is the first and an invalid item for the
  later one (the stance of `ts-0112-px`). Alternative: keep both and call it a collision. Recommend as built.
- **A collided key has one row for each path, no one `task`.** `get` of it gives `task` absent, `ambiguous: [key]`,
  `collisions` and every copy. Alternative: show the first path. Recommend as built: nothing is picked for the user.
- **Divergence compares state** (status and category) of the same path. A version or title difference is not one; the
  drawer's copies carry the version. Every workspace is compared with its own root, including a workspace of another
  line, and the caller (the board, the selected line) filters. Recommend.
- **The category is the declared one**, from the task's kind when the source declared the kind, else from the source's
  statuses. It replaces the category the provider put in `state`. A status the kind lacks but the source declares is
  accepted. Alternative: refuse it. Recommend lenient until a pack has kinds whose statuses clash.
- **A change brings only a state.** `apply_changed` updates a held copy from the event; a new copy returns `NeedsTask`
  and the daemon fetches that one task and applies the event again with it. A change that makes the status undeclared
  drops the copy, so the fix needs the task again. Alternative: keep the dropped copy. Recommend as built.
- **A rename in one checkout moves that copy only.** The old key resolves to the new one when it holds nothing. The
  copy keeps its `id` until its source lists it again, so `ts-0116-wh` should `get` it after a rename.
- **`reconcile` also gives `task.invalid`** for an item that is new or has a new reason (the item's list names only
  changed and removed), and gives no removal for a task that became invalid. It says nothing of a checkout it was not
  given a list for or a source that is stale. Its `diff` names status, title, path, kind, labels, owners, collections
  and each field. Recommend.
- **A bare link target that fits several tasks of its source is `missing`**, so a gate refuses it. A link of a type that
  targets a feature is left out, since `HookLink` cannot say it (planning, 4c).
- **Links read the shown copy**; for a collided key, the first path's.
- **Stale is explicit**: `mark_stale` and `mark_fresh`; `load` does not clear it. The daemon clears it after a full
  read.
- **Collections** of a source are those its shown checkout listed (default root, else first).
- **`task.collision` is not consumed.** Collisions come from the lists, as `ts-0116-wh` says.
- Plan text, 0.5 `tasks[].links`: "Each entry also has `missing`: true when the corpus holds no task with that key
  (`kind` and `status` are then empty and `category` is `backlog`)."

- Owner decisions (2026-11-17, at the build): every recommendation above, on delegation. The 0.5 plan text is applied to
  the `tasks[]` links entry in the hook payload, and it also says that a bare id naming more than one task is `missing`.
  At review, two mutations were caught: a collision counted by copies instead of paths (3 tests failed, among them
  `the_same_path_in_two_checkouts_with_two_states_is_a_divergence_and_no_collision`) and a link's `stale` forced false
  (`a_link_to_a_source_that_is_stale_is_marked_stale`).

### Not done, by design

- The store cache, its migration, the events' persistence, the notification pump (which fetches for `NeedsTask`,
  clears stale, and relists after a restart) and the checkout list from `git worktree list`: `ts-0116-wh`.
- `task.list`'s additive fields (`divergent`, `collisions`, `stale_sources`, `refreshing`), `runs` and `latest_run`
  on `task.get`, and a Queue item for `task.invalid`: `ts-0116-wh`.
- Views, fields and the closure: `ts-0118-wp`. It reads `rows`, `links_of` (`missing` and `category`) and `invalid`.
- Feature links: 4c. The time to build a corpus of 1,600 tasks: `ts-0116-wh` records it.
