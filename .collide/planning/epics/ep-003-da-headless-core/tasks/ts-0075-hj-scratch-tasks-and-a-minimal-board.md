---
id: ts-0075-hj
kind: task
status: done
title: "Scratch tasks and a minimal board in the daemon"
owners: []
links:
  - { type: blocked-by, to: ts-0074-cu }
---

## Goal

The daemon keeps boards and scratch tasks in its store (`host/src/daemon.rs`, `host/src/store.rs`). This is what
Stage 3c pulls from Stage 4b: `create` and `setStatus`, and nothing else. No corpus, no views.

- `board.open` in its smallest form. A folder in a git checkout becomes a board:
  - a stable id for the checkout's top level, so opening it twice gives the same board;
  - its root;
  - one line, the branch the checkout has now.
- `board.list`: the boards opened before, read from the store.
- `task.apply` on source `scratch`:
  - `create` takes the prompt, and its first line becomes the title;
  - `setStatus` takes one of the five categories (`ready`, `active`, `review`, `done`, `dropped`).
- `task.get` of a scratch task, with the whole prompt.
- Each write records a `task.changed` event, with the connection's actor.

## Success criteria

- A scratch task created over the socket has the key `scratch:<id>`, its first line as the title, and status
  `ready` (0.10).
- `setStatus` moves it to each category. An unknown category is refused, and the message names the five.
- Scratch tasks belong to one board. A key from one board is not found on another (0.10: one list per user per
  board).
- Boards and scratch tasks survive a daemon restart.
- `board.open` on a folder that is not a git checkout returns no board and a failed `git_repo` check, in Collide's
  words, with the fix. `init_git: true` is refused as not built yet (Stage 6a).
- A scratch task is never written into the repository (0.1: scratch tasks are never committed).

## Tests

Write these first:

