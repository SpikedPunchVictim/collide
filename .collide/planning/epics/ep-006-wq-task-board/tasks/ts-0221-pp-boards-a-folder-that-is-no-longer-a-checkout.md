---
id: ts-0221-pp
kind: task
status: ready
title: "Boards: a folder that is no longer a checkout"
owners: []
links:
  - { type: blocked-by, to: ts-0220-oc }
---

## Goal

A board whose folder is there but is no longer its git checkout gets the same page as a board whose folder is gone,
with "Locate the folder…", and says which of the two it is.

What exists.
- `host/src/daemon.rs` `board_list`: `missing` is true only when the folder is not a directory. A folder that exists
  but is not a checkout is listed with no line (`boards::checkout_of` fails), not as missing.
- `app/src/App.tsx` shows `MissingBoard` (`app/src/frame/MissingBoard.tsx`) only when `isMissing`. Otherwise it opens
  the board, and `host/src/boards.rs` refuses: "`<folder>` is not inside a git checkout, so Collide cannot open it as
  a board."
- `ts-0197-uj`, the owner's run, step 22: a stray daemon whose `HOME` was the moved folder made it again, holding only
  `Library/`. The board showed the refusal, not "its folder is gone", and Locate was not offered.
- A board knows its repository's root commits (`ts-0191-nv`), and Locate checks a picked folder against them.

What this adds.
- `board.list` marks a board missing when its folder is gone or is no longer a checkout of the board's repository: not
  a checkout at all, or a checkout whose root commits differ from the board's. Each says why (a reason field, or two
  states).
- `MissingBoard` says which: "its folder is gone" or "its folder is no longer this repository's checkout", and offers
  Locate in both.
- A board whose `git` read fails for another reason (`git` not found, a timeout) is not marked missing: it keeps
  today's error, so a passing fault never offers Locate.

## Success criteria

- Move a board's folder away and make an empty folder in its place: the board's page says the folder is no longer
  the repository's checkout, and "Locate the folder…" opens the board at its new place.
- Put a different repository's checkout in its place: the same page.
- Remove the folder: "its folder is gone", as now.
- With `git` missing from the daemon's tools path, a board is not marked missing.

## Tests

- `host/tests/` (a temp root, `env_clear`): `board.list` over a board whose folder is gone, empty, another repository
  and its own checkout; and with no `git` on the path.
- `app/test/` (vitest): `MissingBoard`'s words per reason.
- `app/browser/` (bridge): the empty-folder case shows the page and Locate opens the board (extend the boards spec).
- The schema: the reason in `schemas/` if the result changes, with `collide-checks`.

## Notes

- IMPLEMENTATION_PLAN.md: Stage 6a (Line selector: "A missing board's row says 'folder not found'"); revision 33.
- **An item decision: one flag or a reason.** Recommend a reason beside `missing` (`gone`, `not-a-checkout`,
  `other-repository`), so the page's words follow it and `missing` keeps its meaning for every client.
- A board from before root commits were recorded has none: it is marked missing only when its folder is gone or is
  not a checkout.
- The test rule: no test touches the real home, real launchd, a real daemon, the keychain or the network. Each test
  that runs the `collide` binary clears the environment and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root
  (`host/tests/common/mod.rs` `TempRoot`).
- Nothing here runs a real `claude`. New dependencies: none expected.
- Stage 6a item (revision 33, 2026-11-27), from `ts-0197-uj`'s owner run. It runs after `ts-0220-oc`, before
  `ts-0222-gx`.
