---
id: ts-0094-av
kind: task
status: ready
title: "Git object reader: tree ids, trees and blobs"
owners: []
links:
  - { type: blocked-by, to: ts-0093-am }
---

## Goal

Everything Stage 4a loads is read from git objects at a ref, never from the working tree (0.6, 0.7, S4). This item is
the reader: one module in `host` that answers four questions about a repository, and nothing else.

- **What is the tree id** of `<ref>:<path>`? A pack's pin is that id (0.7, Pack sources). `tree_at(repo, rev, path)`
  resolves `rev` to a commit first, then the path in that commit, and says whether the entry is a tree or a blob.
  A `rev` that starts with `-` is refused before git runs. A path with a `..` part, a leading `/`, a NUL or a newline
  is refused.
- **What is in this tree?** `list(repo, tree)` runs `ls-tree -r -z --full-tree` and returns the entries (mode, kind, id,
  path), sorted by path. Modes are kept as git writes them: `100644`, `100755`, `120000` (symlink), `160000`
  (submodule). A path that is not UTF-8 is refused, naming it lossily. The list is capped (see the decision below).
- **What does this blob hold?** `read_blobs(repo, ids)` reads many blobs through one `cat-file --batch` process and
  checks each reply's type and size against what was asked. `blob_at(repo, rev, path)` reads one file, with the
  hardened YAML loader's size cap, for `board.yaml`, `config.yaml` and the like.
- **What would these files' tree id be?** `tree_of_folder(folder)` is for `dev:` packs and for the materializer's
  re-hash (`ts-0095-kp`): the id git would give the folder's files, computed by git itself in a temporary bare
  repository (`git init --bare` in a temp folder, then `git add --all --force` with `GIT_DIR`, `GIT_WORK_TREE` and
  `GIT_INDEX_FILE` pointing there, then `write-tree`). The folder's own repository, its config and its ignore files
  are never consulted; `--force` is so that a `node_modules` or an ignored file is counted, then refused by the
  materializer, instead of silently dropped.

Every git call goes through one function that:

- runs `git` through `bounded_run::run_bounded`, with a timeout (30 s; a hang is an error naming the command);
- clears the environment and sets `PATH`, `LC_ALL=C`, `GIT_TERMINAL_PROMPT=0`, `GIT_OPTIONAL_LOCKS=0`,
  `GIT_NO_REPLACE_OBJECTS=1` (a replace ref must not change the content under a tree id) and `GIT_CONFIG_GLOBAL` and
  `GIT_CONFIG_SYSTEM` set to `/dev/null`, so the user's git config cannot change what is read;
- gives `GitError` with the command's name, the exit code and the tail of stderr (`bounded_run::tail`).

The reader takes a git directory, so a bare mirror (`ts-0100-pu`) is read as a checkout is. It reads no file from a
working tree.

## Success criteria

- `tree_at` for a nested path equals `git rev-parse <ref>:<path>`. The same files at two commits give the same id. A
  path that is a file says blob, and a missing path or ref is an error naming it.
- Edit a tracked file in the working tree without committing, and `list` and `read_blobs` at the ref still give the
  committed content (the snapshot is not the workspace).
- `list` keeps modes: an executable file `100755`, a symlink `120000`, a submodule `160000`. A file whose name holds a
  space, a newline or a quote comes back whole (the `-z` form); a name that is not UTF-8 is refused.
