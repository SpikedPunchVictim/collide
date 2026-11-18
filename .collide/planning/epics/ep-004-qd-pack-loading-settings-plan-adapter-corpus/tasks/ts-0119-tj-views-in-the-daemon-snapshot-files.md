---
id: ts-0119-tj
kind: task
status: done
title: "Views in the daemon: snapshot files, personal views and task.list"
owners: []
links:
  - { type: blocked-by, to: ts-0118-wp }
---

## Goal

Load the views of a board, evaluate them in the daemon, and serve them: `task.views`, `task.save_view` and `task.list`
with a `view`. A view comes from the trusted snapshot, never from the working tree, and one view that fails to load
never hides the others.

What exists. `host/src/board_load.rs` lists the files of `.collide/views/` into `LoadedLine::views` (`ViewFile`: name
and blob id) and puts each blob id in the trust hash, and does not read them. Packs may declare `views:` in their
manifest (`core/src/pack.rs`). `core/src/view.rs` has `View` and `check_view`. After `ts-0118-wp`,
`core/src/view_eval.rs` evaluates one. `core/src/owner/task.rs` has the wire types `ViewEntry`, `ViewSource`,
`TaskViewsParams`, `TaskViewsResult`, `TaskSaveViewParams`, `TaskSaveViewResult` and `ViewRef`; the daemon serves none
of the three methods. `host/src/user_packs.rs` has the per-board folder `~/.config/collide/boards/<board-id>/`
(`board_dir`), and `host/src/settings_files.rs` reads a file there.

What this adds.
- Reading the views of a line: each `.collide/views/*.yaml` from the snapshot's blob (git object, not the file system),
  parsed, checked with `compile_view`, tagged `Repo`. A pack's `views:` are tagged `Pack`, with the pack's name. A
  personal view under the board's folder `views/` is tagged `Personal` and read from the file system (it is the user's
  own file, outside the repo and outside trust).
- Name rules: a repo view and a personal view of one name is a duplicate; the personal one is the one used, and the
  repo's gets an error naming it. A pack's view is named `pack/name`.
- `task.views`: every view with its member count, its error count and its load error.
- `task.list` with `view: Named` or `view: Inline` (an ad-hoc view from the filter bar), `ready`, `kind` and `checkout`,
  against the corpus of `ts-0116-wh`.
- `task.save_view`: checks the view as a load does, then writes the YAML into the repo's `.collide/views/` of the main
  checkout (not committed), or the personal folder. It refuses to overwrite a view of the same name unless the call says
  so.
- A view's load happens again when the line's trust hash changes, and a view that is not trusted is not read. Trust
  covers the blob ids that are already in the hash.

## Success criteria

- A view file on a branch that is not the line's snapshot is not used. A view file edited in the working tree and not
  committed is not used. The same view committed on the line is used after the line is reloaded.
