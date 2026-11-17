---
id: ts-0102-vu
kind: task
status: done
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

### Built

- **Lines with trust.** `host/src/board_lines.rs` (`Lines`) gives `board.open` and `board.list` a real `snapshot` (the
  commit of `HEAD`, `boards::head_commit`) and a real `trust` per line. It loads at that commit on every call. A load
  that fails lists the line with its problems and an empty hash. A checkout with no commit has no line. `ref_name` is
  the branch, `HEAD` when detached. An unreadable `trust.yaml` leaves the line untrusted and names the file.
  `boards::summary` is gone; the daemon calls `ConfigService::lines()`.
- **`board.trust`** refuses a stale hash with `CONFLICT` and records nothing. `BoardTrustParams` gained `hosts` and
  `dev_folders`. **`board.trust_review`** is new and writes nothing: hash, `changes` (`TrustChange`, with the files
  and diff lines of a folder), each repo or `git:` pack's `PackReviewData`, and the `hosts` and `dev_folders` a failed
  load needs. `LineSummary.problems` is a third wire change (see Decisions). `pnpm gen` was run.
- **`board.init`** (`host/src/board_init.rs`): writes only `.collide/board.yaml` (a two-line comment and
  `packs: []`) through `write_atomically`. It refuses when `.collide` exists, commits nothing, edits no `.gitignore`.
  `dry_run` writes nothing. Both board writers refuse a run's agent (`NOT_PERMITTED`).
- **CLI** (`host/src/board_cli.rs`, arms in `host/src/bin/collide.rs`, rows in `COMMANDS` and `TREE`):
  `collide init [--dry-run]`; `collide trust [--line <ref>] [--yes] [--allow-host <host>] [--allow-dev <folder>]`.
  `trust` prints the hash, each change with its diff, and each pack's review, then asks `Trust this? [y/N]` on a
  terminal. With no terminal and no `--yes` it exits 5 and names `--yes` and the terminal. `--yes` trusts exactly the
  hash shown. A host or `dev:` folder the load needs is asked about first (or `--allow-host`, `--allow-dev`), recorded
  on a yes, and the line is reviewed again.
- **Pack data.** `ProviderLaunch.data` and `with_pack_data` set `COLLIDE_PACK_DATA` to a 0700 folder, one per pack and
  board (`board_load::pack_data_dir`). `command()` stays infallible.
- **Disk floor.** `disk.floor_gib` is read through the settings layers per board: `ConfigService::floor`, which does
  not go through `get` (no keychain check). `FileRunConfig.floor` and `PhasesConfig.disk` hold a `FloorSource`
  (`host/src/disk.rs`) instead of a `Floor`; the daemon's source finds the run's board and logs the problems.
  `DaemonOptions.floor` is gone. A bad value (0, a string) is a problem and 5 GiB holds.
- **Carried work.** `Trust::forget_board` (and `TrustFile::remove_board`) removes a board's `trust.yaml` entries.
  `Trust::changes_since_trust_with_mirrors` passes each `git:` pack's mirror to `Trust::diff`. `LoadProblem` gained
  `needs_dev_trust`, and `Load::dev_pack` refuses a `dev:` folder never trusted (user pack, link, offered working
  copy) before it reads the manifest.
- **Harness.** `app/browser/support/world.ts` `removeRoot` makes the tree writable first: a board's first load now
  copies the SDK read-only into the daemon's cache, which `rm` could not empty. `config_fixture.rs` gained
  `plain_factory`.

Tests, by file. Seen red at runtime means it failed against the old behaviour or a stub before the code was written.

- `host/tests/board_lines.rs` (24): lines, snapshot, trust, review, hosts, a git pack's diff. 23 seen red against the
  old daemon.
- `host/tests/board_init.rs` (9): all seen red (method not served).
- `host/tests/board_cli.rs` (26): the question, the refusal with no terminal, `--yes`, a host, a `dev:` folder, the
  end-to-end change (`a_committed_change_to_a_repo_pack_loads_after_the_prompt_that_showed_its_diff`), `init`, usage.
  All 26 seen red against a stub that exits 6.
- `host/tests/board_process.rs` (4): the binary with an empty, non-terminal input. Not seen red: written after the
  code, over the same paths `board_cli.rs` saw red.
