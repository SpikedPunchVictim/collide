---
id: ts-0096-ph
kind: task
status: active
title: "Board loader: the Collide directory, board, lock and packs"
owners: []
links:
  - { type: blocked-by, to: ts-0095-kp }
---

## Goal

Load a line from its snapshot: the Collide directory, `board.yaml`, `packs.lock`, `config.yaml`, the profiles, the
views (listed, not parsed: Stage 4b) and every pack, each checked, and every refusal that guards the load. One function
in `host`, `load_line(repo, ref, ...)`, that returns a `LoadedLine` or every problem found, each naming the file and
the key. Nothing is read from the working tree, so an edited `pack.yaml`, hook, `board.yaml` or `config.yaml` on a
branch is not used until it is committed at the ref (and trusted: `ts-0097-kw`).

- **The Collide directory** (0.7). At the ref, `.collide` is a tree (the directory), or a blob (the pointer file, parsed
  with the hardened loader and `board::check_pointer`: `dir: tools/collide`). The directory it names must exist at the
  ref and be a tree. A `dir` that leaves the repo is refused by `check_pointer`, and one that names a blob, a missing
  path or a symlink is refused here, naming the pointer. With neither, the line has no Collide directory and loads with
  the built-in packs only (0.11, first run works with no pack).
- **The files**, read as blobs at the ref: `board.yaml` (`check_board`), `packs.lock` (`check_lock`), `config.yaml`
  (`check_config_file`, then `repo_file_refusals`: the layers are `ts-0098-nx`'s), each file under `profiles/`
  (`check_profile`), and the names and blob ids of `views/*.yaml`. A file over the YAML cap, with an anchor or a
  duplicate key, is the loader's own refusal (`yaml.rs`).
- **The packs**, in `board.yaml`'s order. For each source:
  - `repo:<path>`: the tree id at `<ref>:<path>` is the pin (computed, never locked);
  - `git:<locator>`: the pin is the lock's `tree`, and the source must have a lock entry whose `source` is the same
    text, else the load fails naming the pack. The objects come from the mirror (`ts-0100-pu`); until then a `git:` pack
    whose entry is already in the cache loads, and one that is not is a load problem that names the pack and the commit;
  - `dev:` in `board.yaml` is refused (`check_board` already does; the loader keeps its message).
  Each pack is materialized (`ts-0095-kp`), its `pack/pack.yaml` is read with the hardened loader, `check_manifest`
  runs, and its `name` must be unique among the line's packs and not equal a built-in pack's (`collide`,
  `collide/plan`). The id and template-field rules (`resolve::is_safe_token`) are checked on every role, workflow and
  prompt name the manifest declares.
- **The requirements** (0.7). `requires.sdk` is a range of `@collide/pack-sdk`, checked against the SDK's version
  (`ensure_sdk`'s). `requires.tools` is each tool found on the captured `PATH` (`builtin_packs::Environment`) with its
  version in range when one is given: the tool's `--version` is run once with a short timeout and the first
  `major.minor[.patch]` in its output is the version. A missing tool or one out of range fails the load naming the pack
  and the tool, and carries the pack's `hint`. A range is `>=`, `>`, `<=`, `<` or `=` followed by a version, joined by
  spaces (`">=0.1 <0.2"`), checked by a pure function in `core` (`version_range`); a form it does not know is refused
  naming it.
- **The built-in packs** (0.4, 0.6). `BuiltinPacks` gives `collide` and `collide/plan` from the app, marked `Builtin`:
  no tree id, no cache entry, no trust. They come first in the order.
- **Pack data** (0.7). `pack_data_dir(data_root, board, pack)` gives the folder `COLLIDE_PACK_DATA` names, one per pack
  per board, made on first use with mode `0700`. The daemon passes it in `ts-0102-vu`.

`LoadedLine` holds: the Collide directory path, the checked `Board`, `PackLock`, the repo's `ConfigFile`, the profiles,
the list of view files, and for each pack its name, origin, source, pin, cache entry folder and manifest. It also gives
`trust_inputs()`: the list of (name, git id) pairs the trust hash covers (`ts-0097-kw` hashes it).

## Success criteria

- Edit `pack.yaml`, a hook, `board.yaml` and `config.yaml` in the working tree without committing: the load shows the
  committed content for each. Commit them at the ref and the load shows the new content, with a different
  `trust_inputs()`.
- In a repo whose `.collide` file says `dir: tools/collide`, the board, profiles, views and `config.yaml` load from
  `tools/collide/`, and a `repo:` pack path outside it still loads. A `dir` that leaves the repo, is absolute, names a
  missing path or a file is refused, naming the pointer file and the `dir`.
- A pack that commits `node_modules/@collide/pack-sdk`, or a symlink, is refused at load and the message names the
  path (from `ts-0095-kp`, surfaced with the pack's name).
- A `git:` pack with a lock entry loads from the cache by its pinned tree id and not by the remote's ref. A `git:` pack
  with no lock entry fails naming it. A lock entry whose `source` text differs from `board.yaml`'s fails.
- A manifest with a role id that fails the safe-token rule, or a template field that does, fails naming the pack file
  and the key.
- Two packs with one name fail naming both sources. A pack named `collide` fails naming the built-in.
- `requires.tools: [{ name: tool-that-is-missing, hint: "install it" }]` fails the load naming the pack, the tool and
  the hint. `requires.tools: [{ name: node, version: ">=999" }]` fails naming the version found. A fake tool on a temp
  `PATH` that prints `v22.18.0` meets `>=22.18`.
- `requires.sdk: ">=9.0"` fails against SDK 0.1.0, naming both. `">=0.1 <0.2"` loads.
- A line with no `.collide` loads with the two built-in packs and no problems.
- All problems of a load are returned together (a bad board and a bad pack in one load give both), in file order.
- `pack_data_dir` is distinct per pack and per board, is made once, and is `0700`.
- `version_range` accepts the five operators, refuses `^1`, `~1`, `1.x` and `*` naming the form, and compares
  numerically (`>=1.10` rejects `1.9`).
- No new crate.

## Tests

Write these first, over repositories made by `git` in a temp root with fixtures from `fixtures/` (the `board`,
`config`, `locks`, `manifests`, `packs`, `pointer` and `profiles` folders), the cache and a stand-in SDK in the same
root:

- the snapshot-not-workspace test for each of the four files, uncommitted and then committed;
- the pointer cases above, and a `.collide` that is a symlink;
- the pack cases: `repo:`, `git:` with a primed cache, `git:` with no lock entry, a mismatched lock `source`, a
  materializer refusal surfaced with the pack's name, a bad manifest, a bad id, a bad template field;
- the name cases: two equal, one equal to a built-in;
- `requires`: a missing tool, a tool in range and out of range (fake tools that print their version), a bad range form,
  the SDK range;
- `version_range` as a pure test in `core` (a table);
- no `.collide`; several problems at once; the order of the problems;
- the built-in packs: present, first, `Builtin`, with no cache entry made;
- `pack_data_dir`: distinct, once, `0700`, a name that is not a safe token refused.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4a: Minimal pack loading (the Collide directory; `pack.yaml`; `board.yaml` and the lock; the `requires`
    checks; the built-in `collide` pack); Pack data; the tests ("snapshot-not-workspace"; "id and template-field
    validation"; "the pointer file").
  - 0.4: The built-in `collide` pack. 0.6: Materialization; Built-in packs. 0.7: The Collide directory; `requires`;
    Pack sources; Pack data.
  - 0.9: YAML is untrusted input.
- Carried from `ts-0026-es`: the `git:` locator grammar and "comparing a pin with a fetched tree" are `ts-0100-pu`'s;
  this item reads the lock's `tree` as the pin and compares nothing with a remote.
- **Decision for the item, with a recommendation: where `requires.tools` runs.** The loader runs each tool's
  `--version` through `run_bounded` with a 5 s timeout, the captured `PATH` and a cleared environment otherwise.
  Recommend that: a pack's load must say what is missing before a run does. Alternative: check only that the file
  exists and leave the version to the run (cheaper, but "node >= 22.18" is then unchecked).
- **Decision for the item:** the version range is a small pure function in `core`, not the `semver` crate: `semver` is
  in `Cargo.lock` through other crates, but its requirement syntax joins comparators with commas, and a pack writes
  `">=0.1 <0.2"`. Recommend hand-written (about 60 lines, a table test).
- **Decision for the item:** a pack's `name` in `pack.yaml` is its identity. `board.yaml` entries carry only a source,
  so the name is read from the manifest; a manifest whose `name` is not a safe token is refused.
- A line is one `ref`: this item loads one line. Several lines and per-line overrides are Stage 14 (0.12).
- Views are listed by name and blob id only. Their parse and the CEL check are Stage 4b's.
- Area: `host/src/board_load.rs` (new), `core/src/version_range.rs` (new, pure), `host/src/builtin_packs.rs` (the
  built-in list, if it needs a listing), `host/tests`, `fixtures/`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 4a item (revision 23 scope, split 2026-11-16). Items of Stage 4a run one at a time, in id order, because
  they share `core`, `host` and the generated files.
- Owner decisions (2026-11-16, at the split): the recommendations above are taken under the owner's delegation; they
  stay overridable until the item is built.
