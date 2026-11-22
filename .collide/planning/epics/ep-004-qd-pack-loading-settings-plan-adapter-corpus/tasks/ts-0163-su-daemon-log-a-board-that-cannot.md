---
id: ts-0163-su
kind: task
status: done
title: "Daemon log: a board that cannot be read"
owners: []
links: []
---

## Goal

Two kinds of noise in the real daemon's log, seen in `ts-0138-pn` (2026-11-21):

- A board whose folder was deleted (two boards of earlier checks, under `/tmp`) is tried again for ever: the corpus logs
  "its sources are not read" for each, about every 10 s, as long as the daemon runs (`corpus_service.rs`).
- When the daemon stops, each board's plan source logs "plan could not be listed: ... `list` came before `initialize`"
  (`supervisor.rs`): a relist races the providers' shutdown.

## Success criteria

- A board whose folder is gone is logged once when it goes, and again only when it changes (comes back, or is removed
  from the daemon). Its retries back off.
- A stop logs no listing failure for a source that is being shut down.

## Tests

- Write first and see red: a daemon with a board whose folder is removed: one log line over several retry periods.
- A stop with a board listing in flight: no "could not be listed" line.

## Notes

- Recommendation: back off to a long period, and keep the board in `board.list` with its reason, so the app can show
  it. Alternative: drop the board from the daemon when its folder is gone. That loses its runs' link to it.
- Low priority.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers.

### Built

The corpus logs a board that cannot be read once and looks at a board whose folder is gone far less often; `board.list`
gives the board's reason; a stop no longer logs a listing failure. `host/` and `core/`.

**The causes.** `BoardCorpus` (`host/src/corpus_service.rs`) looks at its board every `recheck` (10 s). A look whose
board line or checkouts could not be read logged "its sources are not read" and tried again at the next look, with no
memory of the last reason. At a stop the daemon sets its stop flag, then ends the providers, then joins the corpus
threads. A relist in flight is still running when its provider ends: its next page, or its next call, fails ("the
daemon is shutting down", or "`list` came before `initialize`" from `SupervisedProvider::call` after `stop`), and the
corpus logged each as "could not be listed".

**The change.**

- `host/src/corpus_service.rs`: `Inner::note_unread` records the reason of a look that could not read the board and logs
  only when it differs from the reason the last look gave. `Inner::note_read` clears it and logs "its sources are read
  again" if there was one. A folder that is gone fails first in the line's read or first in git's, with different
  texts, so while the folder is not a directory the reason is one fixed text. `look` uses both;
  `startup` already set the reason, so its first look is the baseline.
  `Inner::period` is `recheck`, or `GONE_PERIODS` (60) times it while the board is unread and its folder does not exist;
  `wait_for_next_look` waits that long. A wake (`BoardCorpus::wake`) still ends the wait. `BoardCorpus::sources_unread`
  gives the reason. `Inner::log_list_failure` does not log while the daemon is stopping; the start's and `relist`'s
  list failures use it. The cause is the failure, not the text: a list that fails during a stop is not a failure of the
  source.
- `core/src/owner/board.rs`: `BoardSummary.sources_unread: Option<String>`, optional and skipped when absent, so an
  older client reads the answer. `pnpm gen` was run; the four generated TypeScript files and four schemas are its
  output.
  `host/src/board_lines.rs` sets `None`; `board_list` in `host/src/daemon.rs` fills it from the board's corpus.
- `host/tests/fixtures/corpus_pack_provider.mjs`: a `list` that starts to wait for `hold-list` writes `holding`, so a
  test knows a list is in flight.

**Tests** (`host/tests/daemon_corpus.rs`), each waiting on a condition with a deadline:
`a_board_whose_folder_is_gone_is_logged_once_over_many_looks_and_again_when_it_is_read_again` (folder moved aside, then
back; three long waits pass between),
`board_list_keeps_a_board_whose_folder_is_gone_with_its_reason_until_it_is_read_again`,
`a_stop_with_a_listing_in_flight_logs_no_listing_failure` (a one-task page, the list held, `Daemon::stop` on a thread,
the hold lifted once "daemon stopped" is in the log: the list ends with the stop flag set, at its second page).

**Seen red at runtime.** The first, against the code as it was: 135 "its sources are not read" lines, where 1 was
expected. The third: "docs could not be listed: the daemon is shutting down" in the log. The second, with
`BoardCorpus::sources_unread` stubbed to `None`: it timed out waiting for the reason. The existing tests of an unread
line (`a_line_that_does_not_load_says_why_in_the_listing_until_it_loads_again` and its neighbors) pass as before.

### Decisions for the owner

- **The long period is 60 times `recheck`, 10 minutes** (`GONE_PERIODS`). A folder that comes back is read again within
  it. Alternatives: 1 hour (quieter, a slower return); a separate daemon option for the period (a test would not need a
  short `recheck`, but it is one more option). Recommendation: keep 10 minutes.
- **Only a board whose folder is gone backs off.** A board that is unread for another reason (a `board.yaml` that does
  not load, mid-edit) keeps the 10 s look, so a fix is seen at once. Its repeats are still not logged. Alternative:
  back off every unread board; a person who fixes a line would wait up to 10 minutes. Recommendation: as built.
- **"Changes" means the reason text differs.** A reason that changes (a folder gone, then a folder that is not a
  checkout) logs again, as does the board coming back ("its sources are read again"). Alternative: log only the first
  and the return. Recommendation: as built.
- **The stop is fixed in the corpus, not the supervisor.** Any list failure while the daemon stops is not logged.
  `SupervisedProvider::call` still answers "`list` came before `initialize`" to a call after `stop`; that is right for
  any caller that is not the corpus. Alternative: a distinct error from the supervisor after `stop`, mapped to
  `SourceError::Unavailable`, which the corpus would then treat as a stop. It changes `ProviderError`, which `core`
  owns. Recommendation: as built.
- **The field is `BoardSummary.sources_unread`**, the name `task.list` already uses for the same reason. Alternative: a
  field on `LineSummary`; the folder is gone with no line, so the board is the right place.

No plan text change is proposed.

### Not done, by design

- **The app does not show the reason.** `board.list` carries it; a board switcher row that says "folder not found" is a
  later item in the app's board switcher.
- **"Removed from the daemon" logs nothing.** The daemon has no way to remove a board yet (`Corpora` has no remove, and
  the store has no delete). When one is added, it should end the board's corpus and log once.
- **`board.open` of a folder that came back does not wake the corpus.** The board is read again at the next look, within
  10 minutes. `Corpora::ensure` runs on each task request, so a wake there would end the back-off; a wake only from
  `board.open` is a small later change.
- **`board.open` does not fill `sources_unread`**; only `board.list` does.