- A changed view blob changes the trust hash, and until it is trusted the view is not read (the stage's "an edited view
  on a branch is not used, and a changed hash asks again").
- A view that fails to parse, is over 1 KB, names a corpus variable, or is too costly shows in `task.views` with its
  error and no members, and the other views list as usual. A fixture whose compile panics (through the injected compile
  function) is one of them.
- Two views, one repo and one personal, with the same name: the personal one is used and the repo one has the error.
- `task.list` with `view: Named` returns the view's members in the corpus order, with the view's error and budget
  counts, and the invalid items of the board.
- `task.list` with `view: Inline` evaluates the view without saving it, and a bad inline view is a refused request with
  the reason, not a crash.
- `task.save_view` to the repo writes a file the next load reads, to personal writes to the board's folder, and each
  refuses an invalid view, a name that is not a safe file name, and an overwrite with no flag. Nothing is committed.
- A personal view in a board folder whose id is not a safe name is an error, not a path outside the folder.
- The count on `task.views` equals the length of the list for the same view.
- `ready`, `kind` and `checkout` filters apply on top of the view.
- The view files of an untrusted line are listed as not trusted and not read.

## Tests

Write these first. Daemon tests as in `host/tests/daemon_socket.rs`, in a temp repo with a fake provider.

- A view committed on the line, edited in the working tree, committed on another branch: which is used.
- The trust hash changes with a view blob, and an untrusted line reads no view.
- Four broken views (not YAML, over 1 KB, corpus variable, panic) next to a good one; the good one lists.
- The same name in repo and personal; a pack's view; a bad personal view.
- `task.list` by name and inline, with the filters, the counts, and the invalid items.
- `task.save_view` to each target; the refusals; the written file reloads to the same view.
- A board id with a path in it.
- Personal view files are fixtures in the temp `HOME`, never the owner's own.
- A closure view over the fixture corpus of `ts-0116-wh` through the socket, and the same numbers as the pure test of
  `ts-0118-wp`.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4b: Views ("Views from the snapshot's `.collide/views/*.yaml` and from personal views"); Success Criteria: "An
    edited provider script or view on a branch is not used, and a changed hash asks again."
  - 0.7: Keys added for planning (a pack's `views:`). 0.9: Trust (views are hashed). 0.10: Views (the YAML, personal
    views at `~/.config/collide/boards/<board-id>/views/`). 0.11: Filter bar, Save as View.
- Carried from `ts-0096-ph` and `ts-0097-kw`: views were listed and hashed and not parsed, and the trust hash already
  holds each blob id.
- **Decision for the item, with a recommendation: a repo view and a personal view of one name.** The personal one wins,
  and the repo one shows the error, so a person's own copy cannot be hidden by a pull. Alternative: the repo's wins,
  which makes a personal view impossible to change by name.
- **Decision for the item, with a recommendation: where `save_view` to the repo writes.** The main checkout's
  `.collide/views/`, not a workspace's, and not committed (Collide never commits). The trust hash changes at the next
  commit, so the owner is asked again. Alternative: write into the snapshot's checkout of the current branch. There is
  no such thing for a line whose root is a workspace.
- **Decision for the item, with a recommendation: personal views are global to the board, not to a line.** As the plan
  says (0.9, 0.10). They are read at each `task.views` call and not hashed.
- **Decision for the item: `task.views` counts.** The count of members and of errors comes from the last evaluation and
  is recomputed when the corpus changes, not for each call.
- **Decision for the item: pack views.** Read from the pack's manifest as the pack loader gives them, tagged `Pack`.
  They are trusted with the pack.
- Out of scope: `task.sections` and the planning pages (4c); `layout` and the Field (Stage 6); launching from a view
  (Stage 11); saving a Tracks view (not 4b).
- Area: `host/src/board_load.rs` (read the blobs), a new `host/src/views.rs`, `host/src/daemon.rs`,
  `host/src/user_packs.rs` or `host/src/settings_files.rs` (only to find the folder), `core/src/owner/task.rs` (only if
  a field is added), `host/tests/`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Settings files are fixtures, never the owner's own `settings.json`. Every git repository is a temp one with
  `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_SYSTEM` set to empty files.
- Stage 4b item (revision 23 scope, split 2026-11-17). Items of Stage 4b run one at a time, in id order, because they
  share `core`, `host` and the generated files.
- Nothing here runs a real `claude`.
- Owner decisions (2026-11-17, at the split): the recommendations above, on delegation.

### Built

No new crate or package. Wire types changed (additive), so `pnpm gen` was run and the generated files are in step.

- `core/src/owner/task.rs`: `ViewReport` (new: `name`, `error`, `members`, the `ViewCounts` fields, `steps`, `cycle`;
  `ViewReport::failed(name, reason)`). `TaskListResult.view: Option<ViewReport>` (additive).
  `TaskSaveViewParams.overwrite` (default false).
- `core/src/view_eval.rs`: `compile_view_with(view, tasks, budgets, before_parse)` (`compile_view` calls it with a
  no-op) and `view_report(name, &ViewResult)`. `core/src/view_expr.rs`: `compile_with_hook`, the stdlib parse with a
  hook called on the parse's own thread, so a panic in it is `ExprError::Panicked` as `compile_with` gives it.
- `core/src/corpus.rs`: `ListFilter.only` (a view's member keys; `ready`, `kind` and `checkout` apply on top) and
  `Corpus::stamp()`, new at each change through a process-wide counter and the same on a copy.
- `host/src/views.rs` (new): `personal_dir` (through `UserScope::board_dir`, so an unsafe board id is an error and
  never a path), `personal_candidates`, `repo_candidates` (blobs of the line's commit, only while the line is
  trusted, sizes checked before a read), `pack_candidates` (`pack/name`; repo and `git:` packs wait for the line),
  `ViewCache` (`compile`, `assemble`: first of a name wins, later ones get an error; `evaluate`: kept per board, per
  corpus stamp, at most 64 for one stamp), `repo_dir`, `save_view` (safe-token name, overwrite refused unless asked,
  atomic write, refused through a link: `SaveError::Blocked`, added at review).
- `host/src/board_load.rs`: `ViewFile.size`. `host/src/board_lines.rs`: `SourceLine.git_dir`, `Lines::home()`.
- `host/src/daemon.rs`: `task_views`, `task_save_view` (refused for a run's agent; a repo save refused while the
  root is busy, added at review) and `task_list` through a view in
  place of the old refusal. `DaemonOptions.view_before_parse` (test seam for the panic fixture).
- Tests: `core/tests/corpus.rs` (`the_only_filter_*` x2, `the_stamp_changes_*`), `core/tests/view_eval.rs` (4: panicking
  parse, the hook, the report, the failed report), `core/tests/owner_protocol.rs` (samples carry `view` and
  `overwrite`),
  `host/tests/views_files.rs` (3: unsafe board ids, non-regular personal files, the cache; added at review:
  `.collide/views` as a link, `.collide` as a link, `<name>.yaml` as a link with
  overwrite, a missing folder still made), and in
  `host/tests/daemon_corpus.rs` 16 tests over the socket, named for each criterion: committed vs working tree vs
  other branch, trust hash and untrusted line, four broken views, same name, pack view, bad personal view, list by
  name, filters on a view, inline, counts follow the corpus, closure with the pure test's numbers, a line that does
  not load, and `save_view` to each target and its refusals. The provider fixture takes `labels`, `collections`
  and `links`. Added at review: a repo save to a root in a rebase (the refusal, its words, no file, a personal save
  still works, free again after `rebase --abort`) and a repo save with `.collide/views` as a
  link (`CONFLICT`), and `a_run_agent_may_not_save_a_view_and_nothing_is_written` (the test binary re-runs itself as a
  child with the run marker, as `pause.rs` does: both targets answer `NOT_PERMITTED`, no file, and the person's own
  client then saves with the same calls). I removed
  `a_view_is_not_served_yet_and_says_which_item_serves_it`: it asserted the old refusal.
- **Seen red at runtime.** Old behaviour (a view refused, `task.views` and `task.save_view` not served): all 16 daemon
  tests failed. One mutation each, restored after: `only` ignored (the 2 `only` tests); stamp not changing (the stamp
  test); hook not called (the 2 hook tests and `four_broken_views_*`); `view_report` dropping steps and invalid items
  (the report test and the closure test); `is_trusted` always true (the trust test); repo views read from the working
  tree (`a_committed_view_*`); personal after repo (the same-name test); overwrite and name checks off (the save
  refusals test); personal path without the safe-name check (the board id test); cache never hits (the cache test).
  Not red by a mutation: `a_view_on_another_branch_*` (only the old behaviour turns it red, through its positive part:
  a line lists only its own tree) and `a_failed_report_*` (new type).
- Seen red at runtime for the review changes, before the code: the rebase test (the save succeeded) and the three
  link tests (each save wrote through the link). The run-agent test, with `refuse_run_agent` removed from
  `task_save_view`: both calls answered `ok`. The `CONFLICT` link test was written with the code.

### Decisions for the owner

1. **A file that cannot be read is listed with an empty view.** `ViewEntry.view` is a `View` with only the file name
   (without `.yaml`) as its name, and the reason in `error`. Alternative: make `view` optional on the wire. Recommend
   keeping it: the Field needs no special case. Applies to a parse error, a file over 256 KiB, and an untrusted line.
2. **A view is looked up by its `name:`, not by its file name.** Saving writes `<name>.yaml`, so they agree for what the
   app writes. Alternative: the file name wins. Recommend keeping.
3. **`task.save_view` needs a name that is a safe token** (1 to 64 of letters, digits, `.`, `_`, `-`, not starting
   with `-` or `.`): a name with a space is refused with that reason. This is what the person sees in Save as View.
   Alternative: turn a free-form name into a file name (`My view` to `my-view.yaml`) and keep the name inside the
   file. Recommend the alternative when the Filter bar is built (Stage 4c): the dialog is where the person types.
4. **`error_count` on a view** is the board's invalid items plus the tasks whose `where` raised an error or ran out of
   steps. The item says "items that failed to read on this view". Alternative: invalid items only.
5. **A line that does not load refuses `task.views` and a named list** (`CONFLICT`, with the problems), personal views
   included. An inline view needs no line. Alternative: list the personal views anyway. Recommend keeping: the
   board is not usable then, and the problems are the message.
6. **A board id that is not a safe name makes `task.views` and a named list fail** (`CONFLICT`). The daemon makes
   ids, so this cannot happen from the app. Alternative: one entry with the error.
7. **Pack views** of `Builtin`, `User` and `Linked` packs are trusted (the user's own or shipped); those of `Repo` and
   `Git` packs wait for the line. Recommend keeping.
8. **One name, one view across all three places**, in the order personal, repo, pack. Two repo files with the same
   `name:` give the first by file name and an error on the second.
9. **`task_count` is per key, a list row is per path.** They are equal unless a key is held at two paths (a collision),
   where the list has two rows. Alternative: count rows. Recommend keeping; the Field shows tasks.
10. **Changed at review: a repo save to a busy root is refused with the reason, not queued and not written straight.**
    `task.save_view` with `target: repo` checks `checkout::root_busy` on the board root first (with the daemon's git
    environment). While a rebase, merge, cherry-pick, revert or bisect is in progress there it answers `CONFLICT`,
    naming what is in progress and saying the person can save a personal view or try again when it is done. Nothing
    is written. A personal save is not affected. Writing `.collide/views/<name>.yaml` mid-rebase could block the
    rebase (an untracked file in the way of a commit that adds it) or mix into its conflict resolution. The
    alternative is a queue kind for view saves. Proposed plan text for 0.5, after the Root busy bullet's first
    sentence: "A view's save (`task.save_view` to the repo) is refused instead, with the reason, since Save as View
    is the person's own act and they can retry (`ts-0119-tj`)."
11. **`TaskSaveViewParams.overwrite`** is the flag's name, and `TaskSaveViewResult.path` is absolute. A personal file is
    written with mode `0600`, a repo file with `0644`.
12. Not added: `unresolved_dependencies` (`ts-0118-wp` offered it). Add it with the Field (Stage 4c) if it shows one.
13. A closure whose goal matches nothing is `goal_unmatched` in the report, not `error`: the view loads and lists none.
14. **Added at review: a save never writes through a link.** Before a save writes, each folder below the board root
    (the Collide folder, then `views`) that exists must be a real folder by `symlink_metadata`, and an existing
    `<name>.yaml` a regular file; otherwise `CONFLICT` names the folder or file and nothing is written, with or
    without `overwrite`. A missing folder is still made. A repository can commit `.collide` or `.collide/views` as a
    link, and `save_view` needs no trust, so the text check of `repo_dir` was not enough. The personal target shares
    the check, below its board folder.

- Owner decisions (2026-11-18, at the build): all taken as recommended, on the owner's delegation. Two were changed or
  added at review: decision 10, a repo save to a busy root is refused with the reason (plan 0.5 says so), and decision
  14, no save writes through a link. Decision 3 (a name with a space is refused) is what Save as View shows until the
  dialog of Stage 4c turns a typed name into a file name.

### Not done, by design

- The CLI: `collide tasks --view <name>` and `--checkout` (`ts-0120-wm`). The plan has no CLI to list or save views;
  the app's Save as View is the only writer.
- A parsed repo view is not kept between calls: each `task.views` or named list reads the line again (git reads and the
  loader). Only the evaluation is kept. Keep the parse by (commit, trust hash) if a call proves slow at 1,600 tasks.
- `task.sections`, planning pages, the Field's `layout`, launching from a view and saving a Tracks view: later stages.
