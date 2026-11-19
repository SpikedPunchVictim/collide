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
