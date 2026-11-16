---
id: ts-0101-bd
kind: task
status: ready
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
- Owner decisions (2026-11-16, at the split): the recommendations above are taken under the owner's delegation; they
  stay overridable until the item is built.
