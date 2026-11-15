---
id: ts-0053-gn
kind: task
status: active
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

### Built

- `host/Cargo.toml`: `rusqlite` 0.42 with the `bundled` feature, in `host` only (the dependency check passes; `core` is untouched). No other dependency.
- `host/src/store.rs`: `Store::open` (makes the folder, quick-checks the file, switches it to WAL, runs `migrate`), `Store::append_event`, `Store::events_since`, `Store::last_seq`, `Store::write_events_since` (the log as JSON lines through `std::io::Write`), `quarantine` (and `quarantine_at`, `quarantine_database`), `MIGRATIONS` (one: the `events` table), `StoreError::OutOfOrder`.
- Test count in `store.rs`: 41 (the four stub-era tests became 3 kept, 1 removed: the `should_panic` stub test; 38 added). They cover: a fresh store migrates, reopening runs none (`migrate` returns 0), a store from each older version migrates forward (looped over `MIGRATIONS`, so one case per migration), migrations run in order from the stored version, a failing migration leaves the version before, `TooNew` leaves the file where it is; event order, actor, experiment and wall time kept (a later event with an earlier time), `events_since` filtering, gap, repeat and first-number refusals, a line a newer build wrote; the on-demand `ENOSPC` writer failing on each write in turn and on `flush`, and SQLite's own full disk (`PRAGMA max_page_count`); quarantine of garbage, a few bytes, a half-truncated store and a store cut to 512 bytes, the moved file kept whole, a second quarantine of the same name getting a different path (and a third), the name holding time and reason, a WAL file moving with its database; no lock: `Store` is `Send + Sync`, nothing is held between calls (an exclusive transaction from another connection starts at once), an append finishes while another connection sits in an open read transaction, reads and appends run in two threads without a gap.
- `checks/tests/real_tree.rs`: the three `store:` tags moved from `HOST_OUTLINE_TAGS` (21 to 18) to `FILLED_TAGS` (5 to 8) as `(tag, "3a", "ts-0053-gn")`. `stub-tags.json` is unchanged; the tags count 0 stubs.

### Decisions for the owner

1. **The event log is a SQLite table (`events`: `seq` primary key, `wall_ms`, `line` = the event's JSON), and the `std::io::Write` path is `write_events_since`, which streams stored lines out as JSON lines.** Passed on: a JSON-lines file beside the database, appended through `Write`. A file would need its own gap check, its own crash handling for a half-written last line, and a second place to keep in step with the database. In the table, an append is one atomic statement that takes the number only if it is the next. The cost: the `ENOSPC`-on-demand writer exercises the log's output, not its append. The append meets a full disk through SQLite's own `SQLITE_FULL`, which is also tested on demand (`max_page_count`) and is the same `StoreError::Io` with kind `StorageFull`. Recommend: keep.
2. **No lock other work waits on: a `Store` holds only its folder, and every call opens its own connection (WAL, 5 s busy timeout), runs one short statement and closes.** Passed on: one `Mutex<Connection>` (boring, but every caller waits on it, which is what 0.2 rules out) or a connection pool (more code for volumes this small). Opening a connection costs microseconds against an append that happens a few times a second. Recommend: keep; revisit only if the event rate is measured to matter.
3. **`StoreError::OutOfOrder { expected, found }` is new.** The stub said a gap or a repeat is "refused" but had no variant for it. Recommend: keep.
4. **The first event is number 1**, so `events_since(0)` is the whole log (a fresh client has seen nothing). The shim's journal starts at 0; this log differs on purpose. Recommend: keep.
5. **`open` keeps the stub's shape: a store it moved aside is `Err(Quarantined { moved_to, .. })`, and the next `open` makes the fresh store.** Passed on: returning `Ok` with the report attached, so a caller starts in one call. That would change the signature; it is the caller's (the daemon's) shape to choose when it is built. This one needs no change now and cannot ignore a quarantine by accident. Recommend: keep until the daemon item says otherwise.
6. **Unreadable means SQLite reports `NOTADB` or `CORRUPT`, or `PRAGMA quick_check` returns anything but `ok`.** Permission errors, a full disk and a store from a newer build (`TooNew`) are errors and never move the file. The quick check reads the whole file, so opening a store costs time in proportion to its size. Recommend: keep; if the log grows to gigabytes, run the check on a schedule instead.
7. **A store's WAL file gets a second name (a hard link) before the open, and moves with the database when it is quarantined.** Measured: the bundled SQLite deletes the `-wal` file of a database it cannot read, and a WAL holds committed writes the database file does not. Without the link, "never deletes" would hold for the database file and not for the writes beside it. The `-shm` index is rewritten in place and holds nothing, so it moves if it is still there. Recommend: keep.
8. **Quarantine moves with a hard link and then removes the old name, not `rename`**, because `rename` replaces an existing file silently and a link fails with `AlreadyExists`, so the next name (`-2`, `-3`) is tried. A filesystem without hard links gets a `StoreError::Io`. Recommend: keep.
9. **Durability setting: SQLite's default `synchronous` (FULL) is kept**, not `NORMAL`. With WAL, `NORMAL` can lose the last committed events on power loss; the event log is the recovery source (0.8), so it is paid for. Recommend: keep.
10. **rusqlite 0.42 and not the newest, 0.43**, to meet the age rule; both need Rust 1.89, and the toolchain is 1.98. Recommend: move to 0.43 when the owner likes; no API used here changed between them as far as the docs say, but it was not tried.

- Owner decisions (2026-11-15): the recommendations above are accepted as built. Follow-up for the daemon item (ts-0056-ag): with no connection held, the last close of each call checkpoints and removes the WAL; measure the append cost under a real event rate before keeping one connection per call.

### Not done, by design

- No tables beyond `events`. Runs, phases and workspaces get their migrations in the items that need them (the numbering allows it, and the per-migration test loops over `MIGRATIONS`).
- Nothing calls the store yet: the daemon item (Stage 3a) opens it, assigns event numbers (`last_seq() + 1` at start), reports a quarantine to the log and the Queue, and decides how `TooNew` reaches the owner.
- Retention and pruning of the event log, and an index on `wall_ms`, wait until a reader needs them.
- The shim's `exit.json` is quarantined by the shim item, which calls `quarantine`; the function here is ready for it and tested on a plain file.
