---
id: ts-0125-uh
kind: task
status: done
title: "Plan pack: the private root"
owners: []
links:
  - { type: blocked-by, to: ts-0124-xm }
---

## Goal

The markdown provider has a second root, private to the user and board, that is never in git. Items in it list, link and
change like the shared ones.

What exists. The provider reads one root (`ts-0124-xm` makes it the `layout` root). The daemon starts the built-in
`collide/plan` provider with `COLLIDE_PACK_DATA` set to `<daemon data>/pack-data/<board>/<pack>` (`host/src/sources.rs`
`with_pack_data`, `host/src/board_load.rs` `pack_data_dir`, `host/src/builtin_packs.rs`). The direct CLI starts the
provider with no board, so it has no pack data folder. `packs/plan/src/lock.ts` takes a lock per root.

What this adds.
- When `COLLIDE_PACK_DATA` is an absolute folder, the private root is `<it>/planning/`, with the same layout as the
  shared root (`epics/<epic>/...`, `spikes/`). `list` for the main checkout reads both roots. A call that names another
  checkout lists only the shared root, so a private item is not a copy in every worktree.
- A private item reports `fields.root = "private"` and a `path` that starts with `private:`, so it is never taken for a
  file of the checkout. Its `version` is the content hash, as for a shared item.
- `create` takes `root`, `shared` or `private`. The default is `shared` (see Notes). `get`, `setStatus`, `claim`,
  `unclaim`, `link` and `reid` find the item by id in either root and write where it is, under that root's lock.
- A write to a private item returns no `files`, because nothing in it is committed. `--commit` on it is refused with
  that reason.
- Ids are taken across both roots and every ref. A private id is never looked for in refs, and never leaves the
  machine.
- A private epic folder is named by the epic's id alone (`epics/ep-004-qd/proposals/...`), so `part-of` comes from the
  path as in the shared root, and the epic itself may be shared.
- With no `COLLIDE_PACK_DATA`, `root: private` is refused with the reason and listing is as before.

## Success criteria

- A private proposal created for an epic lists, with `part-of` the epic, and `git status --porcelain` in the repository
  is empty after the create, a `setStatus`, a `link` and a `claim` on it.
- The file is under `<pack data>/planning/`; a `list` that names a worktree checkout does not show it.
- A private and a shared create get different numbers, and a forced collision across the two roots is redrawn.
- A private item that links to a shared one (`blocked-by`) and back shows both links through the daemon's corpus.
- `--commit` on a private write is refused, and no file of the repository changes.
- Without `COLLIDE_PACK_DATA`, `root: private` is refused, and a repository with no private root lists as before.
- Two files with one id, one in each root, are a `task.invalid` for the second, as the duplicate rule says.

## Tests

Write these first.

- Provider tests with a repository and a data folder in two sibling temp folders: create, list, write, link and claim
  in each root, `git status` after each, ids across roots, the duplicate across roots, a named checkout.
- A host test through the daemon (temp `HOME` and `COLLIDE_DAEMON_ROOT`): `task.list` shows a private item and its
  `part-of`, which shows that the daemon's pack data folder reaches the built-in provider.
- The existing provider tests unedited.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4c: Planning ("the private root"); Success Criteria ("a private-root item never appears in `git status`").
  - 0.10: Planning sources ("two roots: a private one in the pack's data directory, and a shared one"). 0.7: Pack data.
  - `docs/research/planning.md` 2.4 (the two roots table).
- **Owner's choice, with a recommendation: the root a `create` uses when none is named.** Recommend `shared`, as today,
  and `root=private` to opt in. The dogfood board and every existing command behave as before. The Plan… session of
  Stage 9d names `private` itself. Alternative: `private` by default, which suits one person planning alone and stops a
  stray commit, but makes `collide task create` on this repository write where `git` cannot see it. This sets where a
  new item goes.
- **Decision for the item: how a private item shows.** `path` with the `private:` prefix and `fields.root`, listed only
  for the main checkout. Alternative: a named checkout for the data folder. No: it is not a checkout.
