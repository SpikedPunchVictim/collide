---
id: ts-0101-bd
kind: task
status: done
title: "User packs, working copies and collide pack"
owners: []
links:
  - { type: blocked-by, to: ts-0100-pu }
---

## Goal

The packs that belong to one user rather than to the repo, and the commands that manage them (0.7, Repo packs and user
packs; Working copies; Switching packs). `pack update` is `ts-0100-pu`'s.

- **User packs.** `~/.config/collide/packs.yaml` (`UserPacks`) and its lock, `~/.config/collide/packs.lock`, from `git:`
  and `dev:` sources, available on every board and switched on per board in the user's own board folder
  (`~/.config/collide/boards/<board-id>/`, the folder of `ts-0098-nx`; a `packs.yaml` there lists the names switched on
  and the working copies linked). The load order is the board's packs, then the user's that are on.
- **`collide pack add <source>`.** Resolves the source (a `git:` fetch under `ts-0100-pu`'s rules; a `dev:` folder's
  tree id by `tree_of_folder`), materializes it, reads its manifest for the name, and shows the person what they would
  trust: `pack_review` (source, commit, the hooks, providers and observers it runs, its `requires`). Only with the
  person's yes does it write `packs.yaml` and the lock, and record trust at the tree id (`ts-0097-kw`). Through the
  wire, `config.pack_add` takes `confirm` (default false): without it the result carries the review and nothing is
  written.
- **The name clash.** A user pack named like a repo pack is a load error that names both sources, so the team's gates
  and reviewers stay the team's. A user pack named like a built-in is refused too.
- **`dev:` rules.** `board.yaml` refuses `dev:` (done). A `dev:` folder inside a Collide workspace is refused, at `pack
  add` and at load: the check takes the worktree root and the store's recorded workspace paths as arguments, and refuses
  a folder whose real path is under either. A `dev:` pack is hashed at every load, its tree id recorded by the caller,
  and it is badged (`PackEntry.dev`). Experiments refusing it is a later stage.
- **`collide pack link <name> [<folder>]`.** Overrides a repo pack with a `dev:` folder for one user on one board: the
  link is in the board's user folder. With no folder, the folder is the pack's own in the user's checkout, the default
  line's root (`.collide/packs/<name>`, resolved through the pointer file if there is one). The override is badged. A
  folder that is not a pack, is inside a workspace, or whose manifest name differs from `<name>` is refused.
- **Working copies.** A new folder under `.collide/packs/` in the user's checkout that `board.yaml` does not list is
  offered by `collide pack list` as a working copy (a new `PackOrigin`, `offered`), with no `board.yaml` edit; linking
  it makes it a `dev:` pack. A repo pack whose folder in that checkout differs from the line's ref (a different tree id
  from `tree_of_folder`) is marked, so `pack link` can be offered. An integrate that changed a working copy makes the
  next load wait for an acknowledgement that shows the diff (0.9): the load returns it as a problem naming the diff, and
  the acknowledgement is recorded in `trust.yaml` as the tree id seen.
- **`collide pack enable|disable <name>`.** For a user pack, per board. A repo pack is refused: it switches through
  `board.yaml`.
- **`collide pack list`.** The board's packs, then the user's, each with source, origin, enabled, and a `dev` mark.
  Serves `config.pack_list`, `config.pack_add`, `config.pack_link` and `config.pack_enable` (the wire types exist; `add`
  gains `confirm` and `PackEntry` gains `review`, so `pnpm gen` runs). The `cli: pack` stub is filled.

## Success criteria

- A user pack named like a repo pack is refused at load, naming both sources. A user pack named `collide` is refused.
- `collide pack link <name>` overrides a repo pack for that user only, on that board only: another user's load, and the
  same user's load on another board, still use the repo's pack. `collide pack list` marks it dev.
- A `dev:` folder inside a workspace is refused, at `add` and at load, naming the folder and the workspace.
- `collide pack link <name>` with no folder runs the uncommitted pack folder from the user's checkout, badged dev: an
  edit to its `pack.yaml` in the working tree is read, where the same edit in a repo pack's line snapshot is not.
