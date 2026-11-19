---
id: ts-0141-sn
kind: task
status: done
title: "CLI: a listing says when a source of the board could not start"
owners: []
links:
  - { type: blocked-by, to: ts-0136-rr }
---

## Goal

Since `ts-0136-rr` every task command goes through the daemon. When a source of the board never starts (its pack fails
to load, its bundle is missing), `collide tasks` prints nothing on stdout or stderr and exits 0: the failure is only in
the daemon log (`failed_to_start` in `host/src/corpus_service.rs`). `task <id>` and `board.open` do name the problem.
An empty board and a broken one look the same from the CLI. Found at the `ts-0136-rr` review (its decision 10).

## Success criteria

- `task.list` names each source of the board that could not start, with the reason, in its result (beside
  `stale_sources`, which covers only sources that declared once).
- `collide tasks`, `tasks --view` and `tasks --ready` print one stderr line per such source,
  `collide: source <name> could not start: <reason>`, and still list what the others hold. The exit stays 0 when
  something listed, and is 1 when no source started.
- The app's list shows the same names (the field is on the wire; the app item that renders it may come later).

## Tests

- Write first and see red at runtime: a daemon test whose board names a pack that does not load gets the source and
  the reason in `task.list`.
- A CLI test over the same board: the stderr line, and exit 1 when no source started.
- A board with one good and one broken source lists the good one's tasks, exit 0, with the line.

## Notes

- `COLLIDE_BUILTIN_PACKS` is not forwarded when a CLI starts the daemon (`FORWARDED_ENV`: the daemon root, HOME and
  SHELL). Recommendation: leave it so. It is a test and dev override of the daemon's own, and a running daemon would
  ignore a CLI's value anyway; the stderr line above is what tells a person the bundle is wrong.
- Option B of decision 10 (exit 1 on any empty list) is not taken: it would fail an honestly empty board.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers at the split.

### Built

- Wire (`core/src/owner/task.rs`): `NotStartedSource { source, reason }`, and two additive fields on `TaskListResult`:
  `not_started: Vec<NotStartedSource>` and `sources_started: u32` (the sources that run, the scratch source apart).
  Both come with the first page only, like `stale_sources`. `pnpm gen` regenerated `schemas/TaskListResult.*.json` and
  `packages/pack-sdk/src/generated/TaskListResult.*.ts`. The app has no reader yet (the item says it may come later).
- Daemon (`host/src/corpus_service.rs`): the `failing` map holds a `Failing { times, next, reason }` instead of a tuple;
  `failed_to_start` stores the reason on every failure. `BoardCorpus::list` fills `not_started` from `failing`, leaving
  out a source that is already in `stale_sources`, and `sources_started` from `runs`. `look` drops from `failing` each
  source the line no longer names, so a removed pack is not reported for ever. `core/src/corpus.rs` only gains the two
  empty fields in its two list constructors and a doc line.
- CLI (`host/src/task_daemon.rs`, `tasks`): one stderr line per entry, `collide: source <name> could not start:
  <reason>`, before the stale lines. The exit is 1 when `not_started` is not empty and `sources_started` is 0, else as
  before (10 for a view error still wins). The same code serves `tasks --view` and `tasks --ready`. The help text
  (`host/src/task_cli.rs`) and its stored copy (`host/tests/fixtures/task-cli-help.txt`) say so.
- Tests. Daemon, `host/tests/daemon_corpus.rs`:
  `a_source_that_never_started_is_named_with_its_reason_until_it_starts` (an untrusted line, then trusted),
  `a_source_that_failed_after_it_declared_is_stale_and_not_in_the_list_of_those_that_did_not_start`,
  `a_source_the_line_no_longer_names_is_not_listed_as_unstarted`. That rig has no built bundle, so the plan source
  fails too (reason: not in the bundle); the tests expect it. CLI, `host/tests/task_daemon_cli.rs` (real plan pack, plus
  a repo pack whose provider exits at once):
  `a_listing_names_a_source_that_could_not_start_and_lists_the_others_and_exits_0`,
  `the_same_line_comes_with_a_view_and_with_ready`, `a_listing_exits_1_when_no_source_of_the_board_started` (the crash
  pack declares `plan`, which replaces the built-in one),
  `a_board_with_nothing_wrong_says_nothing_and_an_honestly_empty_one_exits_0`,
  `json_carries_the_unstarted_source_on_the_wire`.