- **Decision for the item: Share and Move.** Share (a copy from one root to the other) and `record` of the files are
  Stage 12. Until then a person copies a file by hand.
- **Decision for the item: the direct CLI.** It has no pack data folder, so it does not see the private root until
  `ts-0136-rr` moves it behind the daemon.
- Area: `packs/plan/src/scan.ts`, `create.ts`, `write.ts`, `ids.ts`, `reid.ts`, `lock.ts`, `provider.ts`,
  `packs/plan/test/`, `host/tests/`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test that runs the `collide` binary
  clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as
  `host/tests/common/mod.rs` `TempRoot` does. Settings files are fixtures, never the owner's own `settings.json`. A temp
  repository sets `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_SYSTEM` to empty files.
- Stage 4c item (revision 23 scope, split 2026-11-18). Items of Stage 4c run one at a time, in id order, because they
  share `core`, `host`, `packs/plan` and the generated files.
- Nothing here runs a real `claude`. No dependency is added.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers at the split.

### Built

The provider has a second root, the private one, on the seams the `layout` setting made. With
`COLLIDE_PACK_DATA` an absolute folder, the private root is `<it>/planning/`, read after the shared roots.

- `packs/plan/src/provider.ts`: `PlanProviderOptions.packData` and `ServeOptions.packData` (default
  `COLLIDE_PACK_DATA`; `null` is none; a relative path is none). `privateRootOf` gives `<packData>/planning`. `scan`
  reads it for the main checkout only: a call with no `checkout`, or one that names the main checkout. The daemon names
  the main checkout by path like any other, so `isMainCheckout` (`checkout.ts`) compares real paths. The `create` op
  schema has `root`, `shared` or `private`.
- `packs/plan/src/scan.ts`: `readEpicFiles` and `scanEpics` take `privateDir`. `FoundItem` has `root` and `absolute`.
  `pathOf` gives `private:epics/...` for a private file, and `PRIVATE_PREFIX` is the one place the prefix is spelled.
  `toTask` adds `fields.root = "private"` to a private item and nothing to a shared one. A private file that does not
  read is `invalid` with a `private:` path. Shared roots are scanned first, so a duplicate id puts the private file
  second, and it is the `task.invalid` one.
- `packs/plan/src/ids.ts`: `takenIds` takes `privateDir` and reads it from disk only. Refs are asked about the
  shared epics folders and never about the private one. The `look` hook gets `privateDir` as a third argument.
- `packs/plan/src/create.ts`: `checkCreate` checks `root`. `createLocked` writes to the root named, returns no `files`
  for a private item, and names a private epic folder by the epic's id alone. A private task or proposal may be for an
  epic that is shared (its folder is then made in the private root) or private.
- `packs/plan/src/write.ts`: `applyOp` takes `packPlanning`. A write to one item locks the root the item is in, found
  by one scan before the lock, and `writeLocked` refuses if the item is in the other root when it gets the lock.
  `reid` locks each root that exists, shared first. `create` locks the root it writes to. A private write returns no
  `files`, makes no file or folder in the repository, and a named checkout other than the main one neither sees nor
  creates a private item. `root=private` with no pack data is refused naming `COLLIDE_PACK_DATA`.
- `packs/plan/src/reid.ts`: reads both roots, renames a private file where it is, and leaves private paths out of
  `files`.
- `host/src/task_cli.rs` `commit_reported`: `--commit` on a result whose `path` starts with `private:` prints
  "--commit refused: ... is in the private root" and exits 1. The write stands, as when a commit fails.
- `host/tests/common/plan_rig.rs` (new): the rig of `daemon_plan_layout.rs` moved here, unchanged but for
  `rig(bundle_name)`, so two test files use it.

Tests, by title:
- `packs/plan/test/private-root.test.ts` (25): a private item (created under the data folder with `part-of` and
  `fields.root`; no root field on a shared item; setStatus, link, claim and unclaim write no repository file and report
  none; a repository with no planning folder stays bare; a private epic's folder; an epic that is nowhere is refused;
  links both ways between a private and a shared item; four creates at once), the lock (private writes wait on the
  private lock only, shared writes on the shared lock only, a reid holds the private lock too), the root a create names
  (default shared, bad value, no pack data, a non-main checkout), no private root lists as before, a named checkout,
  ids across the two roots (different numbers, `takenIds`, a forced collision redrawn in both directions), a duplicate
  id across roots, and reid with a private item.
