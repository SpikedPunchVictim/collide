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
- Owner decisions: none recorded yet. The recommendation (keep waiting, explain the wait) stands until the owner
  answers.
