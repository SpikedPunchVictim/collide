---
id: ts-0155-wb
kind: task
status: ready
title: "CLI: the first listing after a daemon restart waits for the whole relist"
owners: []
links: []
---

## Goal

After a daemon restart, the first `collide tasks` takes as long as a cold start (5.5 s against 5.7 s on 1,659 tasks, the
agent run of `ts-0121-xp`), though the store's corpus cache (`ts-0116-wh`) is served at once. `ts-0150-dj` found why:
the daemon answers from the cache with `refreshing: true` in about 0.07 s, and `Session::settled_list` in
`host/src/task_daemon.rs` asks again until the relist of every source for every checkout ends. Nothing is wrong; the
wait is unexplained.

## Success criteria

- Recommended: the CLI keeps waiting for the settled list (a script never gets a list that may be stale), and while it
  waits on a listing that is `refreshing` for longer than 0.5 s it prints one line on standard error saying the daemon
  is listing its sources again after a restart. Standard output is unchanged; `--json` is unchanged.
- Alternative, if the owner takes it: the CLI prints the cached list at once with a standard error line that it is the
  last run's list, and `--wait` keeps today's behaviour. Faster, but a script reading the first output may act on a
  stale list.

## Tests

- Red at runtime first: a daemon test over a restarted daemon whose relist is held back (a provider that answers `list`
  slowly) shows the standard error line, and none when the list settles within 0.5 s.

## Notes

- Found by `ts-0150-dj` (Decision 5), measured through `host/tests/daemon_corpus.rs` with temp roots: cold 3.79 s; after
  a restart the first answer in 65 to 73 ms with all 1,600 tasks, the settled one after 1.24 to 1.37 s.
- Owner decisions: 2026-11-19, the recommendation (keep waiting, explain the wait on standard error).

### Built

- The recommended criterion: the CLI keeps waiting for the settled list, and says so once on standard error when the
  wait passes 0.5 s. Standard output, `--json` output and the exit code are unchanged.
- `host/src/task_daemon.rs`:
  - `WAIT_NOTICE_AFTER` (0.5 s, public) and `WAIT_NOTICE`, the words:
    `collide: the daemon is still listing the board's sources; waiting for the full list`.
  - `Session` has `notice_after` and `said_waiting`. `Session::settled_list` takes the command's standard error and
    writes the line once per command, on the first `refreshing` answer once the wait has passed `notice_after`. The
    line is not repeated at the retries, and a listing that settles sooner prints nothing.
  - `run_with` is unchanged for callers and passes `WAIT_NOTICE_AFTER` to the new `run_noticing_after`, which takes the
    threshold. A test sets it; no test depends on how long a listing takes.
  - `err` is now passed to `name_in_board`, `key_of` and `resolve_epic`, which reach `settled_list`.
- Which commands wait: every caller of `settled_list`, so the line applies to `collide tasks` (also `--view`, `--ready`,
  `--kind`, `--checkout`), `task <id>` (a key, or a bare or short id), a write on a key or id, `task create` with an
  `epic`, `task reid` and `sections`. `task new` and `task renameSection` do not list and do not wait.
- `host/src/task_cli.rs` `HELP` and `host/tests/fixtures/task-cli-help.txt`: one sentence on the wait and the line.
- `host/tests/fixtures/corpus_pack_provider.mjs`: `hold-list` in the pack data folder holds a `list` answer until the
  file is gone (30 s at most).
- Tests, `host/tests/daemon_corpus.rs`, over a restarted daemon whose first relist is held back:
  - `a_command_waiting_for_the_relist_after_a_restart_says_so_once_and_prints_what_it_would_have` for `tasks`,
    `tasks --json`, `task files:1` and `task 1`: the line is there, exactly once after the command has asked again for
    half a second, the command is still waiting, and after the release its exit and standard output equal those of the
    same command on the settled daemon, which says nothing.
  - `a_listing_that_settles_before_the_notice_time_says_nothing`.
  - `the_notice_comes_after_half_a_second`.
- `host/tests/task_daemon_cli.rs`: its local `run_with` calls `run_noticing_after` with an hour. Those tests compare
  standard error with `""` over a daemon that has not listed yet, so the 0.5 s line must not depend on load there.
- Red at runtime before the code (the print disabled, the rest in place): the first test (it timed out waiting for the
  line). The second and third are characterisations, green first.
- Mutations, each red then restored: no `said_waiting` set (the first test: the line six times), no threshold check
  (the second test: the line printed).
- Real waits in tests: after the line appears, the first test lets the command ask for 0.5 s more (a lower bound on
  elapsed time, so load cannot fail it), and the second does the same before it releases the list. The code has no
  clock to set, only the threshold, so the line needs no real 0.5 s.

### Decisions for the owner

1. The words say "still listing the board's sources", not "again after a restart" as the item's draft did: a first
   start with no cache waits the same way, and the line is then not "again". Alternative: the item's words. Recommend
   these.
2. The line applies to every listing the commands wait on, not only `collide tasks`, because they share
   `settled_list`. Alternative: `tasks` only. Recommend all: a write or `task <id>` waits as long.
3. Once per command, not once per listing: `create` with an `epic` waits twice and says it once.
4. The threshold is a parameter of `run_noticing_after`, not a clock. Alternative: a fake clock in `Session`. Recommend
   the parameter: smaller, and the test needs no time at all to get the line.
5. It is counted from the start of the command's wait, and the line is written with the first `refreshing` answer after
   0.5 s. A daemon that answers slower than 0.5 s by itself is not `refreshing` and says nothing.
6. Proposed plan text, Stage 4c CLI bullet, after the sentence on `sources_unread`: "A listing the daemon is still
   refreshing (its first one after a start) is waited for, and one standard error line, `collide: the daemon is still
   listing the board's sources; waiting for the full list`, is printed once when the wait passes 0.5 s. Standard output
   and the exit code are unchanged." Not applied: the plan is the reviewer's.

### Not done, by design

- The alternative (print the cached list at once, with a `--wait`) is not built; the owner took the recommendation.
- The app does not show the wait; it has the `refreshing` flag on the wire already.
- `ts-0158-ow` (Orchard `get` scans per fetch) is unaffected: it is about the daemon's fetch under the gate, not the
  CLI's wait.

### At review

- Reviewer mutations, each red then restored: `task <id>` resolving its id without the command's stderr, and `tasks`
  listing without it (the restart test each).
- The builder's one failing run, `a_listing_says_why_the_line_cannot_be_read_still_lists_what_it_holds_and_exits_1`
  (`ts-0152-jq`), counted every stderr line: with the line broken, the plan source may also fail to refresh and print
  its own "may be out of date" line, so the count was 2 under load. Fixed test-only: that test now leaves the source's
  out-of-date line out of the count. Not caused by this item.
- Decisions 1 to 5 taken as recommended (owner delegation). Decision 6's text applied, shortened, to the plan's 4c CLI
  bullet.
