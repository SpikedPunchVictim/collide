---
id: ts-0056-ag
kind: task
status: active
title: "Daemon: socket server, handshake and on-demand start"
owners: []
links:
  - { type: blocked-by, to: ts-0052-xt }
  - { type: blocked-by, to: ts-0053-gn }
  - { type: blocked-by, to: ts-0055-cr }
---

## Goal

The daemon (`host/src/daemon.rs`, `host/src/launch.rs`, `host/src/base_env.rs`): its entry, the socket server speaking the skeleton's JSON-RPC with the version handshake and the per-client token, the CLI token file at `0600`, on-demand start under the lock file with the `setsid` start, and the base environment captured at start and again when the app starts. Replaces the stubs tagged `daemon: entry`, `daemon: socket server`, `daemon: lock file start`, `daemon: setsid start` and `daemon: base environment`.

## Success criteria

- A daemon started from a terminal survives closing that terminal (Stage 3a criterion).
- Two starts at once give one daemon: the flock on the lock file serializes them, and a start waits up to 15 s (0.2, Starting the daemon).
- A client with no token or a wrong token is refused. A client with a different protocol version is refused at the handshake, with both versions in the message.
- The socket directory is `0700` and checked at start, and the token file is `0600`, made at the first start. A socket path over 103 bytes fails fast, naming the path and the limit (0.2, The socket).
- The base environment comes from `$SHELL -l -i -c` printing `env -0` between nonce markers, with stdin from `/dev/null`, under a timeout and capped at 1 MiB, in a session killed after reading. On failure it falls back to the minimal environment and logs why. `LANG` is set when missing. A new capture is used for new phases only (0.2, The base environment).
- A store that cannot be read is moved aside and reported at daemon start, using `ts-0053-gn`; the daemon keeps running.
- The daemon raises its soft open-file limit at start.
- The daemon's own log, written through a writer that fails with `ENOSPC` on demand, does not crash it: logging never panics (Stage 3a criterion; 0.2, Durability).
- No stub tagged `daemon:` remains except `daemon: launchd job` (`ts-0058-ad`) and `daemon: adopt shims` (`ts-0057-ck`).

## Tests

Write first: start, then a second start that finds the running daemon; two starts at once; token and handshake refusals; the long socket path; base-environment capture with a fake shell (nonce markers, a hung shell under the timeout, output over the cap); terminal close, in a test that starts the daemon from a child session and ends that session.

## Notes

- IMPLEMENTATION_PLAN.md, Stage 3a (The daemon and the shim; Socket security; SQLite store, per-client socket token, version handshake); 0.2 (Starting the daemon; The base environment; The socket; Durability).
- Area: `host`. Uses `ts-0052-xt` for the actor and `ts-0053-gn` for the store.
- Stage 3a item (revision 13 scope, split 2026-11-15). Items of Stage 3a run one at a time, in id order, because they share `host` and the generated files.
