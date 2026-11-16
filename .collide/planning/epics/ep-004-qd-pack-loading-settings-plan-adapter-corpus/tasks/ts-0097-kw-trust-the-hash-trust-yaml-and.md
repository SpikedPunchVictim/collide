---
id: ts-0097-kw
kind: task
status: active
title: "Trust: the hash, trust.yaml and the diff"
owners: []
links:
  - { type: blocked-by, to: ts-0096-ph }
---

## Goal

What a person has trusted, kept outside the repo, and what changed since (0.9, Trust). A library in `host` and its
types in `core`; the daemon and the CLI that ask the person are `ts-0102-vu`.

- **The trust hash.** Over a `LoadedLine`'s `trust_inputs()`: the tree id of each repo pack (a `git:` pack through the
  lock), the lock, the pointer file if there is one, `board.yaml`, `config.yaml`, the views, the profiles
  (`.collide/profiles/` is in the hash: 0.9), and the tree hashes of each loaded pack's `protectedPaths`. Each input is
  a name and a git id, written as sorted `name NUL id LF` lines, so the same inputs give the same text, and the hash is
  of that text. A built-in pack is not an input (0.4: trusted with the app).
- **The store**, `~/.config/collide/trust.yaml`, written through `yaml_out` (`ts-0093-am`), atomically (a temp file in
  the same folder, then rename), with mode `0600` in a `0700` folder, and read with the hardened loader. It holds, per
  board root and line ref, the trusted hash and the inputs it was made from; per user pack, its tree id and source
  (trusted when added); per `dev:` folder, the folder's path.
- **The decision.** `trust_state(store, line)` is `Trusted { hash }` when the line's current hash equals the stored one,
  else `Untrusted { hash }` with the current hash, which is the shape of `owner::board::TrustState`. A line that is
  untrusted gates what runs repo code (the later stages call `require_trusted`, which returns the hash to ask about).
  `trust(store, line, hash)` re-computes the hash and refuses when it differs from the one given, so a person trusts
  what they were shown (the `board.trust` rule).
- **The diff** a changed hash shows. `diff(old_inputs, new_inputs)` lists each input added, removed or changed. For a
  changed pack tree it adds the file list from `git diff-tree -r --name-status <old> <new>` (read through the reader),
  and for a changed text file the first 200 changed lines of its diff, so a committed pack change "loads after a trust
  prompt that shows the diff" (4a criterion). When the old objects are gone (a pruned clone), the diff says so and still
  lists the input as changed.
- **Per source** (0.9). A repo pack is covered by the line's hash. A user pack is trusted when added, keyed by its tree
  id; `trust_pack(store, source, tree)` records it, and a new tree id for the same source is untrusted again, with the
  diff. A `dev:` pack is trusted per folder: a change inside it does not ask again, but its tree id is recorded by the
  caller on every load (the badge, `ts-0101-bd`). A `dev:` folder inside a Collide workspace is refused there.
- The prompt's text for a user pack (source, commit, the hooks, providers and observers it runs, its `requires`) is a
  function of the loaded manifest, `pack_review(manifest)`, which the CLI and the app (6a) print.

## Success criteria

- An edited `pack.yaml`, hook, `board.yaml` or `config.yaml` on a branch is not trusted: the hash changes only when the
  change is committed at the ref, and then the state is `Untrusted` until `trust` is called. A changed hash asks again.
- `trust` with a hash other than the current one fails and stores nothing. With the current one it stores it, and the
  state is `Trusted`.
- A repo pack change committed at the line's ref, with no lock edit, gives `Untrusted` and a diff that names the pack,
  the files changed and, for `pack.yaml`, the changed lines.
- A new view, a changed profile, a changed pointer file and a changed lock each change the hash. A change to a file the
  hash does not cover (a `README.md` in the repo, an untracked file) does not.
- A `protectedPaths` entry's tree is in the hash: a change under a protected folder asks again.
- Two lines of one board (two refs) have two entries. Two boards (two roots) have two entries.
- `trust.yaml` is written atomically with `0600`, and a crash between the temp file and the rename leaves the old file.
  A `trust.yaml` that is malformed, has a duplicate key or is over the cap is a load error that names the file and is
  never read as "trusted": the state is `Untrusted`.
