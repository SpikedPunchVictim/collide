---
id: ts-0097-kw
kind: task
status: ready
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
