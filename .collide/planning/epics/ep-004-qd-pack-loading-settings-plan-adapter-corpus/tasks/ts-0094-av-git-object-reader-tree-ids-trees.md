---
id: ts-0094-av
kind: task
status: done
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

### Built

One module in `host` that reads git objects, and a fixture helper for its tests. No crate was added. No wire type
changed, so `pnpm gen` was not needed. Nothing reads a working tree and nothing is written outside the scratch folder
of `tree_of_folder`.

- `host/src/git_objects.rs` (new), listed in `host/src/lib.rs`:
  - `GitObjects`, with `new()` (git found on the process's `PATH`), `with_path`, `with_timeout` and `with_scratch_dir`,
    which tests use to put a different `git` first, shorten the timeout and aim the temporary folder. Its methods are
    `tree_at`, `list`, `read_blobs`, `blob_at` and `tree_of_folder`. The module's free functions of the same names
    call `GitObjects::new()`, so later items write `git_objects::list(repo, tree)` as their text says.
  - `GitObjects::command` is the one place that sets up git: `env_clear()`, then `PATH`, `LC_ALL=C`,
    `GIT_TERMINAL_PROMPT=0`, `GIT_OPTIONAL_LOCKS=0`, `GIT_NO_REPLACE_OBJECTS=1`, `GIT_CONFIG_GLOBAL` and
    `GIT_CONFIG_SYSTEM` as `/dev/null`, plus `GIT_CONFIG_NOSYSTEM=1` and `GIT_ATTR_NOSYSTEM=1`, and `GIT_DIR` for the
    repository. `HOME` is not set. Each call runs through `bounded_run::run_bounded` with `GIT_TIMEOUT` (30 s).
  - `GitError`: `Refused` (an argument git never saw), `Run` (did not start, or timed out), `Failed` (command, exit
    code, `bounded_run::tail` of stderr), `BadReply`, `NoSuchRev`, `NoSuchPath`, `NotABlob`, `MissingObject`,
    `WrongObject`, `NotUtf8`, `TooManyEntries`, `BlobTooLarge`, `TreeTooLarge` and `Folder`. Each names its subject.
  - Constants a test can read: `MAX_TREE_ENTRIES` (2,000), `MAX_BLOB_BYTES` (8 MiB), `MAX_TREE_BYTES` (32 MiB),
    `GIT_TIMEOUT`.
  - `tree_at(repo, rev, path) -> TreeAt { id, kind }`: two spawns. `rev-parse --verify --quiet --end-of-options
    <rev>^{commit}` (exit 1 is `NoSuchRev`), then `cat-file --batch-check` with `<commit>:<path>` on stdin, so a path
    is never an option. A `rev` that is empty, starts with `-`, or holds a NUL or newline is refused before git runs.
    A path is refused for a `..` or `.` part, an empty part, a leading `/`, a NUL or a newline. The empty path is the
    commit's own tree.
  - `list(repo, tree) -> Vec<TreeEntry { mode, kind, id, path, size }>`: `ls-tree -r -z -l --full-tree`. `tree` must
    be a full lower-case hex id. Sorted by path. Refuses on the entry count, a blob over the cap (naming its path), the
    total size, and a path that is not UTF-8 (named lossily).
  - `read_blobs(repo, ids)`: ids are checked as hex, then sent 200 at a time to one `cat-file --batch`. Each reply's
    id, type and size are checked against the ask and the bytes that came. A missing object, a non-blob and a blob over
    the cap are errors naming the id. No ids runs no git.
  - `blob_at(repo, rev, path) -> Option<Vec<u8>>`: `None` for a missing path, an error for a folder, and an error naming
    the path for a blob over `collide_core::yaml::MAX_YAML_BYTES`. The size comes from `tree_at`'s reply, so an
    oversized file is not read.
  - `tree_of_folder(folder)`: makes `collide-tree-<pid>-<n>-<nanos>` (mode 0700, by `create`, so it is always new) in
    the scratch folder, runs `git init --bare --template= --object-format=sha1` there, then `add --all --force` with
    `GIT_DIR`, `GIT_WORK_TREE` and `GIT_INDEX_FILE`, then `write-tree`. A guard removes the folder on every exit. The
    add is given `core.filemode=true`, `core.symlinks=true`, `core.ignorecase=false`, `core.precomposeunicode=false`,
    `core.autocrlf=false` and `--attr-source <empty tree>`, so the file system, the user and a `.gitattributes` in the
    folder cannot change a hashed byte.
- `host/tests/common/git_repo.rs` (new), listed in `host/tests/common/mod.rs`: `GitRepo` (a checkout in a `TempRoot`
  with `write`, `write_executable`, `symlink`, `remove`, `commit_all`, `blob`, `tree_of` for raw-byte names, `git`),
  and `git_command`, `git_bytes` and `git_text`. Every git call clears the environment and sets `HOME` and
  `COLLIDE_DAEMON_ROOT` to the temp root, `GIT_CONFIG_NOSYSTEM=1`, `GIT_CONFIG_GLOBAL=/dev/null`, and a fixed author,
  committer and date.
- `host/tests/git_objects.rs` (new, 56 tests). No test touches the real home or the real git config.
  - `tree_at`: `tree_at_a_nested_path_equals_rev_parse`, `tree_at_the_empty_path_is_the_commit_tree`,
    `the_same_files_at_two_commits_give_the_same_id`, `tree_at_a_file_says_blob`,
    `tree_at_a_missing_path_is_an_error_naming_the_path`, `tree_at_a_missing_ref_is_an_error_naming_the_ref`,
    `a_rev_that_starts_with_a_dash_is_refused_and_git_does_not_run`,
    `a_path_that_could_leave_the_tree_or_break_the_framing_is_refused`,
    `a_bare_repository_is_read_as_a_checkout_is`.
  - Snapshot: `an_uncommitted_edit_a_deleted_file_and_an_untracked_file_do_not_change_what_is_read`.
  - `list`: `list_keeps_the_modes_git_writes`, `list_gives_a_blob_its_id_and_size`,
    `list_returns_a_name_with_a_space_a_newline_or_a_quote_whole`,
    `list_refuses_a_name_that_is_not_utf8_naming_it_lossily`, `list_is_sorted_by_path_with_folders_flattened`,
    `list_of_an_empty_tree_is_empty`, `list_refuses_a_tree_over_the_entry_cap_naming_the_count`,
    `list_allows_a_tree_of_exactly_the_entry_cap`, `list_refuses_a_blob_over_the_size_cap_naming_its_path`,
    `list_refuses_a_tree_over_the_total_size_cap`,
    `list_refuses_a_tree_that_is_not_a_full_hex_id_and_git_does_not_run`,
    `list_of_a_tree_id_that_is_not_there_is_an_error`.
  - `read_blobs`: `read_blobs_gives_the_bytes_git_shows_for_text_and_for_binary`,
    `read_blobs_with_no_ids_is_empty_and_runs_no_git`, `read_blobs_returns_one_blob_in_the_order_asked`,
    `read_blobs_reads_1000_blobs_of_4_kib_in_order`, `read_blobs_reads_5000_blobs_in_order`,
    `read_blobs_with_a_missing_object_names_the_id`, `read_blobs_with_a_tree_id_is_an_error_naming_the_id`,
    `read_blobs_refuses_an_id_that_is_not_a_full_hex_id`, `read_blobs_refuses_a_blob_over_the_size_cap_naming_the_id`,
    `read_blobs_gives_the_original_when_a_replace_ref_swaps_a_blob`.
  - `blob_at`: `blob_at_a_file_gives_its_bytes`, `blob_at_a_missing_file_is_none`,
    `blob_at_a_missing_ref_is_an_error`, `blob_at_a_folder_is_an_error_naming_the_path`,
    `blob_at_a_file_over_the_yaml_cap_is_an_error_naming_the_path`.
  - `tree_of_folder`: `a_folder_of_plain_files_has_the_id_git_gives`,
    `an_executable_file_changes_the_id_and_matches_git`, `an_ignored_file_and_a_node_modules_folder_are_counted`,
    `a_symlink_is_hashed_as_a_link_and_not_followed`, `nested_folders_match_and_an_empty_folder_is_absent`,
    `a_folder_with_no_files_has_the_empty_tree_id`,
    `two_folders_with_equal_files_have_the_same_id_with_or_without_a_repository`,
    `a_folder_has_the_id_of_the_commit_that_holds_its_files`,
    `a_gitattributes_file_does_not_change_the_bytes_that_are_hashed`,
    `a_repository_config_that_names_an_fsmonitor_is_never_run_when_a_folder_is_hashed`,
    `a_repository_config_that_names_an_fsmonitor_is_never_run_when_objects_are_read`,
    `a_folder_that_is_not_there_or_is_a_file_is_an_error_naming_it`,
    `tree_of_folder_writes_only_in_its_scratch_folder_and_removes_it`,
    `tree_of_folder_removes_its_scratch_folder_when_git_fails`.
  - How git is run: `git_is_run_with_a_clean_environment_and_the_hardening_variables` (a recording `git` first on
    `PATH`; the exact set of variable names, so nothing from the test's own environment leaks),
    `the_add_of_a_folder_hash_names_the_private_repository_and_the_folder`,
    `a_git_that_hangs_fails_within_the_timeout_naming_the_command`,
    `a_git_that_exits_non_zero_gives_the_command_the_code_and_the_stderr_tail`,
    `a_missing_git_is_an_error_naming_the_command`.

**Red.** The module was first a stub whose methods return an error. All 56 tests failed against it. After the code
went in, 55 passed and `list_keeps_the_modes_git_writes` failed because the fixture's `git add --all` dropped the
submodule entry; the fixture was fixed, not the code. Three changes to the code were then tried to see that the tests
catch them, and each was undone:

- without `GIT_NO_REPLACE_OBJECTS`, three tests failed (the replace-ref test and the two that read the environment);
- without `--attr-source`, `a_gitattributes_file_does_not_change_the_bytes_that_are_hashed` failed;
- with 5,000 ids to one `cat-file` process, the 1,000 and 5,000 blob tests hung and had to be killed (see decision 1).

**Checks** (run after the last edit): `cargo build --workspace` ok; `cargo clippy --workspace --all-targets -- -D
warnings` ok (it first flagged two `.clone()` slices in the tests, fixed); `cargo fmt --all --check` ok. The others
are in the report to the reviewer.

### Decisions for the owner

1. **200 ids to a `cat-file` process, not 500.** The item says 500. `run_bounded` writes all of stdin before it reads
   any output, and that write is not under its timeout. If the input does not fit the pipe while git is blocked on a
   full output pipe, both sides wait for ever. With 5,000 ids to one process the tests hung until killed, and the
   30 s timeout never fired. 500 ids passed here (macOS grows its pipes), but 500 SHA-256 ids are 32 KB, so 200 (13 KB
   at most) leaves a margin. The cost is more spawns: 25 for 5,000 ids. Recommend 200. Alternative: make `run_bounded`
   write stdin on a thread so the timeout covers it, then raise the chunk. That changes a shared helper that another
   item owns, so it is left alone. It is a real gap in `run_bounded` for any later caller with a large stdin.
2. **`GitObjects` struct with free-function wrappers.** The item names free functions. They could not be tested for the
   hanging `git`, the fake `git` and the scratch folder without a way to change `PATH`, the timeout and the scratch
   folder, and the test process's own environment cannot be changed safely while tests run in parallel. So the
   functions call `GitObjects::new()`. Alternative: only free functions, and test the hang and the environment by
   starting a child process. Recommend the struct.
3. **`list` uses `ls-tree -l` and `TreeEntry` has a `size`.** The item says `ls-tree -r -z --full-tree`. The size caps
   (a blob 8 MiB, a tree 32 MiB) need sizes, and `-l` gives them in the same call. Alternative: ask `cat-file
   --batch-check` for the sizes, one more spawn. Recommend `-l`. The materializer can use `size` too.
4. **Caps as the item recommends.** 2,000 files, 8 MiB a blob, 32 MiB a tree; the count is checked first, then each
   blob, then the total. A tree of exactly 2,000 is allowed (tested).
5. **`tree_of_folder` ignores the folder's `.gitattributes`** (`--attr-source` with the empty tree). Without it, an
   `eol`, `ident` or `working-tree-encoding` rule would change the bytes hashed, and the re-hash of a materialized pack
   could not match its tree id. The item does not mention attributes. This needs git 2.40; an older git fails the add
   with an error and the id is not computed. Alternative: set `GIT_ATTR_SOURCE` in the environment, which an older git
   ignores without a word. Recommend the flag, because it fails loudly. The plan or the docs may want to say "git 2.40
   or newer" (Apple Git here is 2.50).
6. **The id is SHA-1.** The item says a SHA-256 repository gives a SHA-256 id. The scratch repository is made with
   `--object-format=sha1` so a user's git config cannot change it, and it matches the lock's ids. `list`, `read_blobs`
   and `tree_at` accept 64-digit ids, so a SHA-256 repository can be read but not compared with a folder's id.
   Recommend SHA-1 for now.
7. **`GIT_DIR` is the way the repository is given**, not a working directory, so `git` never searches upward. The
   `repo` argument is the `.git` folder of a checkout or a bare mirror. A caller that holds a checkout's root passes
   `<root>/.git`. A linked worktree's `.git` is a file; git follows it, but this was not tested.
8. **Stricter than the item on paths.** A `.` part, an empty part (`a//b`) and a trailing `/` are refused as well as
   `..`, because git reads `rev:./x` relative to its working folder. The empty path means the root tree.
9. **`tree_at` on a submodule path says "no such path".** `cat-file` reports a submodule commit as missing. The message
   says so. Callers that need to know a submodule is there use `list`, which shows mode `160000`.
10. **`GIT_CONFIG_NOSYSTEM`, `GIT_ATTR_NOSYSTEM` and no `HOME` are added** to the item's list. `GIT_CONFIG_SYSTEM` is
    ignored by a git older than 2.32, and `GIT_CONFIG_GLOBAL` does not stop git reading the user's attributes file
    below `HOME`. Recommend keeping them.
11. **Scratch folder is the system's temp folder** (`std::env::temp_dir()`), as other host code does, and a test can aim
    it elsewhere. Alternative: a folder under the cache. Recommend the system's: it is one temporary folder, removed in
    the same call.

- Owner decisions (2026-11-16, at the build): decisions 1 to 8 taken as recommended, under the owner's delegation.
  Decision 1's alternative was also done in review: `host/src/bounded_run.rs` `run_bounded` now writes stdin on a
  thread of its own, so a large input can neither block the reads nor escape the timeout
  (`host/tests/bounded_run.rs`, two tests, red before the change). The chunk stays at 200 ids, now to bound what one
  reply holds. `tree_of_folder` needs git 2.40 or newer for `--attr-source`; an older git fails naming the command.
  The gates also found `host/tests/acp_phases.rs` failing about one run in three, from before this item: a second ACP
  turn with no answer. Fixed in review, and noted under `ts-0092-tw` decision 6.

### Not done, by design

- No materializer, no refusal of symlinks, submodules, `node_modules` or `package.json` (`ts-0095-kp`). `list` reports
  mode `120000` and `160000` and the materializer decides.
- No lock, no trust hash, no `git:` fetch or mirror (`ts-0096-ph`, `ts-0097-kw`, `ts-0100-pu`). Fetch builds its own
  environment, as the item says.
- `read_blobs` holds one chunk's reply in memory and checks a blob's size only when it reads the reply's header. The
  caps are enforced before any read only for ids that come from `list`. A caller with ids from elsewhere gets the cap
  after the bytes are read.
- `blob_at` does not look at the mode: a symlink named `board.yaml` returns its target text. The loader (`ts-0096-ph`)
  can call `list` or `tree_at` if it needs to refuse a link.
- `tree_of_folder` of a folder that holds another repository (a `.git` below the top) records that folder as a
  submodule, as `git add` does. The materializer refuses submodules, so its re-hash never meets one.
- A test of the user's own `~/.gitconfig` is not made: the reader runs git with a cleared environment, so a test would
  have to change the process's `HOME`, which is unsafe while tests run in parallel. The recording-`git` test shows that
  `GIT_CONFIG_GLOBAL` is `/dev/null` and `HOME` is absent, and the fsmonitor tests show that a repository's own config
  runs nothing.
- `host/src/boards.rs`, `checkout.rs` and `commit.rs` are not changed.
