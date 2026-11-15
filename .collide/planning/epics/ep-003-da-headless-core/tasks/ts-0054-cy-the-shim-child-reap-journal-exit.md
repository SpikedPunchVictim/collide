---
id: ts-0054-cy
kind: task
status: active
title: "The shim: child, reap, journal, exit record and logs"
owners: []
links:
  - { type: blocked-by, to: ts-0051-wh }
---

## Goal

The shim of 0.2 (`host/src/shim.rs`), up to its link with the daemon. It spawns the child with `std::process::Command` and `pre_exec(setsid)`, reaps it with `libc::wait4` on the pid, writes the journal (one ordered file of `{seq, wall_ns, dir, bytes}`, each record written before the child sees the bytes), writes `exit.json` with the exit totals, keeps the terminal ring, and writes the file-backed child logs with their caps. Replaces the stubs tagged `shim: entry`, `shim: spawn child`, `shim: reap`, `shim: journal writer`, `shim: exit record` and `shim: pty ring`.

## Success criteria

- An agent's exit is never taken from std's or tokio's `wait`, and no tokio `Child` is dropped (measured S3). A test runs a python3 child that exits with a known code and the shim records that code.
- A session replayed from the shim's journal reaches the same state as the live one, and a journal cut short replays up to the last whole record (Stage 3a criterion). This reuses `ts-0039-ps`'s replay over a journal written by the real shim.
- Writing the child's stream and logs through a writer that fails with `ENOSPC` on demand does not crash the shim. On a full disk the shim keeps its exit record in memory and serves it until it is acknowledged (0.2, Durability).
- A stream line over the cap is cut and marked. A phase log over its cap stops growing, with a final marker (0.2, Durability).
- `exit.json` carries the exit totals of 0.13. A shim that cannot write it leaves no partial file.
- No stub tagged `shim:` remains except `shim: daemon link`, which is `ts-0057-ck`.

## Tests

Write first: python3 children (normal exit, non-zero exit, a burst of output, one over-long line), the journal's order against what the child received, the cut-short replay, the `ENOSPC` writer, `exit.json` written and kept in memory.

## Notes

