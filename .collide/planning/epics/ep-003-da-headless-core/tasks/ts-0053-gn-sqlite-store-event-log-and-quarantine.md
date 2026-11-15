---
id: ts-0053-gn
kind: task
status: ready
title: "SQLite store, event log and quarantine"
owners: []
links:
  - { type: blocked-by, to: ts-0050-xc }
---

## Goal

The SQLite store of `host/src/store.rs`: open with migrations, the event log (`append_event`, `events_since`), and quarantine of a store that cannot be read. Replaces the stubs tagged `store: open and migrate`, `store: event log` and `store: quarantine`.

## Success criteria

- A new store migrates to the current schema; opening it again runs no migration; a store from an older schema version migrates forward, with a test per migration.
- Events come back from `events_since` in the order they were written, with the actor and time they were written with.
- An unreadable store (garbage bytes, a truncated file) is moved aside, reported with its new path, and a fresh store opens in its place. The moved file is never overwritten (Stage 3a criterion; 0.2, Durability).
- A write that fails with `ENOSPC`, from a writer that fails on demand, returns an error and does not panic (0.2, Durability).
- State is saved without holding a lock that other work waits on (0.2, Durability).
- No stub tagged `store:` remains.

## Tests

Write first: migrations, event order, quarantine of a garbage and a truncated store, the on-demand `ENOSPC` writer.

## Notes

- IMPLEMENTATION_PLAN.md, Stage 3a (SQLite store and event log); 0.2 (Durability); 0.8 (the data model).
- Area: `host` (`store.rs`). The SQLite dependency belongs to `host`, never `core` (the dependency check enforces it).
- Stage 3a item (revision 13 scope, split 2026-11-15). Items of Stage 3a run one at a time, in id order, because they share `host` and the generated files.
