---
id: ts-0191-nv
kind: task
status: done
title: "Boards: stable ids, the main checkout, a missing board and locate"
owners: []
links:
  - { type: blocked-by, to: ts-0216-ve }
---

## Goal

A board keeps one id for its life, which a lost store never gives to another repository. A worktree opens as a line
of its repository's board, not as a board of its own. A board whose folder was moved shows as missing, deletes
nothing, and comes back as it was after "locate". The board menu gets the system folder picker.

What exists.
- `host/src/scratch_store.rs` names boards `board-<n>` by count (`BOARD_PREFIX`). `ts-0075-hj` carried two fixes
  here: an id a lost store never reuses (personal views and other files are keyed by it, `UserScope::board_dir`),
  and the board root becoming the main checkout, so a linked worktree is a line.
- `board.open` keys a board by the checkout's own top level. `host/src/checkout.rs` `main_checkout` finds the main
  checkout and is not used there.
- `host/src/corpus_service.rs` notices a board whose folder is gone and looks again every `GONE_PERIODS` rechecks.
  `BoardSummary` (`core/src/owner/board.rs`) has `id`, `root`, `lines` and `sources_unread`, and no missing flag.
  No method moves a board to a new folder.
- From `ts-0179-ke`: the board menu, with a typed folder path.

What this adds.
- New boards get a random id (`board-` and a short random suffix, checked unique in the store). Existing
  `board-<n>` ids are kept, so nothing on disk moves.
- `board.open` on a linked worktree opens the main checkout's board, with the worktree as a line of it. Opening the
  main checkout after a worktree finds the same board.
