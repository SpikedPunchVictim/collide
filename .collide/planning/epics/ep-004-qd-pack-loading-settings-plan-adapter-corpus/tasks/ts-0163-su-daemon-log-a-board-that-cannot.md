---
id: ts-0163-su
kind: task
status: ready
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