- `host/tests/daemon_plan_private.rs` (3):
  `a_private_item_lists_with_its_epic_and_a_write_to_it_leaves_git_status_empty`,
  `a_private_item_and_a_shared_one_that_link_to_each_other_show_both_links_in_the_corpus`,
  `commit_on_a_write_to_a_private_item_is_refused_and_changes_nothing_in_the_repository`. They run the real built plan
  pack through the daemon, with the data folder under the test's own root.
- The existing `packs/plan` tests are unedited (`test/helpers.ts` gained an optional `connect({ packData })`).

Red at runtime: before any change to `src/`, 20 of the 22 tests then written failed (the other 2 are the "no root
field" and "lists as before" cases). After the code, each of these was broken in turn and the named tests went red:
named checkout still lists private (1), a private write reports its file (1), `takenIds` ignores the private root (4),
`reid` reports private files (2), a private write locks the shared root (3), the `private:` prefix dropped (6), private
create takes no lock (2), `reid` skips the private lock (1). Each was put back and the file passed. For the host tests,
`privateRootOf` was stubbed to return null (the old behaviour): all 3 failed. The `--commit` test was seen red on its
own before the `commit_reported` change (exit 0, not 1).

`cargo test --workspace` passed (4427 tests, 0 failed) in a run after my last edit.
`ready_and_kind_apply_on_top_of_a_view` in `task_daemon_cli.rs` failed once while I ran
`pnpm test` beside it, and passed alone and in the full run.

### Decisions for the owner

1. The default root of `create` (taken as the item recommends, on the owner's delegation): `shared`, and `root=private`
   opts in. Alternative: `private` by default. Recommend as built.
2. `--commit` on a private write is refused after the write, in `commit_reported`: the write is made, nothing is
   committed, exit 1. Alternative: refuse before the write in each caller. Recommend as built: the write
   touches nothing in the repository, and one place holds the rule.
3. A write to a private item takes only the private root's lock, so it never makes a file in the repository. `create`
   takes the lock of the root it writes to only, so a shared and a private create at the same moment can both pick the
   next number. They still draw different suffixes and each draw is checked just before the write, as for a ref that
   arrives late. Alternative: both locks on every create, which makes a lock file in a repository that has no planning
   folder. Recommend as built.
4. A private folder is named by the epic's id alone for every private epic, also one made with `root=private`
   (`epics/ep-002-xx/epic.md`). A private task for a shared epic needs that epic's folder in the current shared root; an
   epic that only a `previous` root holds is refused with where it is, as for a shared create.
5. A create with `root=private` and a `checkout` that is not the main one is refused. Alternative: ignore `checkout`.
   Recommend as built: a private item is not in a checkout.
6. `fields.root` is set for a private item only. Alternative: `shared` on every shared item, which changes every
   existing listing. Recommend as built.
7. `reid` of a private item rewrites the links in shared files that name it, and says so in `files` (those files only).
8. The help text of `collide task` is not changed: the direct CLI has no pack data folder and refuses `root=private`
   with the reason, until `ts-0136-rr`.

- Owner decisions (2026-11-18, at the build): `create` with no `root` uses the shared root, as the item recommends, on
  the owner's delegation; the rest as built. The `reid` of a shared epic that leaves a private folder under the old id
  is carried to `ts-0127-js`, which owns `reid` over every kind.

### Not done, by design

- `reid` of a shared epic does not rename a private folder named by the old epic id, so the `part-of` of the private
  items in it goes stale. It needs the epic's rename to reach the other root. No item names it yet; propose adding it
  to `ts-0136-rr` or a small item of its own.
- Share and Move between roots, and `record` of the files: Stage 12.
- The direct CLI sees no private root until `ts-0136-rr` moves it behind the daemon.
