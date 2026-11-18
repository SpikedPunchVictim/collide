---
id: ts-0125-uh
kind: task
status: ready
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