- A new `.collide/packs/` folder in the user's checkout is listed as a working copy, and appears nowhere else until
  linked.
- A repo pack whose working folder differs from the ref is marked as having a working copy to use.
- `pack add` of a `git:` source on a fixture remote: without `confirm` the review is shown (source, commit, hooks,
  providers, observers, `requires`) and no file is written; with it, `packs.yaml`, the lock and `trust.yaml` are
  written. A second add of the same source changes nothing and says so.
- The user's pack is trusted at its tree id (`ts-0097-kw`): `pack update` to a new tree id asks again.
- `enable` and `disable` change the board's folder only. On a repo pack they are refused, naming `board.yaml`.
- The load order is the board's packs, then the user's enabled packs, and a `dev:` link replaces the repo pack in place.
- An integrate that changes a linked working copy makes the next load report the acknowledgement with the diff, and
  recording it clears the problem for that tree id only.
- A `dev:` tree id equals `git write-tree` over the same files (covered by `ts-0094-av`, asserted again here through
  the loader).
- No new crate.

## Tests

Write these first, in temp roots, with a fixture remote and a fixture checkout:

- user packs: add (preview and confirm), a second add, a `dev:` add, a bad manifest, a name clash with a repo pack and
  with a built-in, the load order;
- link: with a folder, with none, a folder not a pack, a folder in a workspace, a name that differs, per-board and
  per-user scope (two users are two `HOME`s; two boards are two folders);
- working copies: the offered list, the differing repo pack, an integrate that changes the linked folder and the
  acknowledgement with its diff;
- enable and disable, and the refusal on a repo pack;
- `pack list` output: origin, enabled, dev mark, offered;
- the CLI and the daemon over a temp root: each subcommand's exit code and message, and the wire types' round trip;
- `pnpm gen` and `pnpm check` with the stub filled.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4a: Pack sources and scope (user packs; trust per source; working copies; `collide pack
    add|link|enable|disable| list`); Success Criteria (the name clash; `pack link` and `pack list` marks dev; a `dev:`
    folder in a workspace; `pack link` with no folder; a new `.collide/packs/` folder as a working copy).
  - 0.7: Repo packs and user packs; Working copies; Switching packs. 0.9: Trust, per pack source (the acknowledgement
    for a working copy that an integrate changed).
  - 0.4: the worktree root (what "a workspace" is for the `dev:` refusal).
- Carried from `ts-0026-es`: whether a user profile's `packs` are user packs is `ts-0098-nx`'s check, and uses the
  user packs this item loads.
- **Decision for the item, with a recommendation: what "inside a workspace" means.** The plan says a `dev:` folder
  inside a Collide workspace is refused, because agents could edit their own pack. Recommend the check refuses a real
  path under the worktree root or equal to or under a workspace path the store holds. It does not look at git. Both are
  inputs to the function, so tests give temp paths. Alternative: also refuse any folder whose git directory is a Collide
  worktree's; it covers a hand-made worktree, which is the user's own checkout and not an agent's.
- **Decision for the item, with a recommendation: the working-copy acknowledgement.** The plan says the next run waits
  "in the Queue". the Queue item for it is a later stage's, so this item returns the acknowledgement as a load problem
  with its diff and records it in `trust.yaml`, and the stage that has the Queue item puts it in front of the person.
  Alternative: build a Queue item here, which pulls the Queue's types into 4a. Recommend the problem.
