---
id: ts-0102-vu
kind: task
status: ready
title: "Daemon and CLI: lines with trust, collide init, pack data and the disk floor"
owners: []
links:
  - { type: blocked-by, to: ts-0101-bd }
---

## Goal

The library of the earlier items put in front of the daemon and the person. Until now `boards::summary` gives each line
an empty `Untrusted` hash and no snapshot, `board.trust` and `board.init` answer "not served yet", there is no
`collide init`, the daemon's disk floor is fixed at 5 GiB, and no pack gets a data directory.

- **Lines with their trust** (`board.open`, `board.list`). A board's line is the branch its checkout is on (as today).
  Its `snapshot` is the commit that branch names, its `trust` is `trust_state` of the loaded line (`ts-0097-kw`): the
  real hash, `Trusted` or `Untrusted`. A line whose load fails (a bad `board.yaml`) is listed with its problems in the
  first-run style, not as an error of `board.list`; a checkout with no commit has no line. Loading uses the cache, so a
  repeat is cheap.
- **`board.trust`** (`BoardTrustParams`: `board`, `line`, `hash`): loads the line again, refuses when the hash differs
  from the one given, otherwise records it, and returns the new `TrustState`. It gains an optional `hosts` list, the
  hosts the person approved for a first fetch (`ts-0100-pu`), recorded in the hosts file.
- **`board.trust_review`** (new): for a line, the current hash, the changes since what was trusted (the diff of
  `ts-0097-kw`), the packs' reviews, and the hosts a fetch would contact for the first time. The CLI and, later, the app
  show it before the person says yes. It writes nothing.
- **`board.init`** (`BoardInitParams`: `board`, `dry_run`): "Share with team" (0.11). Writes `.collide/board.yaml` into
  the board's root: a header comment saying what the file is and where the docs are, and `packs: []`, so the built-in
  packs still run. It refuses when `.collide` exists (a directory or a pointer file), naming it. It never commits, never
  edits `.gitignore` and writes nothing else. `dry_run` returns the list and writes nothing. The lock appears with the
  first `git:` pack (`ts-0100-pu` writes it).
- **`collide init [--dry-run]`** prints the files written (or that would be). **`collide trust [--line <ref>] [--yes]`**
  shows the review (the hash, each change with its diff, each pack's review, the new hosts) and asks `Trust this? [y/N]`
  on a terminal; with no terminal and no `--yes` it refuses and says why. `--yes` trusts exactly the hash it shows.
  Both go through the daemon like the other commands, and both are new in the CLI table.
- **Pack data.** A provider's command (`builtin_packs::ProviderLaunch::command`) sets `COLLIDE_PACK_DATA` to
  `pack_data_dir(...)` (`ts-0096-ph`) for the pack and board, so a provider or hook can keep its own files (0.7).
  Nothing launches a provider yet (Stage 4b); the variable is part of the command a test can inspect.
- **The disk floor.** `daemon.rs` builds `disk::Floor::default()`. It reads `disk.floor_gib` through the layers
  (`ts-0098-nx`) instead: for each check, the effective value for that board (the repo's value, or the user's when it
  is `overridable`, which `disk.floor_gib` is). A bad value is reported as a load problem and the default is used.

## Success criteria

- `board.list` on a fixture repo shows the line's commit as `snapshot` and a real hash; after `board.trust` with that
  hash the state is `Trusted`; after a commit that changes `board.yaml` it is `Untrusted` again with a different hash.
- `board.trust` with a stale hash fails and records nothing.
- `board.trust_review` after a committed pack change lists the pack and its changed files and, for `pack.yaml`, the
  lines; before any trust it lists everything as new. It lists a first-time host and writes nothing.
- `collide trust` with no terminal and no `--yes` exits nonzero and says to use `--yes` or a terminal; with `--yes` it
  trusts the shown hash; with a terminal (a pty in the test) `y` trusts and anything else does not.
- A repo pack change committed at the line's ref, with no lock edit, loads after a trust prompt that shows the diff
  (the stage's criterion, end to end through the CLI).
- `board.init` with `dry_run` lists `.collide/board.yaml` and writes nothing. Without it, it writes the file, which
  loads with `check_board`, and a second call refuses naming `.collide`. A repo with a `.collide` pointer file refuses
  too. No other file is made or changed, and nothing is committed.
- `collide init` after `git init` and a first commit works, and `board.list` then shows the line as `Untrusted` until
  the `.collide/board.yaml` is committed (the snapshot has no such file before) and then until trusted.
- A provider command for a pack on a board has `COLLIDE_PACK_DATA` set to a folder that exists, is `0700`, and is not
  shared with another pack or another board.
- `disk.floor_gib: 1` in the user's file lowers the floor the guard uses (a fake free-space reading just under 5 GiB
  passes), and the same key in the repo's file raises it unless the user's wins, as `overridable` says. A value of 0 or
  a string is reported and the 5 GiB default holds.
- `board.open` of a folder that is not a repo is unchanged, and so is every 3a to 3d test.
- `pnpm gen` is run for the two wire changes (`board.trust_review`, `hosts`), and `pnpm check` passes.
- No new crate.

## Tests

Write these first, over fixture repositories, a temp `HOME` and a daemon in a temp `COLLIDE_DAEMON_ROOT`:

- `board.list` and `board.open`: the line's snapshot and hash, trusted, changed, a bad `board.yaml`, no commit, a folder
  that is no longer a checkout;
- `board.trust` and `board.trust_review`: the stale hash, the review of a pack change, the review of a first load, the
  new-host list, nothing written by a review;
- `board.init`: dry run, write, repeat, a pointer file, a read-only root, the file's content (it loads, it has the
  comment, it holds `packs: []`);
