---
id: ts-0141-sn
kind: task
status: ready
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