- `BoardSummary.missing`: the board's folder is gone. The board menu's row says "folder not found" (note 15).
  Selecting it shows the missing page (the mock's `missingHtml`): where Collide looked, "Nothing has been deleted",
  its tasks are kept, and "Locate the folder…". Remove from Collide is shown disabled ("Arrives with board removal").
- `board.locate { board, path }`: checks the folder is the same repository (below), then moves the board's root
  there and keeps its id, scratch tasks, trust entry, views and pack data. A different repository is refused with
  the reason.
- The folder picker: "Open a folder…" and "Locate the folder…" open the system folder picker in the app, through a
  shell-local method (`shell.pick_folder`), answered in `shell.rs` like `shell.open_external`. The bridge and the
  fake-owner page keep the typed path box.

## Success criteria

- **A missing board** (0.2; revision 22): a board whose repository folder was moved shows as missing with
  "locate", deletes nothing, and after "locate" shows its tasks as they were. Its Up next list is Stage 11's (see
  the split's text to apply).
- A lost store (the store file removed) never gives an existing personal-views folder's id to another repository.
- `board.open` on a worktree returns the main checkout's board, and its lines include the worktree.
- Locating a different repository is refused, and the board stays missing.

## Tests

Write these first.

- `host/tests/`: ids (new random, old kept, never reused after a lost store); a worktree opened first and then the
  main checkout; a moved folder becomes missing, its scratch tasks and personal views kept; locate to the moved
  folder; locate to another repository refused.
- `app/test/`: the menu row and the missing page's words.
- `app/browser/boards.spec.ts` (bridge): the test moves a temp repo's folder; the row says "folder not found";
  locate through the typed path; the tasks show again.
- `app/src-tauri/src/shell.rs` unit test: `shell.pick_folder` is answered in the shell through a fake picker and
  never forwarded.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 6a: Line selector ("A missing board's row says 'folder not found'"). Success Criteria: A missing board.
  - 0.2: After a restart, Boards and lines (revision 22). 0.11: Line selector. 0.12: lines.
- Design: notes 1 and 15. Mock functions: `missingHtml`, `locateModal`, `popHtml` (the menu row).
- Carried: `ts-0075-hj` (ids a lost store never reuses; the board root as the main checkout).
- **Owner's choice, recommendation taken (2026-11-24): the folder picker.** Recommend `tauri-plugin-dialog` 2.x, the
  release that matches tauri 2.12 (a new crate; the builder pins it in `Cargo.lock`), called from Rust in `shell.rs`, so
  it needs no capability in the webview. Alternatives: `osascript -e 'choose folder'` from the shell (no dependency; a
  script call, macOS only); or the typed path box in the app as in the browser pages.
- **Decision for the item, with a recommendation: the same repository.** Recommend comparing the root commits
  (`git rev-list --max-parents=0 HEAD`) of the old record and the new folder, recorded on first open. Alternative: the
  `origin` URL; it changes with a fork or a rename and is absent for a local repo.
- **Decision for the item, with a recommendation: the id scheme.** Recommend random ids for new boards only.
  Alternative: rename every `board-<n>` and move its files; more risk for no gain.
- Out of scope: the old path chip and Move (Stage 12); removing a board; the Up next list (Stage 11).
- Area: `host/src/scratch_store.rs`, `host/src/board*.rs`, `host/src/checkout.rs`, `host/src/corpus_service.rs`,
  `host/src/daemon.rs`, `core/src/owner/board.rs`, `schemas/` and the generated files, `app/src-tauri/src/shell.rs`,
  `app/src-tauri/Cargo.toml` (if the plugin is taken), `app/src/` (the menu and the missing page), `host/tests/`,
  `app/test/`, `app/browser/`.
- Dependencies: `tauri-plugin-dialog` 2.x (owner, 2026-11-24).
- The test rule: no test touches the real home, real launchd, a real daemon, the keychain or the network. Each test
  that runs the `collide` binary clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to
  a temp root, as `host/tests/common/mod.rs` `TempRoot` does. A temp repository sets `GIT_CONFIG_GLOBAL` and
  `GIT_CONFIG_SYSTEM` to empty files. Keychain writes go through `MemorySecretStore` (`host/src/secret_store.rs`),
  never the login keychain. Browser tests run the installed Chrome with a temp profile
  (`app/browser/support/chrome.ts`), and the bridge's daemon on a temp root (`app/browser/support/world.ts`). No test
  opens the system folder picker: the picker is a seam.
- Stage 6a item (revision 25 scope, split 2026-11-24). Items of Stage 6a run one at a time, in id order, because they
  share `app/`, `core`, `host` and the generated files. No builder runs `tauri build`; the real picker is checked by
  the owner in `ts-0197-uj`.
- Nothing here runs a real `claude`.
- Owner decisions (2026-11-24): the owner took the recommendations at the split.

### Built

- Wire. `core/src/owner/board.rs`: `BoardSummary.missing` (omitted when false), `BoardLocateParams { board, path }`,
  `BoardLocateResult { board, note }`. `core/src/owner.rs` and `core/src/schema.rs`: the method `board.locate` (kind
  Board) and its doc row; `core/tests/owner_protocol.rs` has its fixture row. `pnpm gen` was run: new
  `BoardLocate{Params,Result}` files, and `BoardListResult` and `BoardOpenResult` changed, in `schemas/` and
  `packages/pack-sdk/src/generated/`.
- Ids. `host/src/store.rs` migration 13 (`BOARD_IDENTITY_SQL`): `boards.name`, `boards.root_commits`, unique index
  `boards_by_name`. A NULL name is the legacy `board-<rowid>`, so no id and no file moves. `host/src/scratch_store.rs`:
  `random_suffix` (six lowercase letters from `RandomState`, never a number), `Store::open_board(root, taken)` and
  `open_board_with` (up to 64 tries; a name in the store or `taken` is skipped), `boards`, `board_root`,
  `root_commits`, `record_root_commits` (once; never replaced), `move_board`; `row_of` maps a name to its row for the
  scratch-task calls. `host/src/daemon.rs` `Shared::board_name_taken` also checks the views folder
  (`Lines::board_folder_exists` in `host/src/board_lines.rs`) and `pack-data/<id>`.
- Main checkout. `host/src/checkout.rs` `main_checkout_with` (the env of the daemon's git); `host/src/boards.rs`
  `board_checkout`; `host/src/daemon.rs` `board_open` and `boards_checkouts`: a folder in a linked worktree opens the
  board of the main checkout, and the worktree is an extra line. Trust is keyed by the main root
  (`Lines::summary` takes the lines; `line_of` uses the board root for `BoardRef`).
- Locate. `host/src/boards.rs` `root_commits` (`git rev-list --max-parents=0 HEAD`) and `same_repository`;
  `host/src/daemon.rs` `board_locate`, `board_list` (`missing` is `!is_dir`, and a present board with no record gets
  one), `Shared::note_root_commits`; `core/src/trust.rs` `TrustFile::move_board`, `host/src/trust.rs`
  `Trust::move_board`, `Lines::move_trust`; `host/src/sources.rs` `Sources::stop_board`;
  `host/src/corpus_service.rs` `Corpora::restart` and `BoardCorpus::close`.
- Shell. `app/src-tauri/src/shell.rs`: `PICK_FOLDER`, trait `FolderPicker`, `NoPicker`, `RecordingPicker`,
  `pick_folder`, `Shell::with_picker`, the `rpc` arm (never forwarded). `app/src-tauri/src/lib.rs`: `DialogPicker`
  (`tauri-plugin-dialog`, called from the shell's thread) and `.plugin(tauri_plugin_dialog::init())`. The window's
  capability file is unchanged, so the plugin has no command the page can call.
- App. `app/src/owner/methods.ts` (`board.locate`, `shell.pick_folder`); `app/src/loop/loop.ts` `refreshBoards` and
  `locateBoard` (`LocateOutcome`); `app/src/frame/boards.ts` `isMissing`; `app/src/frame/MissingBoard.tsx` (the page)
  and `LocateWindow.tsx` (a native `<dialog>`); `BoardMenu.tsx` ("folder not found" in the row, a re-read when the
  menu opens, "Choose a folder…" only when `pickFolder` is given); `Frame.tsx`; `App.tsx` (prop `shell`, default
  `'__TAURI_INTERNALS__' in window`; a missing board shows the page, no view picker, and `useField` and `Peek` get no
  board). Test support: `test/support/loop-world.ts` (`missingBoard`, `board.locate`), the `missing-board` world, and
  `browser/fake/main.tsx` (`#shell=1`, `shell.pick_folder`, `collideFake.pickNext`).
- Tests, all seen red at runtime against the old behaviour. For each group a stub was put in, the tests were run and
  failed, and the stub was removed (the files were compared with a copy afterwards).
  - `host/tests/board_identity.rs` (20). Red with `board_checkout` returning its input, `missing` fixed false,
    `same_repository` always true, `move_board` of the trust a no-op, and the taken check ignored:
    `a_linked_worktree_opens_the_board_of_the_main_checkout_with_itself_as_a_line`,
    `opening_the_main_checkout_after_its_worktree_finds_the_same_board`,
    `opening_the_worktree_after_the_main_checkout_finds_the_same_board`,
    `a_board_whose_folder_moved_is_listed_missing_and_nothing_is_deleted`,
    `locate_keeps_the_name_the_tasks_the_trust_the_views_and_the_pack_data`,
    `locate_refuses_a_different_repository_with_the_reason_and_the_board_stays_missing`,
    `locate_refuses_a_folder_that_is_another_boards_root`,
    `a_board_with_no_record_of_its_repository_is_located_with_a_note_that_the_check_could_not_run`,
    `a_repository_with_no_commit_records_nothing_and_is_not_taken_as_the_same_repository`. Red with a constant name,
    the check for another root skipped, a record needing equal sets, the folder not rooted at its top level, and the
    store move a no-op: `a_new_board_is_named_with_six_random_letters_and_the_same_root_gives_the_same_name`,
    `the_record_of_a_repository_with_two_root_commits_is_both_and_sharing_one_is_the_same_repository`,
    `locate_to_a_folder_inside_the_moved_checkout_roots_the_board_at_its_top_level`,
    `locate_survives_a_daemon_restart`,
    `a_board_folder_of_an_earlier_store_is_a_name_the_lines_say_is_taken`. Red with `open_board_with` ignoring
    `taken`: `a_name_taken_in_the_store_or_on_disk_is_never_given_to_a_new_board`,
    `a_store_that_cannot_give_a_free_name_says_so_and_adds_no_board`. Not red, and not expected to be, because they
    describe what must stay true: `a_board_with_its_folder_is_not_missing`,
    `a_board_that_has_a_worktree_as_its_root_is_left_as_it_is_and_the_main_checkout_is_another_board` (red only
    under the constant name),
    `locate_refuses_a_folder_that_is_not_a_checkout_and_a_board_the_daemon_does_not_have`,
    and `a_board_whose_folder_is_there_gets_its_record_when_it_is_listed` is red with the record at list time
    removed.
  - `host/src/store.rs` unit tests (red with `row_of` not matching a legacy row, a record that is replaced, a
    constant name and a no-op move): `a_board_from_before_names_were_stored_keeps_its_numbered_name_and_its_tasks`,
    `a_new_board_beside_numbered_ones_is_never_a_number_and_numbered_boards_keep_working`,
    `a_boards_root_commits_are_recorded_once_and_never_replaced`,
    `a_moved_board_keeps_its_name_and_its_tasks_and_the_old_root_opens_a_new_board`.
  - `host/tests/daemon_corpus.rs` `a_run_agent_may_not_locate_a_board_and_the_board_stays_where_it_is` (red with
    `refuse_run_agent` removed from `board_locate`).
  - `app/src-tauri/tests/rpc.rs` (red with the `PICK_FOLDER` arm removed, so the call was forwarded):
    `shell_pick_folder_answers_the_folder_the_person_chose_and_does_not_forward_it`,
    `shell_pick_folder_answers_no_path_when_the_person_cancels`,
    `shell_pick_folder_refuses_params_it_does_not_have_and_does_not_ask_the_person`,
    `shell_pick_folder_says_why_when_the_picker_fails`, `a_shell_with_no_picker_says_so_and_does_not_forward_the_call`.
    The old unknown-method test now calls `shell.nothing`.
  - `app/test/` (red with `isMissing` false, the two components rendering nothing, the picker button removed, and
    `locateBoard` and `refreshBoards` stubbed): `missing-board.test.tsx` (7), `frame.test.tsx` (the missing row, no
    picker without `pickFolder`, a picker with it; the "no picker" one is a negative and stayed green),
    `loop-page.test.ts` "a board whose folder moved" (6; the listing test and the empty-path test stayed green),
    and `owner-methods.test.ts` (the shell methods are `shell.rs` constants).
  - `app/browser/fake-missing.spec.ts` (10; all red with the same stubs). `app/browser/boards.spec.ts` (bridge, real
    daemon: a temp repo is moved, the page shows the missing page and the row, another repo is refused, the moved
    folder is located by typed path, and the id and the task are back; red with the same stubs).
- Not seen red, by their nature: the generated files, and the `owner_protocol` fixture row (it fails to compile
  without the method).
- Checks run after the last edit: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `pnpm typecheck`, `pnpm --filter @collide/app build`, `pnpm build:packs --out <tmp>/packs`, `pnpm check`
  (all passed). `pnpm test` (all packages: 865 app unit tests, 140 Playwright tests including the new
  specs, 519 plan-fixture tests, 99 pack-sdk tests, packs/plan 32 files) passed.
  `cargo test --workspace --no-fail-fast -- --test-threads=2` ran once at the end: every target passed except three.
  `host/tests/base_env.rs` `a_hung_shell_falls_back_to_the_minimal_environment_and_is_killed` ("no pid in
  /tmp/cdt-...") failed in the full run and again alone; the file and what it tests are untouched, so it is reported
  and not changed. `host/tests/write_guard.rs`
  `a_queued_write_whose_version_is_old_becomes_a_conflict_item_and_overwrites_nothing` failed in the full run and once
  alone while the machine was busy, then passed five times alone (it asserts that the plan watcher has not yet noticed
  the test's own edit: a race, not this item). `config_set_command` is below. After the three, `cargo clippy ... -D
  warnings` and `cargo fmt --all --check` were run again and passed, and the two worktree tests below and
  `config_set_command` (17) passed alone.
- Existing tests this change broke, edited (the first run failed them, so each was seen red before its edit):
  `host/tests/adapters/claude/accept.rs` `a_board_whose_root_is_a_linked_worktree_merges_into_the_branch_it_is_on` and
  `quick_run.rs` `a_board_whose_root_is_a_linked_worktree_runs_from_the_branch_it_is_on` opened a linked worktree with
  `board.open` and expected a board rooted there; they now store such a board (`quick_run.rs` `stored_board`, as an
  earlier build did) and keep their assertions. `host/tests/config_set_command.rs`
  `a_plain_value_in_a_checkout_goes_to_the_boards_own_file_and_is_printed_with_its_layer` read
  `boards/board-1/config.yaml`; it now reads the one folder under `boards/`.

### Decisions for the owner

- **Same repository means the two sets of root commits share a commit.** A repository with two roots (a merged
  history) still matches the folder it moved to. Alternative: equal sets, which refuses a clone that later gained a
  second root. Recommend as built.
- **An empty repository records nothing, and locating onto one is refused** ("no commit yet"), because nothing says it
  is the same repository. Alternative: accept it with a note. Recommend as built.
- **A board with no record is located anyway, with a note** ("could not check"). A board gets its record the first time
  it is opened or listed while its folder is there, so only a board whose folder was already gone before this build
  has none. Alternative: refuse until the person confirms. Recommend as built; the note is the confirmation.
- **A board already rooted at a linked worktree is left alone.** Only new opens use the main checkout, so no id,
  trust entry or view moves. Opening the main checkout then makes a second board. Alternative: a one-time merge of the
  two boards. Recommend as built; merging is a later item if it is ever seen.
- **A name is `board-` and six random letters from `RandomState`**, checked against the store, the views folder
  (`<config>/boards/<id>`) and `pack-data/<id>`. Alternative: a `getrandom` crate; the brief allows no new crate, and a
  name only has to be unique, not secret. Recommend as built.
- **`missing` is read live with `is_dir` each time the boards are listed**, and the app lists again when the board
  menu opens. Alternative: a flag the corpus sets when it notices the folder is gone (it looks every `GONE_PERIODS`),
  which can be stale. Recommend as built.
- **Locate moves the trust entry with the board, replacing one the new root had.** Alternative: ask for trust again.
  Recommend as built: the person trusted this repository, and what is trusted is a hash of the line's files, so a
  changed file still asks.
- **Locate stops the board's providers and restarts its corpus**, so the next read starts at the new root.
  Alternative: do nothing and let the next restart notice. Recommend as built.
- **In the shell the board menu and the locate window show a "Choose a folder…" button beside the typed path.** The
  typed path stays as the way round a picker that fails. Alternative: picker only in the shell. Recommend as built.
- **The locate window lives in `Window`, not in the missing page**, so it stays open after the board stops being
  missing and can show the daemon's note. Alternative: close at once and drop the note. Recommend as built.
- **Tests that opened a worktree as a board now store the board themselves.** `board.open` on a worktree no longer
  gives a board rooted there, so the two tests above build the legacy case through the store. Alternative: delete them
  as unreachable. Recommend as built: such boards still exist on disk and are left alone.
- **`board.locate` is refused to a run's agent** (NOT_PERMITTED), like `board.trust`: it changes where a board points.
- **"Remove from Collide" is a disabled button** with the title "Arrives with board removal", as the mock has it.
- **The app decides it is in the shell by `'__TAURI_INTERNALS__' in window`**, with `App`'s `shell` prop as the
  test's override. Alternative: ask the shell with a method at start-up. Recommend as built; it needs no round trip.
- **Plan text proposed for Stage 6a, ts-0191-nv:** "A board's name is `board-` and six random letters; boards named
  `board-<n>` before this keep their names. A board is rooted at the main checkout; a linked worktree is a line of
  it. `board.locate {board, path}` moves a board to a folder of the same repository (a shared root commit), keeps
  its name, tasks, trust, views and pack data, and refuses another repository with the reason. `BoardSummary.missing`
  is read live."

### Not done, by design

- Removing a board ("Remove from Collide" is shown disabled): a later board-removal item.
- The old path chip and Move: Stage 12.
- The Up next list of a missing board: Stage 11.
- Merging an old board rooted at a worktree into its main checkout's board: not planned.
- Opening the real system picker: no test does; the owner checks it in `ts-0197-uj`. `tauri build` was not run.
