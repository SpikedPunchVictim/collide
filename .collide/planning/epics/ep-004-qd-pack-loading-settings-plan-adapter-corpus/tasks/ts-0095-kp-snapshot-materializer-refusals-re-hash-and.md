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
