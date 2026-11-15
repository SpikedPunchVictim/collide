---
id: ts-0052-xt
kind: task
status: ready
title: "Peer credentials and the process tree"
owners: []
links: []
---

## Goal

The real peer-credential and process-tree readers behind `host/src/auth.rs`, so the actor table that `ts-0033-zj` tested over a fake tree runs on real processes. The peer's uid and pid come from the connection (`LOCAL_PEERPID` and the peer's uid), the daemon's own uid from the process, and the tree's parent, run marker and is-the-app answers from libproc. Replaces the stubs tagged `auth: peer credentials` and `auth: process tree`.

## Success criteria

- A connection from a python3 child of a run (a process with `COLLIDE_RUN` in its tree) is identified as `run agent`, and a person-only operation from it is refused (Stage 3a criterion).
- A connection from a plain process with no marker in its tree is `cli`. One whose tree holds the app's process is `app`, tested with a stand-in for the app's process.
- A process whose environment macOS hides (a platform binary such as `/bin/sh`) is classified through its parents, as 0.2 says.
- A connection from another uid is refused. The socket-directory checks of `ts-0033-zj` (mode `0700`, owner, no symlink) still pass against the real readers.
- No stub tagged `auth:` remains.

## Tests

Write first: real-socket tests with python3 children (marked and unmarked), a `/bin/sh` parent in the tree, and the existing fake-tree table kept as it is.

## Notes

- IMPLEMENTATION_PLAN.md, Stage 3a (Socket security); 0.2 (Who is asking; The socket); 0.10 (person-only operations); 0.1 (the same-user rule: a guardrail).
- Area: `host` (`auth.rs`). The daemon's socket server that calls this is `ts-0056-ag`.
- Stage 3a item (revision 13 scope, split 2026-11-15). Items of Stage 3a run one at a time, in id order, because they share `host` and the generated files.
