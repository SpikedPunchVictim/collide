---
id: ts-0052-xt
kind: task
status: done
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

### Built

- `host/src/sysproc.rs` (new, registered in `host/src/lib.rs`): every call into the system that `auth` makes, and the only `unsafe` of the connection rules (each block small, each with a `// SAFETY:` comment, as in `clock.rs`). On macOS: `getpeereid` (peer uid), `getsockopt(SOL_LOCAL, LOCAL_PEERPID)` (peer pid), `geteuid` (the daemon's uid), `proc_pidinfo(PROC_PIDTBSDINFO)` (parent), `sysctl(KERN_PROCARGS2)` (environment, read again with more room when the buffer was too small) and `proc_pidpath` (executable). All of them are in `libc` 0.2.191, so no `extern "C"` is written by hand. `env_var_in` is the pure parser of the `KERN_PROCARGS2` buffer: it searches the environment only (not an argument that looks like `NAME=value`, not the strings the kernel adds after the environment), needs the whole name, takes the first of two, reads an empty value as a value and a cut-short or negative-count buffer as `None`. Every process reader answers `None` for a process it cannot look up, and a pid over `i32::MAX` is `None`, never a negative pid. Pid 0 is not a parent. Off macOS nothing is read: the peer's uid and pid are an `Unsupported` error, `daemon_uid` is `NO_UID` (`u32::MAX`, which no peer has) and every process lookup is `None`, so a connection is refused and never guessed at.
- `host/src/auth.rs`: the six stubs are filled and no `auth:` stub remains (`pnpm check:stubs` lists both tags at 0; the entries stay in `stub-tags.json`). `UnixPeer::uid`/`pid` and `daemon_uid` call `sysproc`. `SystemProcessTree` reads the parent, `COLLIDE_RUN` and the executable through `sysproc`. The four `should_panic` stub tests are removed; the module doc's tag table became a short "The real readers" section. The types, the traits, `classify_peer`, `admit`, `admit_shim`, `judge`, the directory checks and the 27 tests of `host/tests/auth.rs` over fakes are unchanged, with one signature change (Decisions, first).
- `checks/tests/real_tree.rs`: `auth: peer credentials` and `auth: process tree` move from `HOST_OUTLINE_TAGS` (now 21) to `FILLED_TAGS` (now 5) with item `ts-0052-xt`.
- Tests: `host/tests/real_peer.rs` (new, 23) and 10 inline tests in `host/src/sysproc.rs`. Real socket and real children, each test in its own short folder `/tmp/clp-<pid>-<n>` (the socket path limit is 103 bytes and `target/tmp` is longer), which it removes by file name and then `remove_dir`, never recursively; every child is killed and reaped by a guard. The walk is cut at the test process so that what is above it (cargo, a shell, a run of Collide with `COLLIDE_RUN` set) cannot change an answer, and each child starts with the marker removed unless the test sets it.
  - Credentials (2): the peer's pid is the connecting python3 child's and its uid is this user's; the daemon's uid is the owner of a file this process makes.
  - Who is asking (7): a python3 child with `COLLIDE_RUN` is `run agent` with its run id; a person-only `accept` from it is `Refuse` with `NOT_PERMITTED` (through `Admitted::initialize` and `Session::judge`); a python3 child with no marker is `cli`; a CLI's `reject` becomes a Queue confirmation; a python3 child under a `/bin/bash` that stands in for the app is `app`, and `cli` when the daemon knows no app or another executable.
  - A platform binary in the tree (3): macOS hides the environment of a `/bin/bash` that has the marker (the premise, so a macOS that stops hiding it is noticed); a marker on a python3 above a `/bin/bash` above the client, with none on the shell or the client, is found through the parents; a run marker above the app's stand-in wins over the app.
  - The readers on their own (6): the parent of this process, of a child started here, none for pid 1, nothing for pids that do not exist, this process is the app when told its own executable, and an app path given as a symlink is the same executable.
  - Foreign uid and the directory (5): a peer of uid + 1 over the real tree is refused with `ForeignUid` before its pid is read, and so is a shim's peer; the socket directory made `0700` passes against the real uid; `0750`, another owner and a symlink in its place are refused.
- Run three times in a row, all 23 pass each time, and no `/tmp/clp-*` folder and no child process is left.

### Decisions for the owner

- **`SystemProcessTree` takes the app's executable: `SystemProcessTree::new(Option<PathBuf>)`, a signature change from the unit struct of `ts-0033-zj`.** 0.2 says "the app's process" and does not say how the daemon knows it, and the daemon needs something to compare with. A process is the app when the system says it runs that file (`proc_pidpath`, links resolved on both sides); with `None` nothing is the app, so a person-only operation waits for a Queue confirmation (the safe side). Nothing else used the unit struct. The alternatives: a marker in the app's environment (like `COLLIDE_RUN`, which any child can copy, and which the app's own terminals would carry into the CLI), or the app registering its pid when it connects (needs a second channel, and a pid is reused).
- **The stand-in for the app is `/bin/bash`, not a copy of a binary.** A stand-in only has to be a process with a known executable that is a parent of the client; `/bin/bash` needs no copy (a copied platform binary may not run on Apple silicon). It is `/bin/bash` and not `/bin/sh` because the system reports `/bin/sh` as `/bin/sh` for a moment and as `/bin/bash` after, which made the first version of the test flaky by construction. The alternative is a small compiled stand-in built by the test.
- **The run marker is read from the process's environment as it was at `exec`.** `KERN_PROCARGS2` shows the starting environment, not a later `setenv`. That is what 0.2 means (the marker is inherited by everything a run starts), and a process cannot take the marker off its parents' environment. The alternative, reading a live environment, is not offered by macOS.
- **A process whose parent cannot be read ends the walk, and the answer is what was found so far (the CLI when nothing).** This is 0.2's rule as `ts-0033-zj` built it, and the real readers add one more way to reach it: a process of another user or a hardened one that `proc_pidinfo` refuses. The alternative is to refuse the connection, which would lock a person out of their own CLI under an unreadable terminal.
- **The peer's pid is the one at connect time (`LOCAL_PEERPID`), read once per connection by the daemon.** A pid reused between the read and the walk (the peer exits and another process takes the number) can misclassify one connection; the same-user rule (0.1) makes this a guardrail's limit. The alternative, pinning the process by start time (`pbi_start_tvsec`) along the walk, is what Stage 3a's stop does for shims (`ts-0059-xs`) and could be added here when the daemon holds a connection's tree.
- **The daemon's uid is the effective uid (`geteuid`), as `getpeereid` reports the peer's effective uid.** The two are compared like for like. The alternative is the real uid (`getuid`), which differs only for a setuid binary.
- **Off macOS the readers refuse, not guess.** The `libc` dependency is macOS-only (as in `clock.rs`), so other systems compile with a stub that fails closed (`NO_UID`, `Unsupported`). The alternative is `compile_error!` off macOS, or `SO_PEERCRED` for Linux (its own item, when a second platform is a target).
- **A foreign uid is tested through the fake `PeerCredentials` over the real tree.** A connection from another uid cannot be made without root, so no real test of it is possible here; the refusal, its message and the pid never being read are tested with a peer of uid + 1 and `UnixPeer` is tested for the same-uid case. The directory checks run against the real uid, and a different "daemon uid" stands for another owner.
- Owner decisions (2026-11-15): the recommendations above are accepted as built.

### Not done, by design

- The socket server that calls `admit` on each accepted connection, the version handshake and the token check on the wire: `ts-0056-ag` (tag `daemon: socket server`, Stage 3a). That item also decides where the app's executable path comes from when it builds `SystemProcessTree::new`.
- The app telling the daemon where it is, and the app's own connection: Stage 3c (tag `shell: daemon connection`).
- A Queue item on the wire for a CLI's person-only operation (`QueueConfirmation` is still a host type, see `ts-0033-zj` decision 5): the method bodies of Stage 3a (`ts-0056-ag`).
- Pinning a walked process by its start time against pid reuse: not planned; it comes with the stop's tree walk (`ts-0059-xs`) if the daemon needs it for connections.
- The Linux and Windows readers: not before a second platform is a target.