- Seen red at runtime, with the code compiling and the old behaviour stubbed (`BoardCorpus::list` filling neither
  field): the three daemon tests and four of the five CLI tests; the honestly-empty one passes before and after
  (it guards the exit 0). Three more mutations, each caught by one test: no `stale_sources` filter, no `look` prune, and
  `sources_started` always 0.

### Decisions for the owner

1. Wire shape: a separate `not_started` list of `{ source, reason }`, beside `stale_sources`, and a `sources_started`
   count. Alternative: reuse `StaleSource` (one type fewer), but its doc and meaning are "out of date", which this is
   not. Alternative for the exit rule: the CLI guesses from an empty task list. That fails a board with one good,
   empty source and one broken, and fails `--ready` when nothing is ready. Recommend the count.
2. A source that failed after it declared (it is in `stale_sources`, with the tasks it last listed) is not named in
   `not_started`. The daemon knows this from the corpus: it is in `failing` and stale. One line per source, and the
   existing "may be out of date" line already says so. Alternative: name it in both. Recommend one.
3. How the daemon knows: `failing` already holds every source whose start failed, and `failed_to_start` is the one
   place that fills it, so the list is read from there and no new state is kept. A source leaves it when its next start
   works or when the line stops naming it.
4. "No source started" means no source of the line. The scratch source always runs, so a board whose only planned
   source failed exits 1 even when scratch tasks are listed. Alternative: exit 0 whenever something is listed, as the
   item's text can be read. Recommend 1: the board's planning is gone, and the stderr line says which.
5. `COLLIDE_BUILTIN_PACKS` stays unforwarded, and an honestly empty board exits 0 (the item's recommendations, taken).
6. Plan text, for `IMPLEMENTATION_PLAN.md` 0.2 (the CLI from a terminal), after the sentence on `collide tasks`:
   "A source of the board that could not start is named on stderr (`collide: source <name> could not start:
   <reason>`) and the others are listed; the command exits 1 when none started, and 0 otherwise."
   Not applied: the plan is the reviewer's.

### Not done, by design

- A line that does not load (`board_plan` fails, so the board names no source) still lists nothing and says nothing in
  the CLI; the daemon logs "its sources are not read". It is the same silent empty board, one step earlier. No item
  names it. Worth one: carry the line's problems in the same result, beside `not_started`.
- The commands that resolve an id through `settled_list` (`task <id>` and the writes) do not print the new lines.
  The item names the listings only.
- The app does not render `not_started`: the item says the app item may come later.
- `ts-0142-ua` (the settle race) is untouched. `ts-0138-pn` stays blocked by the items it already names; this one is
  now done. None of `ts-0144-eh` to `ts-0151-fk` changes.

### At review

- Decision 4 taken: no source of the line started means exit 1, scratch tasks or not. The scratch source always runs, so
  counting it would make a broken plan source exit 0.
- Decision 6 applied: Stage 4c's CLI bullet names the stderr line and the exit 1.
- Follow-up `ts-0152-jq`: a line that does not load (`board_plan` fails), or checkouts that cannot be listed, is still a
  silent empty board.
- Mutations, each red then restored: `failing` kept after a start (the daemon test), no reason kept (the daemon test),
  exit 1 on any unstarted source (three CLI tests), the stderr line's words (three CLI tests), exit 0 when none started
  (the CLI test). Under that last one, the ts-0120 queued-write test also failed once: the `settle` race of
  `ts-0142-ua`.
