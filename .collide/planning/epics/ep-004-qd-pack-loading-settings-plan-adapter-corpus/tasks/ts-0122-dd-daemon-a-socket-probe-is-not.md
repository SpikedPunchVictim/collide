---
id: ts-0122-dd
kind: task
status: done
title: "Daemon: a socket probe is not logged as a refused connection"
owners: []
links: []
---

## Goal

The CLI checks that the daemon answers with a bare connect that it closes at once (`answers` in `host/src/launch.rs`).
The daemon's `serve_connection` (`host/src/daemon.rs`) then cannot read the peer's pid (`LOCAL_PEERPID`, `Socket is not
connected`, os error 57) and logs "a connection was refused: could not read the pid of the connecting peer". Nothing
that wanted in was refused, so the line is noise, and it reads like a security event. Seen in the owner's `ts-0083-uf`
check log.

## Success criteria

- A peer that closed before its pid could be read is not logged as refused: no line, or one plain line saying it closed
  before it was read.
- A real refusal (another uid, a process the tree does not admit) still logs as it does now.

## Tests

- Write first and see red at runtime: a connect-and-close to a test daemon leaves no `refused` line in its log.
- A refused peer still writes its `refused` line.

## Notes

- Recommended: fix it in the daemon, so any probe is quiet, not only ours. Alternative: make the probe a real request;
  it costs a round trip on every start.
- Found at `ts-0083-uf` (2026-11-18).

### Built

A daemon fix. No new crate or package, no wire type changed, no generated file.

- `host/src/daemon.rs`
  - `SocketError::PeerClosed` (new): a peer that closed before its pid was read. It is not a refusal.
  - `log_refusal(log, refused)` (new, private): what `serve_connection` calls when `auth::admit` fails. It writes
    nothing for `PeerClosed` and the same `a connection was refused: ...` line as before for every other error.
- `host/src/auth.rs`
  - `admit` maps a pid read that fails with `io::ErrorKind::NotConnected` (`ENOTCONN`, os error 57) to
    `SocketError::PeerClosed`. The kind is matched, not the message. Any other pid error stays `SocketError::Io`.
    The uid check still comes first, so a foreign uid is `ForeignUid` whatever its pid does.
- Tests, and which I saw red at runtime against the old behavior (the variant and `log_refusal` were in place, logging
  as before, so the failures are assertions and not compile errors):
  - `host/tests/daemon_socket.rs`: `a_probe_that_connects_and_closes_is_not_logged_as_refused` (red: the log held the
    `could not read the pid ... Socket is not connected (os error 57)` line, as in the owner's check).
  - `host/tests/auth.rs`: `a_peer_that_closed_before_its_pid_was_read_is_not_a_refusal` (red).
    `a_pid_that_cannot_be_read_for_another_reason_is_still_an_io_error_naming_it` and
    `a_foreign_uid_is_refused_even_when_its_pid_would_not_read` guard the other side and passed before and after.
  - `host/src/daemon.rs` (`tests`): `a_peer_that_closed_before_it_was_read_writes_no_line` (red).
    `a_foreign_uid_still_writes_its_refused_line` and
    `a_pid_that_could_not_be_read_for_another_reason_still_writes_its_refused_line` passed before and after.
- The daemon-level test cannot make a foreign uid without root, so the refused line is checked on `log_refusal`.
  Its absence check waits on a later client's `connected as` line: the daemon accepts in order, so that client comes
  after all 50 probes. It is an ordering argument, not a count of finished probe threads, which the daemon does not
  expose.

### Decisions for the owner

- **No line at all for a peer that closed before it was read.** The item allowed no line or one plain line. The
  daemon's handshake already says nothing for a probe that closes after its pid was read (`Line::Closed`), so this makes
  both timings the same. Alternative: one plain line, `a peer closed before it was read`. It would show a probe on every
  CLI start. Recommend as built.
- **Only the pid read is quiet.** A failure of the uid read (`getpeereid`) still logs as a refusal, since without the
  uid the daemon cannot tell a foreign peer from a probe. Alternative: also quiet on `NotConnected` there. Recommend as
  built; the log from `ts-0083-uf` shows the pid read as the one that fails.
- **A new `SocketError` variant and not a check in the daemon on `Io`'s source kind.** A kind check on `Io` would also
  have silenced the uid read. Alternative: that check in `log_refusal`. Recommend as built.

- Owner decisions (2026-11-18, at the build): all taken as recommended, on the owner's delegation: no line for a peer
  that closed before its pid was read; a failed uid read still logs as a refusal.

### Not done, by design

- Making the CLI probe a real request: the item's alternative, not taken, since it costs a round trip on every start.
- The `launch.rs` doc of `answers` is unchanged: it already says the daemon takes a probe that closes.