- `read_blobs` returns bytes equal to `git show` for text and for a binary file with NULs, in the order asked, for 1,
  1,000 and 5,000 ids (chunked so the child's pipes cannot fill). A missing object is an error naming the id.
- A blob over the size cap is refused naming its path. A tree over the entry cap is refused naming the count.
- `tree_of_folder` equals `git write-tree` over the same files added by a plain `git add -A` in a repository of its
  own. It counts a file in `.gitignore` and a `node_modules` folder, ignores `.git`, and gives the same id for the same
  files in two folders. A folder with an executable file gives `100755` for it.
- A `rev` that starts with `-` is refused and git does not run. A path with `..` is refused.
- A `git` that hangs (a script first on `PATH`) fails within the timeout, naming the command.
- The reader sets `GIT_NO_REPLACE_OBJECTS`: with a replace ref that swaps a blob, `read_blobs` gives the original.
- A user's `~/.gitconfig` with `core.fsmonitor` or an alias for `ls-tree` changes nothing and runs nothing.
- Nothing is written outside the temp folder of `tree_of_folder`, which is removed afterwards even on error.
- No new crate.

## Tests

Write these first. Each builds its repositories with `git` in a temp root (a helper in `host/tests/common`, with a fixed
author and date so ids are stable), and clears the environment as the test rule says:

- `tree_at`: the nested path, the root, two commits with equal content, a blob path, a missing path, a missing ref,
  a ref starting with `-`, a path with `..`;
- the working tree is not the snapshot: an uncommitted edit, a deleted tracked file and an untracked file;
- `list`: modes, odd names (space, newline, quote, non-UTF-8 on a platform that allows it), sorted order, the entry cap,
  and an empty tree;
- `read_blobs`: text, binary, order, 1,000 and 5,000 ids, a missing id, a wrong-type id (a tree id), the size cap;
- `blob_at`: a file, a missing file (`None`), a directory (an error), a file over the YAML cap;
- `tree_of_folder` against `git write-tree` for: plain files, an executable, an ignored file, a `node_modules` folder, a
  symlink, nested folders, an empty folder (absent), and two folders with equal files; and a folder inside a repository
  whose `core.fsmonitor` is set to a script that would leave a marker file (the marker must not appear);
- the environment: a fake `git` first on `PATH` that records its environment, to check the variables above are set and
  nothing from the caller's environment leaks;
- the hanging `git`, and a `git` that exits non-zero with stderr (the message carries the tail).

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4a: Minimal pack loading (snapshot materialization; "files come from git objects"); Pack sources and scope
    ("a `dev:` folder's tree id equals `git write-tree` over the same files").
  - 0.6: Materialization. 0.7: Pack sources (the lock's tree id); Snapshot rule.
  - S4 record: the recipe (steps 1 to 5) and the hazards of `git archive`.
  - 0.9: YAML and frontmatter are untrusted input (the size cap).
- **Decision for the item, with a recommendation: caps.** The plan names none. Recommend a pack tree of at most 2,000
  files, a blob of at most 8 MiB and a whole tree of at most 32 MiB, as constants a test can read; a tree over them is
  refused with its count or size. A line's own `.collide/` folder is listed with `list` too, so the caps apply to it.
  The numbers are generous for packs (S4's were 184 files) and stop a tree that would fill the cache.
- **Decision for the item, with a recommendation: how `tree_of_folder` hashes.** It uses git in a private bare
  repository, so the id is git's own, and a SHA-256 repository gives a SHA-256 id. Alternatives: hash the bytes in
  process (a SHA-1 and a SHA-256 implementation: a new crate, or hand-written), or `git hash-object` per file with
  `mktree`. Recommend the private repository: no crate, git computes it, and the cost (two or three spawns, about
  60 ms) falls on a cache miss and on `dev:` packs only.
- **Decision for the item:** the reader runs git with the user's config switched off, but `git:` fetches need the user's
  credentials (`ts-0100-pu`), so the fetch builds its own environment and does not use this function.
- Chunk `cat-file --batch` input at 500 ids a call: `run_bounded` writes all of stdin before it reads, so a very long
  list could fill the pipe both ways.
- Do not change `host/src/boards.rs`, `checkout.rs` or `commit.rs`, which have their own small git helpers for other
  jobs.
- Area: `host/src/git_objects.rs` (new), `host/src/lib.rs` (module), `host/tests/common` (the repository helper), a test
  target or `host/tests/git_objects.rs`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 4a item (revision 23 scope, split 2026-11-16). Items of Stage 4a run one at a time, in id order, because
  they share `core`, `host` and the generated files.
- Owner decisions (2026-11-16, at the split): the recommendations above are taken under the owner's delegation; they
  stay overridable until the item is built.
