---
id: ts-0095-kp
kind: task
status: active
title: "Snapshot materializer: refusals, re-hash and the cache"
owners: []
links:
  - { type: blocked-by, to: ts-0094-av }
---

## Goal

Turn a pack's git tree id into a cache entry a hook can run from, by the recipe S4 measured and 0.6 sets down. One code
path for every source: `repo:`, `git:` and `dev:` all end at a tree id and a git directory to read it from (the line's
repository, a bare mirror from `ts-0100-pu`, or the `dev:` folder's own files).

`materialize(cache, sdk, repo, tree)` in `host`:

1. **Hit.** `<cache>/packs/<tree id>/` with its marker `.collide-verified` is the answer: no git read, no hash.
2. **Read.** `git_objects::list(repo, tree)`, then refuse, naming the path and the reason, before any file is written:
   - mode `120000` (a symlink) and `160000` (a submodule);
   - any path with a `node_modules` part (a committed `node_modules/@collide/pack-sdk` shadows Collide's SDK: measured
     S4);
   - any file named `package.json` at any depth (Node would use the nearest one, and Collide writes its own);
   - a part that is `.`, `..` or `.git` in any case, an empty part, a backslash or a NUL: a hand-made tree can hold
     them, and `ls-tree` prints them.
3. **Write** into `<cache>/packs/.staging-<tree id>-<pid>/pack/`: each file's bytes from `read_blobs`, with mode `0755`
   for `100755` and `0644` for the rest, and folders `0755`. Then Collide's own `package.json`
   (`{"private":true,"type":"module"}`), the `node_modules/@collide/pack-sdk` symlink to the versioned SDK copy, and
   last the marker.
4. **Re-hash.** `git_objects::tree_of_folder(<staging>/pack)` must equal the tree id. A mismatch removes the staging
   folder and fails naming both ids. (This is what catches a case-insensitive volume that merged `Foo` and `foo`, and a
   write that changed a mode.)
5. **Make it read-only** (files `0444` or `0555`, folders `0555`), then `rename` the staging folder into place. A rename
   that loses to another loader (the target exists and is not empty) discards the staging folder and returns the
   winner's entry.

`ensure_sdk(cache, bundle_sdk)` copies the compiled SDK that the app bundle holds (`node_modules/@collide/pack-sdk`
under the bundle that `pack_location::locate_bundle` finds, built by `scripts/build-packs.ts`) into
`<cache>/sdk/<version>/` once, read-only, with its own marker, and returns that folder. The version is the `version` of
the SDK's `package.json`. It is a copy outside the app bundle, as S4 advises, so the entry's symlink does not point into
an app that is replaced on update. Built-in packs are not materialized (0.6, R5) and never reach this code.

Staging folders left by a loader that died are removed on the next miss when they are older than an hour.

## Success criteria

- A tree with ordinary files materializes: `pack/` holds the same bytes and the same modes (`0755` and `0644`), the
  entry holds Collide's `package.json`, the SDK symlink resolves to the SDK copy, and the marker exists.
- `tree_of_folder(entry/pack)` equals the tree id for the entry the code made.
- The refusals each name the path: a symlink, a submodule, `node_modules/x/y.js`, `hooks/package.json`, a `.git`
  part, a `..` part. A pack that commits `node_modules/@collide/pack-sdk` is refused and nothing of it is written.
  Nothing is left behind after a refusal: no staging folder, no entry.
- A hit returns without spawning git (a fake `git` that fails if run proves it).
- A write that is changed on the way (a test double for the file writer that alters one byte, or a mode) is a
  mismatch error naming both ids, and leaves no entry.
- Two threads and two processes materializing the same tree at once: both get a complete entry, one stages and wins,
  no staging folder is left.
- The entry is read-only and a hook can run from it: `node <entry>/pack/hooks/x.ts` that imports the SDK runs (when
  `node` is on the test's `PATH`; the test says so and passes otherwise, as the existing node tests do).
- `ensure_sdk` is idempotent, read-only, keyed by version, and a second call writes nothing.
- A tree over the caps of `ts-0094-av` is refused before any write.
- Every written path is below the staging folder: a tree whose entry would land outside it never writes a byte
  outside the cache.
- Removing an entry (the tests' helper `remove_read_only`) works, so a temp root can be cleaned up.
- No new crate.

## Tests

Write these first, over repositories built in a temp root, with the cache and the SDK in the same root:

- the happy path: files, an executable, nested folders, an empty file, a file with NULs; the layout and the modes;
- each refusal, with a message that names the path; the committed-SDK pack;
- the re-hash: a double that writes one wrong byte, one wrong mode, and a case-colliding pair on a volume where they
  collide (skip with a message on a case-sensitive volume, and test the writer's own collision check on a fake);
- hit without git; miss then hit; a half-built staging folder from a dead pid is swept after an hour and not before;
- the race: 8 threads on one tree; two child processes (the test binary re-run) on one tree; the loser's staging folder
  is gone;
- read-only entry; a hook that imports the SDK runs;
- `ensure_sdk`: first call copies, second does nothing, a new version gets its own folder;
- the path guard: a hand-made tree object (`git mktree` over crafted entries) with `..` and `.git` parts.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4a: Minimal pack loading (snapshot materialization; refusals); Pack sources and scope (one code path).
  - 0.6: Materialization; "A pack that contains a `node_modules` path or a `package.json` is refused at load";
    "Built-in packs are not materialized".
  - 0.7: Snapshot rule. S4 record: the recipe and its hazards.
- **Decision for the item, with a recommendation: where the file writes are checked.** The re-hash is the check that
  the bytes and modes are right. Recommend the writer is a small trait (`PackWriter`) with a real impl and the test
  double that alters a byte, so the mismatch path is seen failing on its own assertion. Alternative: corrupt the file
  on disk between write and hash with a hook in the test. Recommend the trait: it is one boundary and the plan's
  dependency-injection rule (0.15).
- **Decision for the item:** `ensure_sdk` is called by the loader (`ts-0096-ph`) once per daemon start and on a miss.
  The bundle's SDK path comes from `pack_location`; a bundle with no SDK is an error that names the bundle folder.
- **Decision for the item:** the entry is made read-only, so a cache cleaner must restore write bits before removing
  it. No cleaner is built here; the test helper does it. Retention of old entries is a later item (the cache is keyed
  by tree id and entries are immutable).
- `package.json` is Collide's, with `"type":"module"`, written as bytes, not through a YAML or JSON writer.
- Area: `host/src/pack_cache.rs` (new), `host/src/pack_location.rs` (the SDK path, if it needs a helper), `host/tests`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 4a item (revision 23 scope, split 2026-11-16). Items of Stage 4a run one at a time, in id order, because
  they share `core`, `host` and the generated files.
- Owner decisions (2026-11-16, at the split): the recommendations above are taken under the owner's delegation; they
  stay overridable until the item is built.

### Built

- `host/src/pack_cache.rs` (new, `pub mod pack_cache` in `host/src/lib.rs`):
  - `Materializer` (`new`, `with_git(GitObjects)`, `with_writer(Box<dyn PackWriter>)`) and the free function
    `materialize(cache, sdk, repo, tree) -> Result<PathBuf, CacheError>`, the entry folder `<cache>/packs/<tree>/`.
    Order: `check_id` on the tree id; a hit (marker present) returns with no git; `git_objects::list` (its caps refuse
    before any write); `check_entries`; the SDK folder must hold a `package.json`; then the cache folders are made,
    stale
    staging is swept, and the entry is built in `.staging-<tree>-<pid>-<n>/`: `pack/` through the writer, Collide's
    `package.json`, the `node_modules/@collide/pack-sdk` symlink (absolute), the marker (holds the tree id) last. Then
    `tree_of_folder(staging/pack)` against the tree id (`CacheError::Mismatch` names both), lock, rename.
  - `check_entries(&[TreeEntry])`: refuses, naming the first path in order, a part that is empty, `.`, `..`, `.git`
    (any case), holds a backslash or a NUL, or is `node_modules` (any case); a file named `package.json` (any case);
    mode `120000`, `160000` or any mode that is not `100644` or `100755`. Messages print control characters escaped.
  - `PackWriter` (trait: `write_file(path, bytes, mode)`) and `FsWriter` (`create_new`, then an explicit `chmod`, so the
    umask cannot change the mode and an existing name is an error, never an overwrite).
  - `ensure_sdk(cache, bundle_sdk)`: copies into `<cache>/sdk/<version>/` through the same staging, marker, lock and
    rename. The version comes from `package.json` and must be safe as a folder name. A symlink in the bundle is refused.
  - `publish` (lock below the staging folder, rename, then lock the top folder; a lost rename returns the winner's
    entry; an existing folder with no marker is `CacheError::Incomplete`), `Staging` (a guard that removes the staging
    folder, also read-only ones, when dropped), `sweep_stale_staging` (`.staging-*` folders older than `STALE_STAGING`,
    one hour, on a miss only).
- `host/src/pack_location.rs`: `bundle_sdk(bundle)` finds `<pack>/node_modules/@collide/pack-sdk` (with a
  `package.json`) in the first pack, in name order, that has one; `NoSdk` names the bundle folder. This is where
  `scripts/build-packs.ts` puts it (in `plan/`), not at the bundle's top.
- `host/src/git_objects.rs`: `check_id` is `pub(crate)`. No other change.
- Generated files: no wire type changed, so no `pnpm gen`.
- Tests, `host/tests/pack_cache.rs` (47) and four in `host/tests/pack_location.rs`:
  - happy path: `an_ordinary_tree_materializes_with_its_bytes_and_its_executable_bits`,
    `the_entry_holds_collides_package_json_the_sdk_link_and_the_marker`, `the_entry_hashes_to_its_tree_id`,
    `the_entry_is_read_only_down_to_the_top_folder`, `an_empty_tree_makes_an_empty_pack`,
    `a_tree_of_a_folder_below_the_top_materializes_as_its_own_pack`,
    `a_tree_id_that_is_not_an_id_is_refused_and_nothing_is_made`;
  - refusals: `a_symlink_is_refused_…`, `a_submodule_is_refused_…`, `a_file_under_node_modules_is_refused_…`,
    `a_node_modules_folder_below_the_top_is_refused_too`, `a_committed_sdk_is_refused_and_none_of_the_pack_is_written`,
    `a_package_json_at_any_depth_is_refused_…`, `a_package_json_or_node_modules_in_another_case_is_refused`,
    `a_package_json_folder_or_a_longer_name_is_not_refused`, `a_dot_git_part_is_refused_in_any_case_…`,
    `a_dot_dot_part_is_refused_…_nothing_is_written_outside`, `a_dot_part_and_a_backslash_are_refused`,
    `paths_with_an_empty_part_a_nul_or_a_leading_slash_are_refused_by_the_check`,
    `a_mode_that_is_not_a_files_is_refused_by_the_check`, `a_tree_over_the_entry_cap_is_refused_before_any_write`,
    `a_blob_over_the_size_cap_is_refused_before_any_write`, `a_tree_that_is_not_in_the_repository_is_an_error`;
  - re-hash: `a_writer_that_changes_one_byte_is_a_mismatch_naming_both_ids_and_leaves_no_entry`,
    `a_writer_that_changes_a_mode_is_a_mismatch_and_leaves_no_entry`,
    `a_volume_that_merges_names_that_differ_in_case_is_a_mismatch_on_a_fake`,
    `on_a_volume_that_folds_case_the_real_writer_does_not_merge_names`;
  - hit and miss: `a_hit_returns_without_running_git` (a fake `git` that writes a flag and fails),
    `a_miss_runs_git_and_the_fake_proves_the_test_would_notice`,
    `a_miss_then_a_hit_gives_the_same_entry_and_writes_nothing_the_second_time`,
    `two_trees_make_two_entries_and_share_nothing`, the sweep tests (`a_staging_folder_older_than_an_hour_…`,
    `a_read_only_old_staging_folder_is_swept_too`, `only_staging_folders_are_swept_and_a_hit_sweeps_nothing`),
    `an_entry_without_a_marker_is_an_error_and_is_not_replaced`;
  - race: `eight_threads_on_one_tree_…`, `two_processes_on_one_tree_…` (the test binary re-run through
    `race_child_materializes_when_started_as_a_child`, with a cleared environment and a start file);
  - hook: `a_hook_that_imports_the_sdk_runs_from_the_entry` (the real built bundle, `node` on `PATH`);
  - `ensure_sdk`: `the_first_ensure_sdk_copies_…`, `a_second_ensure_sdk_writes_nothing`,
    `a_new_version_gets_its_own_folder_and_the_old_one_stays`, `threads_that_copy_the_sdk_at_once_…`,
    `an_sdk_whose_package_json_has_no_usable_version_…`, `an_sdk_folder_with_no_package_json_…`,
    `a_symlink_in_the_bundled_sdk_is_refused_and_nothing_is_left`, `the_staging_folder_of_a_dead_sdk_copy_…`;
  - `pack_location.rs`: `the_sdk_is_found_in_the_pack_that_holds_it`, `the_first_pack_in_name_order_…`,
    `a_bundle_with_no_sdk_is_an_error_that_names_the_bundle_folder`,
    `a_bundle_folder_that_is_not_there_is_an_error_that_names_it`.
- Red first: with a stub that held the signatures and `unimplemented!()`, every `pack_cache` test failed (46 of 47; the
  child entry point returns at once when run alone) and the `pack_location` SDK tests failed. The three re-hash tests
  were then checked by mutation: with the comparison switched off they fail, and with it on they pass. The race tests
  were checked to reach the lost-rename path (7 losers over one run). Two tests were first red for a mistake in the
  test (a hand-made tree whose blob was not in the repository gave `ls-tree` a bad size; a NUL in an echoed path), and
  were fixed in the test.

### Decisions for the owner

1. **The top folder of an entry is made read-only after the rename, not before.** The item says lock, then rename.
   Measured here: macOS refuses to rename a folder with no write bit (`Permission denied`; `build-packs.ts` notes the
   same). Everything below the top is locked first; the top is locked right after the rename. Between the two a reader
   sees a complete entry (the marker is in it) with a writable top folder. A crash there leaves one writable folder.
   Alternative: none that keeps the atomic rename. Recommend this.
2. **The staging name has a counter: `.staging-<tree>-<pid>-<n>`.** The item has no counter. Two threads of one process
   share a pid and would build in one folder. Recommend the counter.
3. **`PackWriter` takes the mode and uses `create_new`.** So a case-insensitive volume that already holds `Foo` fails
   the write of `foo` by name, and the re-hash is the second line. On this volume the real writer fails; the merge is
   shown on a fake. The writer sets the mode with `chmod` after writing, so the umask never counts. Recommend.
4. **Modes: the entry is `0444`/`0555`, not `0644`/`0755`.** The success criterion says `pack/` holds modes `0755` and
   `0644`, and also that the entry is read-only; both cannot hold. The tests check the executable bit is kept and
   the exact read-only modes. Recommend reading the criterion as "the same executable bits".
5. **`node_modules` and `package.json` are refused in any case**, as `.git` is. On a case-insensitive volume
   `Node_Modules` is `node_modules` to Node, and `Package.json` is the nearest `package.json`. Alternative: exact match,
   as the item says. Recommend any case. Not done: Unicode look-alikes of `.git` (git's own `is_hfs_dotgit`).
6. **Any mode other than `100644` and `100755` is refused**, with `120000` and `160000` named. `ls-tree` prints no other
   mode, so this is a guard. Recommend.
7. **An entry folder with no marker is an error (`Incomplete`), not replaced.** It can only come from outside this code
   (the rename is atomic). Alternative: remove it and rebuild. Recommend the error: the owner or a cleaner decides.
8. **`ensure_sdk` keys by version only.** A rebuilt SDK with the same `version` is not copied again, so while the SDK
   changes under one version number a dev machine keeps the first copy. Recommend bumping the SDK's version with any
   change that a pack can see; or, later, a content hash in the folder name (Stage 7 or the loader item). Not built.
9. **`bundle_sdk` takes the SDK from the first pack in name order that has one.** The item says "under the bundle"; the
   build puts it under each pack that depends on it, none at the top. Recommend; the packs carry the same build.
10. **The node test fails with a message when `node` is not on `PATH`, instead of passing.** The item says pass; the
    suite's rule (`common::node_missing_message`, ts-0074-cu) is to fail, and the test needs the built bundle, whose
    build needs `node`. Recommend the suite's rule.
11. **`dev:` is not materialized from the folder's own files.** `materialize` reads git objects only. `ts-0101-bd` has a
    `dev:` folder's tree id from `tree_of_folder`, but the objects are not in any repository. It will need to make a
    scratch repository from the folder (`git add` in a bare repository, as `tree_of_folder` does) and pass that
    as `repo`. Recommend this, and that `ts-0101-bd` owns it.
12. **The scratch folder of `tree_of_folder` stays the system's temp folder** (`ts-0094-av` decision 11), so a
    re-hash of a large pack writes there as well as into the cache.

- Owner decisions (2026-11-16, at the build): decisions 1 to 12 taken as recommended, under the owner's delegation.
  One change in review: an entry is keyed by the tree id and the SDK version (`entry_name`, `<cache>/packs/<tree
  id>-<SDK version>/`). Keyed by the tree alone, an entry kept the SDK it was first built with after a new SDK
  shipped, so `requires.sdk` would be checked against one SDK while hooks imported another. Plan 0.6 updated. Test:
  `a_new_sdk_version_makes_a_new_entry_for_the_same_tree_and_links_the_new_sdk`, red before. Carried to Stage 4b,
  where the SDK starts to change: a development build whose SDK changes without a new version keeps the old copy
  (decision 6); the pack build should stamp a dev version, or the owner bumps it.

### Not done, by design

- No loader, no lock, no trust (`ts-0096-ph`, `ts-0097-kw`): the loader calls `ensure_sdk` once per daemon start and
  `materialize` per pack. The loader passes the folder `ensure_sdk` returns.
- No `git:` fetch or mirror (`ts-0100-pu`), no `dev:` scratch repository (`ts-0101-bd`, decision 11).
- No cache cleaner and no retention. A cleaner must give write bits back first (`remove_read_only` in the tests'
  `common`).
- Built-in packs are not materialized and never reach this code.
- No hash check on a hit, as the item says: the marker is the proof. A damaged entry is a cleaner's or a repair
  item's concern.
- No check that `sdk` is the folder `ensure_sdk` made; a caller that passes another folder gets a link to it.
- The sweep reads the folder's modification time. A loader that builds for more than an hour with no new file in the
  top staging folder could have its folder removed; the caps make that very unlikely.