- A user pack is trusted at its tree id and untrusted at another. `pack_review` lists hooks, providers and observers
  and `requires` for a fixture pack.
- The diff of two input lists is deterministic and lists added, removed and changed inputs in name order.
- The hash does not depend on the order packs were read in, nor on the machine's paths.
- No new crate.

## Tests

Write these first, over repositories built in a temp root and a temp `HOME` for the store:

- hash stability and sensitivity, one case for each input kind above, and for each kind that must not change it;
- the working tree is not the snapshot: an uncommitted edit leaves the hash alone;
- `trust` with the shown hash, with a stale hash, and after the repo changed between the prompt and the answer;
- the store: round trip, per board and per line, a malformed file, a duplicate key, a file made `0644` by hand (read,
  then rewritten `0600`), a crash simulated by a write that fails after the temp file;
- the diff: a changed `pack.yaml` (lines), a new hook file, a deleted file, a changed `board.yaml`, a pruned old object;
- user packs: trusted at one tree id, untrusted at the next, and the review text;
- a `dev:` folder: trusted per folder, a content change does not ask again, a folder inside a workspace path is refused;
- the state type equals `owner::board::TrustState` on the wire (a serde round trip).

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4a: Minimal pack loading (the trust prompt over the hash of 0.9); Pack sources and scope (trust per source);
    Success Criteria (edited files not used; a changed hash asks again; the repo pack change with the diff).
  - 0.9: Trust, including Per pack source. 0.12: a line's own entry. 0.7: working copies (the acknowledgement is
    `ts-0101-bd`).
- Carried from `ts-0026-es`: "the pack-profile scope rule (only that pack's own settings)" is `ts-0098-nx`'s.
- **Decision for the owner, with a recommendation: the hash function.** The inputs are git ids, so any hash of them is
  as strong as git. Options:
  - git itself: `git hash-object --stdin` over the canonical text. No crate, one spawn, the same strength as the ids it
    covers (SHA-1, or SHA-256 in a SHA-256 repository);
  - `sha2` 0.10.9 (MIT OR Apache-2.0): already built in this workspace through Tauri, but not a direct dependency of
    `host`. SHA-256 whatever the repository's format. One line in `host/Cargo.toml`;
  - a hand-written SHA-256: no.
  Recommend `git hash-object`: no new dependency, and the weakness (SHA-1 ids) is git's own. If the owner wants SHA-256
  now, `sha2` is a one-function swap: the hash sits behind `fn digest(text) -> String`.
- **Decision for the item, with a recommendation: `protectedPaths` globs.** The plan says "tree hashes of
  `protectedPaths`". Recommend that each pattern's literal prefix folder (up to its first wildcard) is hashed as a tree
  id at the ref. It asks again on any change under that folder, which is more often than a glob match would and never
  less. Alternative: expand the globs against the tree, which needs a matcher that Stage 7 builds anyway (the review
  flag). Recommend the prefix now; Stage 7 may refine it.
- **Decision for the item:** the store keeps the inputs, not only the hash, because a diff needs the old ids. That is a
  list of about a dozen lines per line.
- **Decision for the item:** `trust.yaml` is keyed by the board's root path, not the store's board name, so it does not
  depend on the daemon's database. A moved repo asks again, which is safe.
- The prompt itself (the CLI's, `ts-0102-vu`; the app's, Stage 6a) prints the diff this item builds. A run of repo code
  on an untrusted line is refused by the later stage that runs it; this stage has no hook runner.
- Area: `core/src/trust.rs` (new: the store's types and the diff's), `host/src/trust.rs` (new), `host/tests`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 4a item (revision 23 scope, split 2026-11-16). Items of Stage 4a run one at a time, in id order, because
  they share `core`, `host` and the generated files.
- Owner decisions (2026-11-16, at the split): the recommendations above are taken under the owner's delegation; they
  stay overridable until the item is built.

