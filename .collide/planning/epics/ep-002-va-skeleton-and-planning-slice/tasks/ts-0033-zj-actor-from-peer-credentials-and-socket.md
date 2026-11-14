---
id: ts-0033-zj
kind: task
status: ready
title: "Actor from peer credentials and socket rules"
owners: []
links:
  - { type: blocked-by, to: ts-0032-pp }
---

## Goal

The daemon's connection rules as pure, trait-based code in `host` (an `auth` module), so tests use fakes: the actor read from a connection's peer (the peer pid and the process-tree walk behind traits): a process with `COLLIDE_RUN` in its tree is `run agent`, one whose tree holds the app's process is `app`, anything else is `cli`; the person-only operations (accept, defer, reject and resolve of proposals and questions) allowed only for `app`, turned into a Queue confirmation for `cli`, and refused for `run agent`; the socket directory check (owned by the daemon's uid and mode `0700`, else the daemon refuses to bind and the message names the directory), the peer-uid check, the 103-byte path limit (a longer path fails fast naming the path and the limit); the CLI token file (mode `0600`, in that directory, never an environment variable); and a shim's socket under the same rules without a token. The real `LOCAL_PEERPID`, `getpeereid` and socket binding are Stage 3a.

## Success criteria

- A fake peer whose tree holds `COLLIDE_RUN` is `run agent`, one whose tree holds the app is `app`, and any other is `cli`.
- A person-only operation from a `run agent` is refused, from `cli` becomes a Queue confirmation, and from `app` is allowed.
- A directory that is not owned by the daemon's uid, or is open to others (not `0700`), is refused, and the message names the directory.
- A connection from another uid is refused before any token check.
- A socket path over 103 bytes fails with a message naming the path and the limit.
- The token file is created with mode `0600`, and no function puts the token into an environment variable.

## Tests

Write first: the actor classification table over a fake process tree; the person-only table per actor; directory modes `0700`, `0750` and a foreign owner; the uid check; the long path; the token file mode.

## Notes

- IMPLEMENTATION_PLAN.md, 0.2 (The socket; Who is asking), 0.10 (person-only ops; the actor is read from the connection), 0.1 (the same-user rule: a guardrail), Stage 2: Skeleton, Contracts and Success Criteria (the actor lines).
- Area: `host` (daemon `auth` module).
- Done when the criteria hold. The stage-3a stubs for the real peer lookup carry tags.