- `host/tests/disk_floor.rs` (9): 8 seen red against a stub that always answers 5 GiB (the ninth, no setting, is
  the default). In `adapters/claude/file_run.rs`: `a_floor_of_1_gib_in_the_users_file_lets_a_run_start_with_4_gib_free`
  seen red (timed out); `with_no_floor_setting_4_gib_free_is_under_the_default_floor_and_pauses_the_run` and
  `a_floor_of_0_is_refused_by_the_settings_so_the_default_still_pauses_the_run` hold the old behaviour too.
- `host/tests/user_packs.rs`: `a_dev_source_written_by_hand_is_refused_until_it_is_trusted` and two more, seen red;
  `a_dev_pack_is_hashed_at_every_load_...` now approves its folder first.
- `host/tests/trust.rs`: `forgetting_a_board_removes_its_lines_and_keeps_the_rest` seen red against a stub.
  `a_git_packs_change_shows_its_files_from_the_mirror` seen red with the mirror path taken out.
- `host/tests/builtin_packs.rs`: three of four pack-data tests seen red; the fourth is the unchanged default.
- `core/tests/owner_protocol.rs`: new samples for the three changed or new methods.

### Decisions for the owner

1. **Approving a host or folder is a two-step `board.trust`.** A host or `dev:` folder blocks the load before there is
   a hash, so `board.trust` with an empty hash records only what the load needs (`hosts`, `dev_folders`) and trusts
   nothing. The CLI then reviews again and asks about the hash. Alternative: a separate `board.allow` method. Recommend
   keeping this: no new method, and the hash is still the only thing a yes trusts.
2. **`--yes` trusts the hash, not hosts or folders.** A script passes `--allow-host` and `--allow-dev` for those.
   Alternative: let `--yes` cover them. Recommend not: a first fetch from a host is its own decision (0.9).
3. **A third wire change, `LineSummary.problems`.** The item named two. A bad `board.yaml` needed somewhere to show.
   Alternative: only an empty hash. Recommend keeping it.
4. **`board.init` and `board.trust` refuse a run's agent.** `config.pack_*` and `config.set` do not; that is left as it
   is. No test: it needs a process inside a run. Recommend the same refusal on `config.pack_*` in a later item.
5. **`board.list` loads each call**, so a cold `git:` cache on a known host can fetch during a list. Alternative:
   cache the summary until a commit moves. Recommend that once the board switcher polls.
6. **`board.trust --line` takes any ref that resolves**, keyed by the ref text given. Recommend keeping this.
7. **A bad floor setting never stops a run.** The daemon logs the problem and the 5 GiB default holds. Alternative:
   refuse to start the run. Recommend keeping this: the setting is checked and shown by `config get`.
8. **Pack data is delivered, not yet used.** `with_pack_data` exists and is tested; nothing launches a provider on a
   board yet. Recommend wiring it in the item that does (Stage 4b).
9. **Plan stub tags.** `stub-tags.json` lists `cli: config` and `cli: pack` as 4a though they are built; no tags were
   added for `init` and `trust`. Recommend deleting the two stale rows with the plan revision.

- Owner decisions (2026-11-16, at the build): 1 to 3 and 5 to 8 as recommended, on delegation. 4 changed at the
  review: a run's agent is refused `config.set` and every `config.pack_*` write (add, link, enable, update), as
  `board.trust` is, because adding a user pack or linking a folder trusts it (0.9) and the settings are the person's;
  the reads stay open (`refuse_run_agent` in `host/src/daemon.rs`; test
  `a_runs_agent_may_not_trust_add_switch_or_set_anything_for_the_person` in `host/tests/board_process.rs`, seen red
  at `pack add`). 9 not taken: a filled tag stays in `stub-tags.json` and counts zero, which is how CI shows a stage
  complete (`checks/tests/real_tree.rs`).

### Not done, by design

- A pseudo-terminal test of the question. The terminal check is `IsTerminal` on standard input; the answers are
  tested through `trust_with` with `tty` true and typed input. A test that starts the binary on a terminal would be a
  child of the test run and so look like a run's agent.
- Calling `Trust::forget_board`: no code removes a board yet. It is built and tested for the item that does.
- Measuring the launch line (ts-0103-…), role options (ts-0104-…), the owner check (ts-0105-jv-…).
- `collide run` is unchanged. `collide pack test`, a Queue item for a waiting working copy, and materializing `dev:`
  packs stay with their later stages.
