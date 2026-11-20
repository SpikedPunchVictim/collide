---
id: ts-0152-jq
kind: task
status: ready
title: "CLI: a listing says when the board's line cannot be read"
owners: []
links:
  - { type: blocked-by, to: ts-0141-sn }
---

## Goal

When the daemon cannot read the board's line (`board_plan` fails in `look`, `host/src/corpus_service.rs`) or list its
checkouts (`discover`), it logs the error and returns. No source of the line is named, so `collide tasks` lists the
scratch source alone, prints nothing on stderr and exits 0: the silent empty board of `ts-0141-sn`, one step earlier.
Found at the `ts-0141-sn` review (the builder's Not done).

## Success criteria

- `task.list` carries, on its first page, that the board's sources are not read and why.
- `collide tasks`, `tasks --view` and `tasks --ready` print one stderr line with the reason and exit 1.
- A board whose line reads again says nothing at the next look.

## Tests

- Write first and see red at runtime: a daemon test whose board names a line that does not load gets the reason in
  `task.list`.
- A CLI test over the same board: the stderr line and exit 1; then the line fixed, and the line gone.

## Notes

- Recommendation: keep the last error of `look` in the corpus service's state, cleared by a look that reads, and put it
  on the wire beside `not_started`. Alternative: a `not_started` entry named for the line, which reads as a source that
  is not one.
- Match the words `board.open` uses for the same board, if it names the error.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers at the split.

### Built

- Wire (`core/src/owner/task.rs`): `TaskListResult.sources_unread: Option<String>`, additive, first page only, left out
  of the JSON when `None`. `core/src/corpus.rs` gains the empty field in its two list constructors. `pnpm gen`
  regenerated `schemas/TaskListResult.*.json` and `packages/pack-sdk/src/generated/TaskListResult.*.ts`.
- Daemon (`host/src/corpus_service.rs`): `State.unread` holds the last reason. `startup` and `look` set it when
  `board_plan` or `discover` fails (the line's error first); `look` clears it once both read, before the sources are
  looked at. `BoardCorpus::list` copies it beside `not_started` and `sources_started`.
- CLI (`host/src/task_daemon.rs`, `tasks`): one stderr line, `collide: the board's sources could not be read:
  <reason>`, after the stale lines; the exit is 1 (a view error's 10 still wins). The help text (`host/src/task_cli.rs`)
  and its stored copy (`host/tests/fixtures/task-cli-help.txt`) say so.
- Tests. Daemon, `host/tests/daemon_corpus.rs`: `a_line_that_does_not_load_says_why_in_the_listing_until_it_loads_again`
  and `a_line_that_does_not_load_at_the_start_says_why_before_any_look` (a 30 s recheck, so only the start can name it).
  CLI, `host/tests/task_daemon_cli.rs`:
  `a_listing_says_why_the_line_cannot_be_read_still_lists_what_it_holds_and_exits_1`,
  `a_line_that_is_fixed_or_gone_says_nothing_at_the_next_look`,
  `json_carries_the_reason_the_line_is_unread_and_a_board_that_reads_has_none`, and
  `a_view_or_ready_over_a_line_that_cannot_be_read_is_refused_with_the_lines_problem_and_exits_1`.
- Red at runtime before the code (the field existed, the daemon never set it, the CLI never printed it): both daemon
  tests and the first three CLI tests. The view test is a characterisation of what the daemon already did: green first.
- Mutations, each red then restored: the clear in `look` (the daemon test), the set in `startup` (the start test), the
  set on a `board_plan` failure in `look` (the daemon test), exit 1 on unread (two CLI tests), the stderr line (three
  CLI tests). Survived: the set on a `discover` failure in `look` (see Not done).

### Decisions for the owner

1. The wire carries one `Option<String>`, `sources_unread`, not a list: a look stops at its first failure. Alternative
   (the item's): a `not_started` entry named for the line, which reads as a source that is not one. Recommend the field.
2. `startup` records the reason as well as `look`. The item names `look`; without the start, a daemon that starts over a
   bad line says nothing until its first recheck. Alternative: `look` only. Recommend both.
3. The words: `collide: the board's sources could not be read: <reason>`, beside `source <name> could not start`. The
   reason is the daemon's own text: for a line that does not load, it is the loader's problem text (what `board.open`
   gives in `problems`) behind the daemon's "the board's line does not load, so its sources are not started". Not
   rewritten, so the daemon log and the CLI read alike.
4. Exit 1 even when the scratch source lists tasks, as `ts-0141-sn` decision 4 does for a board whose sources all
   failed: the board's planning is unknown. Alternative: exit 0 when something is listed. Recommend 1.
5. A board whose line fails and whose scratch source lists: stdout lists the scratch tasks and what the other sources
   last listed, stderr has the one unread line (plus a `could not start` or `may be out of date` line when a source has
   one), exit 1. The test covers it.
6. `tasks --view <name>` and `tasks --ready` already exit 1 on such a board: a view comes from the line, so the daemon
   refuses the listing with the loader's text (`collide: the daemon said: <text>`). The item's wording asks for the same
   line from them. Not done: the refusal is a different path, and the line says why. Alternative: have the CLI print
   the unread line for that refusal. Recommend leaving it.
7. While the line is unread, `look` leaves `failing` as it was (it returns before the retain that drops sources the
   line no longer names). Old `not_started` entries may still print with the unread line. Recommend leaving it.
8. Proposed plan text, Stage 4c CLI bullet, after the sentence on a source that could not start: "A board whose line
   or checkouts the daemon could not read at its last look says so on stderr (`collide: the board's sources could not
   be read: <reason>`), lists what the corpus holds, and exits 1; a look that reads clears it. A view named on such a
   board is refused with the line's problem and exits 1." Not applied: the plan is the reviewer's.

### Not done, by design

- A `discover` failure (git cannot list the worktrees while `board_plan` read the line) sets the same field but has no
  test: both calls run git in the same folder, so the test rig cannot make only the second fail. A mutation of that set
  survives. A seam for `discover` would be its own item.
- The app does not render `sources_unread`; the field is on the wire, as `not_started` was.
- `ts-0155-wb` (the wait after a restart) touches `Session::settled_list` and the same stderr lines: it reads
  `refreshing`, which this item does not change. `ts-0153-kp`, `ts-0156-nt` and `ts-0158-ow` are unaffected.

### At review

- Decisions 1 to 7 taken as recommended (owner delegation). Decision 8's plan text applied, shortened, to the 4c CLI
  bullet after the `ts-0141-sn` sentence.
- Accepted untested: the `discover` failure in `look` (the builder's mutation of it survived); it sets the same field as
  the `board_plan` failure beside it, which is tested.
- My mutation: `BoardCorpus::list` not copying the reason, red in the two daemon tests and the CLI tests.
- The builder wrote one `cargo test` output to a fixed file in the system temp folder and removed that file after.