- IMPLEMENTATION_PLAN.md, Stage 3a (The daemon and the shim); 0.2 (the shim and the journal; Durability); 0.13 (the shim's exit totals). The S3 record for `wait4` and setsid. Hostile-process tests are adapted from herdr-core.
- Area: `host` (`shim.rs`), using `core`'s journal and frame types (`ts-0016-ka`).
- Stage 3a item (revision 13 scope, split 2026-11-15). Items of Stage 3a run one at a time, in id order, because they share `host` and the generated files.

### Built

- `host/src/shim.rs`, filling the six tags `shim: entry`, `shim: spawn child`, `shim: reap`, `shim: journal writer`, `shim: exit record` and `shim: pty ring` (now in `FILLED_TAGS` of `checks/tests/real_tree.rs`; they stay in `stub-tags.json` with count 0). `shim: daemon link` (`serve_daemon`) is still its stub.
  - `spawn_child`: `Command` with `env_clear`, the spec's environment and `pre_exec(setsid)`; for a PTY, `openpty` through `libc`, `setsid` and `TIOCSCTTY` in `pre_exec`. It writes `child.json` (`ChildIdentity`, start time from the new `sysproc::start_time_us`, which reads `p_starttime`) and kills and reaps the child if that write fails.
  - `reap`: `libc::wait4` on the pid, retried on `EINTR`; exit or signal from the status; CPU time from `ru_utime`/`ru_stime`; `ru_maxrss` converted to bytes (macOS bytes, Linux KiB). A test reads the module's own source and fails on `.wait(`, `.try_wait(`, `.wait_with_output(`, `.status(`, `.output(` or `tokio::`, and on a second `libc::wait4(`.
  - `JournalWriter`: encodes with core's `encode_record`, writes and flushes each record, and after a failed write refuses every later record, so the file never holds a whole record after a cut one.
  - `Shim` (`start`, `start_with`, `write_to_child`, `close_stdin`, `ring_snapshot`, `problems`, `finish`): one thread per pipe (`LinePump`) cuts stdout and stderr into lines with `LineSplitter`, journals stdout lines as `out` records, calls an optional `on_line` hook, and writes the logs through `CappedLog`. `write_to_child` takes the input lock, appends the `in` record, then writes to the child. `finish` calls `reap`, lets the pumps drain (at most `Limits::drain_after_exit`), then `record_exit`.
  - `write_exit_record` / `record_exit` / `ExitKeeper`: temp file, `fsync`, rename, `fsync` of the directory; a failed write removes the temp file and leaves an older `exit.json` whole; the keeper holds the record and serves `ShimMessage::Exited` until `acknowledge`.
  - `PtyRing` (last `PTY_RING_BYTES`), `ShimSpec::to_json`/`from_json`, `run` (reads `spec.json` from the run directory), `Sinks` and `Limits` (the seams tests use for the `ENOSPC` writer and the small caps).
  - `host/src/sysproc.rs`: `start_time_us` (and `parent_pid` now shares one `bsd_info` read with it). `host/Cargo.toml`: `libc` moved from the macOS-only target to `cfg(unix)` (same crate and version, no new crate); `collide-core` with the `testing` feature added as a dev-dependency, so a real journal can be replayed into the session double.
- Tests added: 37 unit tests in `host/src/shim.rs` (spec and identity JSON, journal writer including `ENOSPC` at every write and a record cut part-way, line splitter at any read size, log cap, ring, `exit.json` atomicity and keeper, `wait4` errors, the source scan) and 2 in `host/src/sysproc.rs`; 38 process tests in `host/tests/shim_child.rs` (python3 children: exit 0, exit 73, SIGKILL, `rusage` totals, exact environment, own session, burst of 20 000 lines, one 50 000-byte line, non-text bytes, journal order against the child's echo, replay equal to the live session, the journal cut at every byte, `ENOSPC` under the journal, under each log, and under `exit.json`, the line and log caps, a grandchild holding the pipe open, a PTY, and the `run` entry). Replaced the `should_panic` stub test for `reap`.

### Decisions for the owner

1. **The child's stdout and stderr are pipes the shim reads, not files the child writes.** The `ChildStdio` doc of the outline said "files for stdout and stderr". With the child writing a file, the shim could neither cut a line nor stop a log at its cap, and the journal's `out` records need whole lines read by the shim. The logs are still files in the run directory that a tailer reads. Alternative passed on: child writes the files directly and the shim tails them for the journal (no caps, a runaway child fills the disk). Cost of the choice: a SIGSTOPped shim stalls its child's output once the pipe fills (64 KiB). Recommend keeping pipes. Does not change what the owner sees; it does change the S3 mechanism.
2. **Cap sizes** (the plan gives none): `LINE_CAP_BYTES` 8 MiB per stdout or stderr line, `LOG_CAP_BYTES` 256 MiB per log file (`stdout.log` and `stderr.log` each), `DRAIN_AFTER_EXIT` 2 s. The cut line is what both the journal and the log hold, ending with `[collide: line cut, N bytes dropped]`; a full log ends with `[collide: log reached N bytes, later output is not kept]`. Alternative: smaller caps (1 MiB line, 64 MiB log). A stream-json tool result with a file in it can pass 1 MiB, and a cut line is unparseable to the session, so I picked the larger line cap. Product-visible: a long phase's log stops at 256 MiB.
3. **The journal has no cap.** Cutting it would break replay (0.2: "a session replayed from the journal reaches the same state"), so only the line cap bounds it. A run that writes 200 MiB of lines has a 200 MiB journal. Alternative: a journal cap that ends the session as failed with cause `disk`. Recommend deciding this with the `paused: disk` item. Flagged because 0.2 says "each phase's log" has a cap and the journal is a second copy of stdout.
4. **A journal that failed stays failed; an `in` write then fails and the child gets nothing.** After one failed write the writer takes no more records (no whole record after a cut one, which replay would call damage), and `write_to_child` refuses, because the journal comes first. The child keeps running and its output keeps going to the logs. Alternative: retry later when space comes back, which needs a way to mark the gap in `seq`. Recommend this behaviour; the daemon should turn the shim's `problems` into a Queue item. Same rule for a log: after a failed write it drops later output and reports the first failure once.
5. **PTY output is not journaled.** The journal is for rebuilding an ACP session from lines; terminal bytes have no lines, and the ring keeps the last 1 MiB. PTY writes (`in`) are journaled. Alternative: journal PTY output in chunks (a second unbounded copy). Recommend not.
6. **`spawn_child` returns the child's pipes (`SpawnedChild`), not just `ChildIdentity`, and `run` reads `spec.json`.** The stub's signature could not hand the shim the pipes. `ShimSpec` is carried as JSON built by hand (`serde_json::Value`), so `host` gains no `serde` dependency. The environment, which may hold tokens, is therefore in a `0600` file and never on a command line. Alternative: derive `Serialize` on `ShimSpec`, which needs `serde` as a direct dependency.
7. **The files are created with `create_new` and mode `0600`**: the journal, both logs, `exit.json` and `child.json` (the last two through the temp file). A run directory that already has a journal is refused, never written over; a stale `exit.json.tmp` from a dead shim is replaced. Alternative: append to an existing journal on a shim restart, which no part of the plan says happens.
8. **`libc` is now a `cfg(unix)` dependency of `host`** (it was macOS-only), because `setsid`, `wait4` and `openpty` are not macOS-only. The macOS-specific readers still sit behind `cfg(target_os = "macos")`.

- Owner decisions (2026-11-15): the recommendations above are accepted as built, including the cap sizes (8 MiB line, 256 MiB per log); they are named constants the owner may change. Pipes match 0.2, where `out` records are read from the child's stdout and the plain logs stay for tailing.

### Not done, by design

- `serve_daemon` (`shim: daemon link`): the socket, the handshake, `Write`, `CloseStdin`, `Signal`, `AckExit`, the peer-uid check and the PTY attach-and-subscribe in one step: ts-0057-ck. The pieces it needs are here: `Shim::write_to_child` (returns the `seq` for `ShimMessage::Wrote`), `close_stdin`, `ring_snapshot`, `problems` (for `ShimMessage::Failed`), `ExitKeeper::pending_message`/`acknowledge`, and `Sinks::on_line` (to feed the live session). Until it lands, `run` leaves the child's stdin open and exits with `exit_code::RECORD_NOT_WRITTEN` when `exit.json` could not be written, because nobody can be served the in-memory record.
- Sending signals to the child's process group, with the `waitid(WNOWAIT)` group check, the stop order and the survivor report: ts-0059-xs. The shim does not signal anything yet.
- Wiring `collide shim` into the `collide` binary, the daemon starting shims, the run-directory layout and the `lost` rule on the daemon's side: ts-0056-ag (daemon) and the adopt-shims item. `run` takes `<run-dir>` with a `spec.json` in it; the daemon writes that file.
- A SIGSTOPped or SIGKILLed shim, and a hostile child that ignores HUP and TERM: those tests are about the daemon's view of a shim and the stop, so they come with ts-0057 and ts-0059.