- **Decision for the item:** `PackOrigin` gains `offered` for a working copy that is not linked; `PackEntry` gains an
  optional `review` (the review text's data), used by `add`.
- **Decision for the item:** `packs.yaml` in the board's folder holds `{ on: [names], links: { name: folder } }`; the
  global `packs.yaml` holds the sources. Both are Collide-owned and written by `yaml_out`.
- `collide pack test` (`tsc --noEmit`) is a later stage (the second-pack stage), not here.
- Area: `host/src/user_packs.rs` (new), `host/src/board_load.rs` (the user's packs, the order, the clash),
  `host/src/daemon.rs` (`config.pack_*`), `host/src/commands.rs`, `core/src/owner/config.rs`, `core/src/lock.rs`,
  `stub-tags.json`, `checks/tests/real_tree.rs`, `host/tests`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 4a item (revision 23 scope, split 2026-11-16). Items of Stage 4a run one at a time, in id order, because
  they share `core`, `host` and the generated files.
- Carried from `ts-0100-pu` (review, 2026-11-16): `pack update` loads the line first (`host/src/pack_update.rs`
  `pack_update`), so any load problem refuses every update. A pack whose locked commit the remote no longer has, or
  one broken at its pin, cannot be moved to a fixed commit with `pack update`; one broken pack also blocks updating
  the others. When this item replaces the lookup (`from_board`, `from_user`), find a `git:` pack by its lock entry
  and source without a clean load, so `pack update` is the way out of a bad pin. A test: a lock whose commit is gone,
  then `pack update <name> --to <ref>` moves it.
- Owner decisions (2026-11-16, at the split): the recommendations above are taken under the owner's delegation; they
  stay overridable until the item is built.

### Built

Wire (`core/src/owner/config.rs`, then `pnpm gen`):

- `PackOrigin::Offered`; `PackEntry.working_copy` and `PackEntry.review`; new `PackReviewData` and `PackReviewLine`.
- `ConfigPackAddParams` gains `confirm` and `approve_hosts`; `ConfigPackResult` gains `written`, `note` and `needs_host`
  (the new-host rule of `ts-0100-pu` applies to `add` too).

Core (`core/src/trust.rs`): `TrustFile.working_copies` (`WorkingCopyTrust { seen, integrated }`) with
`set_working_copy_seen`, `note_integrated`, `working_copy_waiting` and `acknowledge_working_copy`. An acknowledgement
clears only the tree an integrate left, and then that tree is the one seen.

Host:

- `host/src/user_packs.rs` (new): `UserScope` (home, board id, workspace paths), `BoardPacks { on, links }`,
  `read_global_files`, `git_pack_name`, `manifest_name`, `write_sources` and `write_board` (`yaml_out`, then
  `write_atomically`; files 0o600, folders 0o700).
- `host/src/working_copy.rs` (new): `WorkingCopies` (`see`, `note_integrated`, `waiting`, `acknowledge`) and
  `Waiting::render`. Objects of both trees are kept in `<cache>/working-copies.git`.
- `host/src/git_objects.rs`: `tree_of_folder_into` (same id as `tree_of_folder`, objects kept).
- `host/src/board_load.rs`: `PackOrigin::{User, Linked}`, `LoadedPack.dev` and `.overrides`, `LoadContext::with_user`,
  `Load::user_packs` and `Load::dev_pack`, `board_files`. A link replaces the repo pack in place; user packs follow the
  board's. User and linked packs add nothing to `trust_inputs`. A load problem for a waiting working copy carries the
  diff. `git_pack` and `pack_from_mirror` take the lock path and the origin.
- `host/src/pack_update.rs`: `pack_update` no longer loads the line. It reads `board_files` and the user's files, finds
  a pack by name (HEAD blob, cache entry, mirror blob at the locked commit, or the dev folder), and for a `git:` pack
  whose name cannot be read it fetches and checks the name at the new commit. `PackUpdateError` is now `PackError`.
  `tree_changes` and `fetch_for` are shared with the new module.
- `host/src/pack_manage.rs` (new): `pack_add`, `pack_link`, `pack_enable`, `pack_list`.
- `host/src/config_get.rs`, `host/src/daemon.rs`: `ConfigService::{pack_list, pack_add, pack_link, pack_enable}` and
  `with_workspaces` (the workspaces folder plus every `workspace_path` the store holds); the four handlers and arms.
  `config.get` now loads with the user scope, so a linked working copy's settings are read.
- `host/src/pack_cli.rs`, `host/src/bin/collide.rs`, `host/src/commands.rs`: `collide pack add|link|enable|disable|
  list|update`; `pack test` exits `NOT_BUILT` ("a later stage"). The `cli: pack` stub is gone.
- `checks/tests/real_tree.rs`: `FILLED_TAGS` gains `cli: pack`. `host/tests/command_tree.rs`: `report` is the one stub.
  `host/tests/config_get.rs`: the "pack methods are not served" test is removed, because all four are served.

Tests:

- `core/tests/trust.rs`: `a_working_copy_waits_for_an_acknowledgement_only_after_an_integrate_changed_it`,
  `an_acknowledgement_clears_the_tree_it_names_and_not_a_later_one`. Red first (missing methods, compile error).
- `host/tests/user_packs.rs` (40 tests): add (preview, confirm, second add, dev, bad manifest, built-in name, repo
  source), load order, name clash, dev folder in a workspace at add and at load, dev tree id, link (folder, none,
  pointer file, not a pack, name differs, workspace, scope per board and per user), offered folders, differing repo
  pack, integrate and acknowledgement with diff, enable and disable, list, and the CLI (`command::*`, exit codes and
  messages).
- `host/tests/pack_update.rs`: `a_lock_whose_commit_is_gone_is_moved_by_update_to_a_ref`,
  `one_broken_pack_does_not_block_updating_another`,
  `a_name_that_no_pack_at_its_pin_has_is_still_not_found_when_a_pin_is_bad`.
- Red: the new host tests were red as a group, because the host library did not compile (`PackEntry` missing fields,
  no `with_user`, no `working_copy` module). The three `pack_update` tests were written against the old `pack_update`,
  which loaded the line first. I did not run them against the old code once it compiled, so I cannot say each failed
  at runtime. All are green now.

### Decisions for the owner

- **Unlinking.** The plan has `pack link` and no way to undo it. Recommend `collide pack unlink <name>` (removes the
  link from the board's `packs.yaml`), small and the same shape as `link`. Alternative: edit the file by hand.
- **The acknowledgement has no wire or CLI yet.** `WorkingCopies::note_integrated` and `acknowledge` exist, and the load
  problem shows the diff, but nothing calls them: the integrate and the Queue are later stages. Recommend the plan
  text for that stage says the integrate calls `note_integrated` for each linked folder it changed, and the Queue item's
  action calls `acknowledge` with the tree id it showed.
- **`dev:` packs are read in place, not materialized.** No cache entry and no SDK link, so a dev pack's hooks that
  import the SDK resolve it from their own folder. Recommend materializing at the next item that runs a hook from a
  pack. Alternative: link the SDK into the folder, which writes into the person's folder.
- **Dev folders are not trust-enforced at load.** `add` and `link` record trust (`trust_dev`), but a load does not
  require it, as the plan says a dev pack is hashed at every load and trusted by the caller. Recommend ts-0102 decides
  with the trust prompt.
- **Linking a `git:` pack of the board is refused** (a link replaces a repo pack; a git pack moves with `pack update`).
  Alternative: allow it. Recommend refusing until someone asks.
- **A stale link** (its repo pack is gone) loads as an extra dev pack after the board's. Recommend keeping this.
- **`pack list` is refused when the line has a load problem.** It names the problems. Alternative: list what loads.
  Recommend keeping the refusal until the Queue shows problems.
- **Added user packs start off** on every board. Alternative: on for the board where they were added. Recommend off,
  so `add` never changes a board by itself.

- Owner decisions (2026-11-16, at the build): the eight above as recommended, on delegation. `pack unlink` is not
  built: a link is undone by editing the board's `packs.yaml` until an item adds it. Dev-folder trust at load is
  carried to `ts-0102-vu` as a note there. Changed at the review: a user pack whose name cannot be read without
  loading it (a gone `dev:` folder, a cold cache) is loaded only on a board that switched on a name no readable pack
  has, so one broken pack does not fail every board (`Load::user_packs`; test
  `a_user_pack_that_cannot_be_read_does_not_break_a_board_that_has_it_off`, seen red). The three `pack_update` tests
  were checked at the review: each fails when the old load-first refusal is put back.

### Not done, by design

- Daemon lines, the trust prompt, asking for a new host at load, `board.trust*`, `board.init`, `collide init` and
  `collide trust` (ts-0102 to ts-0105).
- The Queue item for a waiting working copy, and the integrate that marks one (a later stage).
- `collide pack test` (a later stage).
- Materializing `dev:` packs (see Decisions).
- Experiments refusing `dev:` (a later stage, as the item says).
