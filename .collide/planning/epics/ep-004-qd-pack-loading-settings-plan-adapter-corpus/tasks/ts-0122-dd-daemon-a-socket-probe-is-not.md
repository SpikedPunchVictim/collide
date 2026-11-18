---
id: ts-0122-dd
kind: task
status: ready
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