### Built

`host/src/git_objects.rs`: four new reads, all through the one hardened `command`. `hash_object` (`git hash-object
--no-filters --stdin`: writes nothing, uses the repository's object format), `object_kind` (a blob, a tree, a commit,
or `None` for a missing object), `diff_tree` (`git diff-tree -r -z --raw --no-abbrev --no-renames`, so each file comes
with its mode and both ids, sorted by path; `TreeChange`, `TreeChangeKind`) and `diff_blobs` (`git diff -U0` between
two blob ids, with `--no-ext-diff` and `--no-textconv`, so a `diff.external` in the repository's own config does not
run).

`core/src/trust.rs` (new, pure): `TrustInput` and `canonical_text` (the `name NUL id LF` lines, sorted; a name or id
that is empty or holds a NUL or a newline is refused, and so is one name with two ids); `diff_inputs` and
`InputChange`; the file's types `TrustFile`, `LineTrust`, `PackTrust`; `protected_root` (the folder before a glob's
first wildcard); the diff's shapes `InputDiff`, `DiffDetail`, `FileDiff`, `LineDiff`, `FileChangeKind`; and
`pack_review` with `PackReview::render`.

`host/src/trust.rs` (new): `TrustStore` (`load`, `save`; the folder is a parameter), `Trust` with `line_hash`,
`trust_state`, `store_problem`, `require_trusted`, `trust`, `changes_since_trust`, `diff`, `trust_pack`, `pack_state`,
`pack_changes`, `trust_dev` and `dev_trusted`, `LineSite`, `LineHash`, `StoreError`, `TrustError`. `trust_state`
returns `owner::board::TrustState` itself, so there is no second wire type. The hash is `digest(git, repo, text)`, one
function. No wire type changed, so no `pnpm gen`. No dependency added.

How it behaves:
- The hash covers `LoadedLine::trust_inputs()` and, for each loaded pack (a built-in pack too), the id at the line's
  commit of the folder before each `protectedPaths` glob's first wildcard (`protected <folder>`; a glob that starts
  with a wildcard is `protected .`, the root tree). A folder that is not in the commit is not an input.
- `trust` computes the hash again from the line it is given and compares. The inputs it stores are the ones it just
  hashed.
- `trust.yaml`: `boards.<root>.<ref>.{hash, inputs}`, `packs.<source>.tree`, `dev: [real paths]`. Written through
  `to_yaml`, to `trust.yaml.tmp` (mode set on the open file, so a leftover temp file with open permissions cannot
  leak its mode), `fsync`, `rename`, `fsync` of the folder. The folder is made `0700` (and an existing one closed to
  `0700`). A file that would be over the YAML cap is not written.
- A store that cannot be read is `Untrusted` for `trust_state` and `pack_state`; `require_trusted`, `trust`,
  `trust_pack` and `trust_dev` return the error naming the file, and `trust` leaves the person's file as it is.
- The diff for a changed folder is `diff_tree` plus the changed lines of each added or modified text file (first 20
  files, first 200 changed lines each, the rest counted); a binary file, a symlink, a submodule, a mode-only change and
  a file past the 20 carry a note. A changed file gives its lines; a new file input gives all its lines. When an old
  object is not in the repository the input is `Changed` with `Unavailable` and the reason.

