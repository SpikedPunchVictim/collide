---
id: ts-0119-tj
kind: task
status: ready
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
