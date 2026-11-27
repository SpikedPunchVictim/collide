---
id: ts-0221-pp
kind: task
status: done
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

### Built

- The wire. `core/src/owner/board.rs`: `MissingReason` (`gone`, `not-a-checkout`, `other-repository`, kebab-case) and
  `BoardSummary.missing_reason` beside `missing`, absent when the board is not missing. `missing` is true for all
  three, so a page that reads only `missing` works as before. `pnpm gen` rewrote `schemas/Board*Result.*.json` and
  `packages/pack-sdk/src/generated/Board*Result.*.ts`; nothing generated was edited by hand.
- The host. `host/src/boards.rs`: `NotACheckout` (`Gone`, `NotInOne`, `Unknown`) and `checkout_or_why`;
  `checkout_of` is now `checkout_or_why` with the message, so its other callers are unchanged. `NotInOne` is what
  the old refusal meant: `git rev-parse --show-toplevel` ran and said no. A `git` that did not run, or a checkout
  that cannot be read, is `Unknown`. `is_other_repository` (some commits found, none recorded). `host/src/daemon.rs`:
  `board_folder` (used by `board_list`) gives the checkout and the reason. `Gone` and `NotInOne` mark the board;
  `Unknown` keeps today's result (no line, not missing). A board with recorded commits whose folder's `HEAD` has
  commits and none recorded is `other-repository`, and has no line. `host/src/board_lines.rs` sets
  `missing_reason: None` in its two summaries. `board.open` and `board.locate` are unchanged.
- The page. `app/src/loop/types.ts` `MissingReason`; `app/src/frame/boards.ts` `missingReason` (a board with no
  reason is `gone`, as an older daemon gives); `app/src/frame/MissingBoard.tsx` takes `reason` and says "its folder is
  gone", "its folder is no longer a git checkout" or "its folder now holds another repository", each with what
  Collide found, and the same Locate button; `app/src/App.tsx` passes it.
- Tests.
  - `host/tests/board_identity.rs` (new, 9): `a_board_whose_folder_is_gone_says_so_with_the_reason_gone`,
    `a_board_whose_folder_is_there_and_is_its_checkout_has_no_reason`,
    `a_folder_made_again_with_no_checkout_in_it_is_missing_because_it_is_not_a_checkout_and_locate_finds_the_board`,
    `another_repository_in_the_folder_is_missing_because_it_is_another_repository_and_has_no_line`,
    `a_board_with_no_record_of_its_repository_is_never_another_repository`,
    `a_board_with_no_record_is_still_missing_when_its_folder_is_gone_or_is_not_a_checkout`,
    `a_repository_with_no_commit_yet_in_the_folder_is_not_marked_another_repository`,
    `with_no_git_on_the_path_a_folder_that_is_there_is_not_marked_missing`,
    `with_no_git_on_the_path_a_folder_that_is_gone_is_still_gone`. `start_with_git_env` is the one helper added; the
    no-git tests start the daemon again with a `PATH` of an empty folder.
  - `app/test/missing-board.test.tsx` (new, 8 of 15 in the file): the words per reason, Locate and the kept tasks per
    reason, and `missingReason` (the daemon's reason, and `gone` when none).
  - `app/browser/boards.spec.ts` (bridge, 3 new): an empty folder made again in the place shows the not-a-checkout
    page and locating opens the board with its task; another repository in the place shows its page with Locate; a
    gone folder still says gone. Each also reads `missing_reason` from `board.list`.
- Red at runtime, against the old behaviour (a board missing only when its folder is not a directory):
  `a_folder_made_again_with_no_checkout_in_it_...`, `another_repository_in_the_folder_...` and
  `a_board_with_no_record_is_still_missing_...` failed. With `Unknown` stubbed to `not-a-checkout`, and a missing
  record stubbed to an empty list, `with_no_git_on_the_path_a_folder_that_is_there_is_not_marked_missing` and
  `a_board_with_no_record_of_its_repository_is_never_another_repository` failed. With the "some commits found" test
  taken out of `is_other_repository`, `a_repository_with_no_commit_yet_in_the_folder_is_not_marked_another_repository`
  failed. In vitest, the two words tests for
  the new reasons and the `missingReason` test failed against a stub that always said `gone` with the old words. In the
  bridge, the empty-folder and other-repository tests failed with the daemon stubbed to the old rule. The stubs were
  removed and the files compared with copies afterwards. The gone, own-checkout and gone-with-no-git tests are
  guards: they pass before and after.

### Decisions for the owner

- **A reason beside `missing`** (the item's recommendation), kebab-case, as the item spells it. Alternative: two
  states. Recommend: as built.
- **"Not a checkout" is what `git rev-parse --show-toplevel` says by failing** (any non-zero exit), as the refusal in
  `board.open` has always meant. A `git` that does not run (not found, a folder it cannot enter) is not that. The
  alternative is to read git's words ("not a git repository") on stderr, which would also keep a checkout git refuses
  for another reason (for example a folder owned by another user) from being marked. Not built: it needs a second
  `git` runner. Recommend: as built, and a wrong Locate offer costs nothing, since the board is never changed.
- **A checkout with no commit yet is not `other-repository`.** Collide cannot tell it from the board's repository,
  and `board.locate` refuses it for the same reason. The board then lists as it did before, with its line. Alternative:
  mark it. Recommend: as built.
- **A missing board has no line, also for `other-repository`.** The checkout there is not the board's, so its branch is
  not shown as the board's line. Alternative: keep the other repository's line. Recommend: as built.
- **The board menu's row still says "folder not found" for all three reasons** (the mock's term, note 15). Only the page
  changes. Alternative: a row word per reason ("no longer a checkout", "another repository"). Recommend: as built for
  now; the row is a hint and the page is where the person reads the cause.
- **The cost.** `board.list` runs one more `git rev-list` for each board whose folder is a checkout and that has
  recorded commits. Alternative: cache by folder and `HEAD`. Recommend: as built until a list is seen to be slow.
- **No fake-owner spec.** The fake world's missing board has no reason, which the page reads as `gone`, so its specs
  are unchanged; the words per reason are the vitest tests and the bridge tests drive a real daemon. Alternative: add
  a fake world per reason. Recommend: as built.
- **Plan text to propose** for Stage 6a, Line selector: "A board is missing when its folder is gone, when git says the
  folder is in no checkout, or when its checkout's root commits are none of the board's recorded ones. `board.list`
  says which with `missing_reason` (`gone`, `not-a-checkout`, `other-repository`) beside `missing`, and the page says
  so and offers Locate in all three. A git read that fails for another reason marks nothing."
- Review (2026-11-27): the recommendations above are taken. One fix in review: a folder made again inside another
  checkout (a home folder kept in git, say) has that checkout's top, not the board's root, so `board_folder` read it
  as the board's checkout. A board with recorded commits then showed `other-repository`; one with none recorded took
  the outer checkout's commits as its own. `board_folder` now marks a folder whose top is not the board's root
  `not-a-checkout`, before any commit is read or recorded (`board.open` and `board.locate` keep the root at the top).
  Test: `a_folder_made_again_inside_another_checkout_is_not_a_checkout_and_its_commits_are_not_recorded`, red without
  the check. Plan text: the proposed Line selector sentence goes in with the next plan revision.

### Not done, by design

- The board menu row words per reason (see Decisions).
- Removing a board (the page's Remove button stays off); `ts-0197-uj` finding 7 still stands.
- `ts-0197-uj` step 22 is not edited; it is the owner's to redo with this build.