Tests (76 new), written first. Red runs, against `todo!()` stubs: `core/tests/trust.rs` 16 of 19 failed (the three that
passed need only the loader: unknown key, duplicate key, the wire type); `host/tests/trust.rs` 46 of 47 failed (the
one that passed, `the_review_text_of_a_loaded_pack_lists_what_it_runs`, needs only core's `pack_review`); the two unit
tests in `host/src/trust.rs` failed. After the build all pass. The first draft of the added-file diff failed 4 tests
and was fixed (git cannot diff against an empty blob that is not stored, so the lines are read from the blob).
`host/tests/git_objects.rs` (8 new) was written after its methods; I checked the one that guards the config
(`diff_blobs_runs_no_external_diff...`) by removing `--no-ext-diff` and saw it fail, and two of the trust tests the same
way (skipping the protected inputs: 4 failed; dropping the stale check: 2 failed).
- `core/tests/trust.rs`: the text (sorted, order-free, one id per name, bad parts, no forged pair); `diff_inputs`
  (added, removed, changed, name order, empty); the file through `to_yaml` and `load_yaml` (round trip, empty, unknown
  and duplicate key naming the file); lines per board and per root; the owner wire shape; `protected_root` cases;
  `pack_review` and `render`.
- `host/tests/trust.rs`: hash is `git hash-object` of the canonical text, stable, machine-independent; an uncommitted
  edit of `pack.yaml`, a hook, `board.yaml` and `config.yaml` leaves it and a commit changes it; a new and a changed
  view and profile; the lock and the pointer file; a `git:` pack through the lock; what does not change it (README, a
  file outside `.collide`, planning notes, an empty commit, an untracked file, a stray file in `profiles/`); the working
  tree is not the snapshot; protected folder, file, appearing and going, root glob, mapping form; fresh store is
  untrusted and writes nothing; trust with the shown, a stale and a changed-after-the-prompt hash; a changed hash asks
  again and `require_trusted` returns the hash; two lines and two boards; the wire round trip; the store (round trip,
  modes, tightening an open folder, a `0644` file, a leftover temp file, malformed, duplicate and unknown key, over the
  cap, a bad store is untrusted and `trust` leaves it, too large to write); the diff (changed `pack.yaml` lines, new
  and deleted hook, changed `board.yaml`, new view, all added with nothing trusted, a pruned object, a bad id, the 200
  line cap, the 20 file cap, binary and symlink, deterministic); user packs (trusted, untrusted at the next tree, per
  source, diff, all added, bad tree id, bad store, review text from a loaded pack); `dev:` (per folder, a content change
  does not ask again, by real path, inside a workspace also through a link, missing and file); the injected reader.
- `host/src/trust.rs` unit tests: `a_write_that_fails_after_the_temp_file_leaves_the_old_file_and_no_temp_file` and
  `a_write_that_fails_on_a_first_save_leaves_no_file` (a file that fails after N bytes, as a full disk does).
- `host/tests/git_objects.rs`: `hash_object_*` (2), `object_kind_*`, `diff_tree_*` (2), `diff_blobs_*` (3).

Checks, all run in the foreground and green: `cargo build --workspace`; `cargo test --workspace`;
`cargo clippy --workspace --all-targets -- -D warnings`; `cargo fmt --all --check`; `pnpm test`; `pnpm typecheck`;
`pnpm --filter @collide/app build`; `pnpm build:packs`; `pnpm check`. The names check (`checks`) caught the
reference pack's lowercase name in three test paths; they are now `/work/main-repo`.

### Decisions for the owner

1. **The hash is `git hash-object`** (taken at the split), behind `digest(git, repo, text)`. It follows the repository's
   object format, so a SHA-256 repository gives a 64-digit hash and the stored hash is only comparable in the same
   repository. Alternative: `sha2`, always SHA-256. Recommend as built.
2. **A protected glob that starts with a wildcard (`*.lock`, `**/x`) is hashed at the root tree**, so any commit asks
   again. Alternatives: refuse the glob in `check_manifest`; or skip it (never less asking, but then it protects
   nothing). Recommend as built until Stage 7's matcher, and a pack author is told to anchor a glob in a folder. A
   glob with a folder before its wildcard (`scripts/**`) is hashed at that folder; one with no wildcard, at that path.
3. **A built-in pack's `protectedPaths` count too**, though its tree is not an input (0.4). The folder is the repo's,
   not the app's. No built-in pack declares one today. Alternative: skip built-ins. Recommend as built.
4. **One entry per line.** Trusting a new hash replaces the old one, so going back to an older commit asks again.
   Alternative: keep the last N hashes per line. Recommend as built: a person re-trusts what they see.
5. **The diff shows lines for every added or modified text file in a changed folder**, not only `pack.yaml`, up to 20
   files and 200 changed lines each, because a changed hook is what a person most needs to read. Alternative: lines for
   `pack.yaml` only. Recommend as built. An added input that is a file (a new view or profile) shows its lines; an added
   pack and a removed input show only their names, since the pack's review is `pack_review`.
6. **`diff_tree` is run with `--raw --no-abbrev`, not `--name-status`**, so each file comes with its blob ids and no
   second listing is needed. The file list is the same.
7. **A `trust.yaml` that cannot be read is `Untrusted`, and `trust` will not replace it.** The person fixes or deletes
   it. Alternative: move it to `trust.yaml.bad` and start again, which loses nothing but trusts nothing either.
   Recommend as built: the file is the person's record, and a silent rewrite hides a problem.
8. **An existing store folder is closed to `0700`** at the first write. It is Collide's own folder. Alternative: leave
   an existing folder as it is, relying on the file's `0600`. Recommend as built.
9. **A store that would pass the YAML cap is not written** (`TooLarge`), so it is never unreadable. Nothing prunes
   entries for boards that are gone: about a dozen lines per line puts the cap at some hundreds of lines. Recommend
   that `ts-0102-vu` removes a board's entries when the board is removed, and the cap is revisited if it is reached.
10. **`dev:` trust is global, by real path, not per board**, and the workspace paths are an argument (`ts-0101-bd`
    passes the store's recorded ones and the worktree root). The folder must exist. Alternative: per board. Recommend as
    built: the item says "per `dev:` folder".
11. **The atomic write is a copy of `shim.rs`'s `write_file_atomically`** (its error is `ShimError`, and it is private),
    with the mode set on the open file. Alternative: extract one helper into `host`. Recommend extracting it when the
    next writer needs it (`collide config set`, `ts-0098-nx`), not before.
12. **`pack_review` lists more than hooks:** workspace steps, integrations, syncs, gates, `record`, metrics,
    `experiment.prepare`, binding scripts, and the `script`, `wait` and decision scripts of every workflow phase, then
    providers, observers (with their events) and `requires`. Alternative: only the manifest's hook tables. Recommend as
    built: a prompt that leaves out a script that runs is worse than a long one.
13. Plan text to propose for 0.9, Trust: "Each line has one entry, the hash and the inputs it was made from; trusting a
    new hash replaces it. A `protectedPaths` glob is hashed at its folder before the first wildcard, or at the root when
    it starts with one."

- Owner decisions (2026-11-16, at the build): decisions 1 to 13 taken as recommended, under the owner's delegation.
  Decision 13's plan text is applied to 0.9 (Trust). On decision 2: a pack whose glob starts with a wildcard asks
  again on every commit; no fixture or built-in pack has one, and Stage 7's matcher is where it narrows. Carried to
  ts-0102-vu: removing a board's entries with the board (decision 9) and passing the mirror's path to `Trust::diff`
  for `git:` packs.

### Not done, by design

- Nothing calls this yet: `board.trust`, `board.trust_review`, `boards::summary`'s real hash and the CLI prompt are
  `ts-0102-vu`. It prints `Trust::changes_since_trust` and `PackReview::render`; the diff and review types are plain
  Rust in `core` and get their wire form there.
- A diff reads one repository. A `git:` pack's tree is in the mirror, not the line's repository, so its input shows as
  changed with "not in this repository". `ts-0100-pu` and `ts-0102-vu` should call `Trust::diff` with the mirror's path
  for the `pack git:` inputs (or `diff` takes a list of repositories then).
- User packs (`ts-0101-bd`): this item gives `trust_pack`, `pack_state` and `pack_changes`; adding a pack, linking, the
  working-copy acknowledgement (a new `TrustFile` field with `#[serde(default)]`) and recording a `dev:` pack's tree id
  on every load are there. The workspace check is `trust_dev` and `dev_trusted`; `ts-0101-bd` supplies the paths.
- No lock around `trust.yaml`: two writers at once can lose one entry. The daemon is the one writer.
- No prune of old entries, no migration field in the file.
- The `git:` fetch rules, hosts and the settings layers are `ts-0100-pu` and `ts-0098-nx`.