- the CLI: `collide init`, `collide trust` with `--yes`, with no terminal, with a terminal and `y` and `n`;
- the end-to-end criterion above;
- `COLLIDE_PACK_DATA` on the provider command, and two packs and two boards differ;
- the floor: from the user's file, from the repo's, `overridable` flip, a bad value, no setting; the guard's checks in
  `disk.rs` stay as they are;
- the wire types' round trip and the checked-in schemas.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4a: Minimal pack loading (`collide init`; the trust prompt); Settings (`disk.floor_gib`); Pack data;
    Success Criteria (a repo pack change loads after a trust prompt that shows the diff).
  - 0.7: Pack data; Settings. 0.9: Trust. 0.11: First run works with no pack and no tasks (`collide init`, Share with
    team). 0.12: lines. Revision 20 (`disk.floor_gib`).
  - Stage 6a owns the Trust UI, Share with team and the Settings page; this item is the daemon side and the CLI.
- Carried from `ts-0055-cr` (the disk floor, revision 20): `disk.floor_gib` is a setting, and the daemon builds
  `Floor::default()` until this item reads it.
- **Decision for the owner, with a recommendation: a CLI command to trust.** The plan has a trust prompt and no CLI
  command for it. Recommend `collide trust`, as above, so a person with no app can trust a line and the criterion can be
  shown end to end. Alternative: leave trust to the app (6a) and test only through `board.trust`. Then 4a's criterion
  "loads after a trust prompt that shows the diff" has no prompt in 4a. Recommend the command: it is small, and it is
  the CLI the plan's `collide init` already implies.
- **Decision for the owner, with a recommendation: what `collide init` writes.** The plan says only "writes
  `board.yaml`"; 0.11 says Share with team "writes `.collide/`". Recommend only `board.yaml`, as a short commented file
  with `packs: []`, because everything else (config, profiles, views, lock) is made when the person adds one.
  Alternative: also make empty `views/` and `profiles/` folders; git does not track empty folders, so they would vanish.
  The built-in packs run with no `board.yaml` at all, so init is only for sharing.
- **Decision for the item:** the line's `ref_name` stays the checkout's branch, and a detached HEAD uses `HEAD`. Lines
  for other refs are Stage 14.
- **Decision for the item:** `board.list` does the loading on each call and relies on the cache. If it is slow on a big
  repo a later item caches the `LoadedLine` by commit; do not add that here.
- **Decision for the item:** `board.trust_review` is a new method, not a field on `LineSummary`, so `board.list` stays
  cheap and the diff is computed only when someone asks.
- Do not change `collide run`, which uses the built-in role and no repo pack. Refusing a run on an untrusted line
  belongs to the stage that runs repo code (Stage 7).
- Area: `host/src/daemon.rs`, `host/src/boards.rs`, `host/src/commands.rs` and `host/src/bin/collide.rs` (`init`,
  `trust`), `host/src/builtin_packs.rs`, `host/src/disk.rs` (the floor source only), `core/src/owner/board.rs` (wire),
  `schemas/`, `host/tests`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Settings files are fixtures, never the owner's own `settings.json`.
- Carried from `ts-0097-kw` (decision 9 and Not done): remove a board's entries from `trust.yaml` when the board is
  removed, and pass the mirror's path to `Trust::diff` so a `git:` pack's change shows its files, not "not in this
  repository".
- Stage 4a item (revision 23 scope, split 2026-11-16). Items of Stage 4a run one at a time, in id order, because
  they share `core`, `host` and the generated files.
- Carried from `ts-0100-pu` (review, 2026-11-16): a repo `git:` pack on a cold cache whose host was never fetched
  from fails the load with `LoadProblem::needs_host`. The trust prompt here asks for that host and records it on a
  yes. (`pack update` no longer loads the line first since `ts-0101-bd`, so `--allow-host` already works there.) A
  timeout kills only git's direct child (`run_bounded`); an `ssh` or remote helper under it can outlive it. A
  process-group kill belongs with the daemon's other process work (Stage 8) if it shows up.
- Carried from `ts-0101-bd` (review, 2026-11-16): a `dev:` folder is trusted per folder (0.9), and `pack add` and
  `pack link` record it (`TrustStore::trust_dev`), but a load does not yet require it: a user pack or a link whose
  folder was never trusted loads. When this item adds the trust prompt, a load refuses an untrusted `dev:` folder
  (user pack, link or offered working copy) and the prompt can trust it. A test: a `dev:` source written into the
  user's `packs.yaml` by hand is refused until trusted.
- Owner decisions (2026-11-16, at the split): the recommendations above are taken under the owner's delegation; they
  stay overridable until the item is built.