- scratch-task create and status (the Stage 3c test), over the socket against a temp-root daemon;
- the first line as the title, for a one-line prompt and a long one;
- an unknown category;
- two boards, and a key that belongs to the other one;
- a daemon restart keeps boards and tasks;
- `board.open` on a plain folder, on a checkout's top level and on a folder inside it;
- `git status` of the checkout is unchanged after create and every `setStatus`.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 3c: what it pulls from Stage 4b (scratch tasks with `create` and `setStatus`).
  - 0.1: Scratch tasks are the one exception.
  - 0.10: Task sources (the scratch source: statuses are the categories, a prompt's first line is the title).
  - Stage 4b: Scratch source (what is finished there).
- **Decision for the item, with a recommendation: where the scratch source lives.** Recommend a small type in
  `host` that implements `core/src/ports.rs` `TaskProvider` over the store: `get` and `apply` real, `list` for one
  board, and `watch` and `preview` answering `Unsupported`. Stage 4b then finishes the same type behind the
  supervisor. The alternative, plain daemon methods with no provider, would be rewritten in 4b.
- **Decision for the item: the short id.** Recommend a number per board (`scratch:1`, `scratch:2`). It is short to
  type and needs no collision rule.
- **Decision for the item: `board.open` checks.** Recommend only `git_repo` in 3c. The `claude` checks come with
  Stage 6a's no-setup open. The window shows the `claude` versions from `daemon.agent_versions` (`ts-0081-wm`).
- `run.start` and `task.*` already take a `board` (`core/src/owner/run.rs` `RunStartParams`, `core/src/owner/task.rs`).
  3c's pull list does not name a board, but scratch tasks are per board, so a minimal one is needed here.
- The store's migrations are append-only. This item adds one for boards and scratch tasks.
- Run `pnpm gen` if any wire type changes.
- Area: `host` (`daemon.rs`, `store.rs`, a new scratch source module), plus `core` only if a wire type changes.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 3c item (revision 22 scope, split 2026-11-15). Items of Stage 3c run one at a time, in id order, because
  they share `host`, the app and the generated files.

### Built

The daemon serves `board.open`, `board.list`, `task.apply` (`create`, `setStatus`) and `task.get` for scratch tasks.
Nothing in `core` changed, so `pnpm gen` had nothing to regenerate.

- `host/src/store.rs`: migration 5 (`boards_and_scratch_tasks`, the `BOARDS_SQL` const): `boards(id, root UNIQUE)` and
  `scratch_tasks(board, num, title, prompt, status, version)`. Test
  `a_store_at_version_four_gains_the_board_and_scratch_task_tables`.
- `host/src/scratch_store.rs` (new): the SQL on `Store`. `open_board`, `boards`, `board_root`, `add_scratch_task`,
  `scratch_task`, `scratch_tasks`, `set_scratch_status`. The number of a new task and its insert are one immediate
  transaction.
- `host/src/scratch.rs` (new): `ScratchSource`, a `TaskProvider` over the store for one board. `initialize`, `list`,
  `get` and `apply` are real. `watch` and `preview` answer `Unsupported`. Also `statuses()`, `split_id`, `title_of` and
  `changed_event`.
- `host/src/boards.rs` (new): `checkout_of` (git `rev-parse --show-toplevel` and the branch), the `git_repo` check, and
  `summary`, which builds a `BoardSummary` with its one line.
- `host/src/daemon.rs`: handlers `board_open`, `board_list`, `task_get`, `task_apply`, routed in `call`, and the
  `Shared.git_env` field (the git environment the daemon is given). The module doc lists the new methods.
- `host/src/lib.rs`: the three modules.
- `host/tests/metrics_store.rs`: `migration_four_makes_the_metrics_tables` read `MIGRATIONS.last()`. It reads
  `MIGRATIONS[3]` now, because a fifth migration exists. Nothing else in that test changed.

**Statuses are data (coordinator's direction, from the owner).** The scratch statuses are a list of
`core::item::StatusDecl`, the shape a provider returns from `initialize`: `scratch::statuses()`, in order ready, active,
review, done, dropped, each with the category of the same name. `ScratchSource` holds that list and uses it for
everything: `setStatus` checks the name against it, the refusal message is built from it, the first entry is the
status a new task gets, `initialize` returns it (and derives `categoryStatus` and the `setStatus` arg enum from it),
and the category of a task is looked up in it. The store keeps only the status name. No other code in `host` or `core`
matches on a scratch status name. The provider test checks that `initialize` reports the five names in order.

**`setStatus` by status or by category (review fix).** It takes exactly one of `status` or `category` (0.10); both, or
neither, is refused. A category maps through `scratch::category_status`, built from the declared list, which is also
what `initialize` reports as `categoryStatus`: there is no category-to-name match in the code. A category the list has
no status for (`backlog`) is refused, and the message names the categories the list covers. The `initialize` schema of
`setStatus` shows both args, with `oneOf` for the two. New tests, written first and seen red:
`set_status_by_category_moves_the_task_to_each_of_the_five`,
`a_category_with_no_scratch_status_is_refused_and_the_message_names_the_ones_covered` and
`set_status_takes_exactly_one_of_status_and_category`; the provider test also checks the schema.

**Tests**, all in `host/tests/scratch_tasks.rs` (26), over the socket against a temp-root daemon, with the test's own
environment cleared for every git command and the daemon's git given a home and config under the root:
- `a_scratch_task_is_created_ready_with_its_key_and_moves_to_each_category` (the Stage 3c test)
- `the_title_is_the_first_line_of_a_one_line_prompt_and_of_a_long_one`
- `task_get_gives_the_whole_prompt`
- `a_prompt_with_no_text_is_refused`
- `an_unknown_category_is_refused_and_the_message_names_the_five`
- `a_task_that_is_not_on_the_board_is_refused_and_a_missing_status_is_refused`
- `a_key_from_one_board_is_not_found_on_another`
- `a_task_of_another_source_is_not_found_and_a_write_to_one_is_refused_as_not_built`
- `a_write_names_its_source_one_way_or_another`
- `a_write_with_a_stale_version_is_a_conflict_that_carries_the_current_task`
- `a_scratch_task_is_never_committed_so_commit_is_refused`
- `a_request_for_a_board_the_daemon_has_not_opened_is_not_found`
- `each_write_records_a_task_changed_event_with_the_actor_of_the_connection`
- `boards_and_scratch_tasks_survive_a_daemon_restart`
- `a_folder_that_is_not_a_checkout_opens_no_board_and_fails_the_git_repo_check_with_the_fix`
- `a_folder_that_is_not_there_opens_no_board`
- `a_relative_root_is_refused`
- `init_git_is_refused_as_not_built_yet`
- `a_checkouts_top_level_and_a_folder_inside_it_open_the_same_board_on_the_branch`
- `board_list_gives_the_boards_opened_before_with_the_branch_each_checkout_is_on_now`
- `a_board_whose_folder_is_gone_is_still_listed_with_no_line`
- `the_checkout_is_unchanged_by_create_and_by_every_set_status` (`git status --porcelain --ignored`, `HEAD` and the
  top-level file list)
- `the_scratch_source_lists_one_board_and_declines_watch_and_preview`

The tests were written after the code compiled, not before: the red step was not seen first. A mutation check stood in
for it: a new task made `active` instead of `ready` failed five of them.

### Decisions for the owner

1. **Where the scratch source lives.** Chose the item's recommendation: `ScratchSource` in `host`, a `TaskProvider`
   over the store. Stage 4b finishes the same type behind the supervisor. Recommendation: keep.
2. **The short id.** Chose a number per board (`scratch:1`, `scratch:2`), as recommended. The long id is the number too.
3. **`board.open` checks.** Chose only `git_repo`, as recommended.
4. **The board's id.** Chose `board-<n>`, given by the store to each checkout root the first time it is opened.
   Alternative: a hash of the root path, which needs no store row but would not keep the tasks if the store were lost,
   since they live there too. Recommendation: keep.
5. **The board's root.** Chose the checkout's top level (`git rev-parse --show-toplevel`, symbolic links resolved), as
   the item says. A linked worktree is therefore its own board. Alternative: the main checkout
   (`host/src/checkout.rs` `main_checkout`), which every worktree of a repository shares, as 0.8 says the board is the
   repo root. Recommendation: keep for 3c, and move to the main checkout when lines arrive (Stage 14).
6. **The line.** The branch is read from git at open and at every list, not stored, so it is the branch "now". A
   checkout on no branch has the line `HEAD`. A board whose folder is gone or no longer a checkout is listed with no
   line. `snapshot` is empty. Trust is `Untrusted` with an empty hash: nothing in a checkout is read for trust in 3c,
   and `Untrusted` fails safe if later code checks it. Alternative: `Trusted` with an empty hash. Recommendation: keep.
7. **`setStatus` args and refusals.** It takes exactly one of `status` (a declared name) or `category` (mapped to the
   declared status for it), as 0.10 says. Both or neither is refused. An unknown name or category is
   `ApplyResult::Refused`, not an RPC error, and so are an unknown task and an unknown op. `backlog` is refused: no
   scratch status has that category. Recommendation: keep.
8. **`setStatus` to the status the task has.** Answers `ok` with no item, no version change and no event. Alternative:
   count it as a write. Recommendation: keep, so the event log holds only changes.
9. **The title.** The first line that has text, trimmed, with no length cap. A prompt with no text is refused.
   Alternative: cap the title (for example at 120 characters). Recommendation: add a cap when the board draws titles.
10. **The event.** `task.changed` has `source: scratch`, the connection's actor on the event, and `diff` with the
    `status` field. The cause is `journal`, the closest of the five. Alternative: a new `ChangeCause` variant such as
    `store` (a wire change, then `pnpm gen`). Recommendation: add it when Stage 4b finishes the source.
11. **Versions.** The version is the count of writes, as text, from 1. `apply` checks `version` when it is given: a
    stale one answers `conflict` with the current task. Recommendation: keep.
12. **Naming the source in `task.apply`.** `source`, or the id's prefix (`scratch:3`); a create has no id, so it needs
    `source`. A `source` that differs from the id's, or any source but `scratch`, is `INVALID_PARAMS`. `commit: true` is
    `INVALID_PARAMS` (0.1: never committed). `checkout` is ignored: a scratch task has no per-checkout state.
13. **Unknown board, relative root.** A board the daemon has not opened is `NOT_FOUND`. A relative `root` is
    `INVALID_PARAMS`, because the daemon's working directory means nothing to a client. `init_git: true` is
    `INVALID_PARAMS` with "not built yet (Stage 6a)", whatever the folder is.
14. **Plan text (propose, not applied).** In 0.10, after the scratch paragraph: "A scratch task's key is `scratch:<n>`,
    with `n` counted inside its board. Its statuses are a declared list, like any source's, and the store keeps the
    status name; the category comes from the list. A write to a scratch task records `task.changed` like any other
    source's, and `commit` is refused."

- Owner decisions (2026-11-15): decisions 1 to 13 taken under the owner's delegation. Decision 14's plan text
  applied, shortened, to 0.10's scratch paragraph; plan revision 23 (statuses are data) covers the rest. Review:
  `setStatus` also takes a `category`, mapped through the declared list, as 0.10 says workflow ops do (added in
  review, red first). The first tests were written after the code; a mutation check stood in for the red step.
  Carried to Stage 6a: the store gives board ids by count, so a lost store would give `board-1` to another
  repository, and Stage 6a, the first to key files by board id (personal views), must use an id a lost store never
  reuses; and the board root becomes the main checkout, so a linked worktree is a line, not its own board.

### Not done, by design

- `board.init`, `board.trust`, `task.list`, `task.preview`, views and the corpus: other items' and stages'.
- `collide task new` (Stage 4b) and the shell's and window's use of these methods (`ts-0077-wm` onward).
- `git init` for a folder that is not a checkout, and the `claude` checks (Stage 6a).
- `watch` and `preview` on the scratch source: they answer `Unsupported`.
- Runs are not linked to tasks yet: `task.get` gives no `runs` and no `latest_run`. `run.start` takes that up.
- Deleting a board or a task, and editing a prompt or a title.
- No notice is sent when a task changes: `ts-0076-cw` delivers events to subscribers.
- No change to the item's frontmatter or status, and nothing committed.
