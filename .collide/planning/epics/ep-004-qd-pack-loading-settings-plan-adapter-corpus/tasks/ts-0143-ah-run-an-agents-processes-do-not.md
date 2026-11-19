---
id: ts-0143-ah
kind: task
status: ready
title: "Run: an agent's processes do not carry the runtime mark, so a stop signals them"
owners: []
links: []
---

## Goal

`collide stop` signals no process of a run's agent. The daemon carries `COLLIDE_RUNTIME=1` (`RUNTIME_MARKER`), set by
`launch.rs` both on the CLI's start and in the launchd job. Its base environment is a login shell's capture, which
inherits the mark (`host/src/base_env.rs`), and `BaseEnv::for_child` strips only `ALWAYS_STRIPPED` and the adapter's
list. So the agent child carries the mark too, and `find_tree` in `host/src/stop.rs` skips it as a runtime process.
Found by the agent run of `ts-0121-xp`'s checklist (2026-11-19): "0 process(es) signalled"; the stand-in `claude` ended
only when its standard input closed. A real `claude` that ends on the interrupt hides this; a hung agent, or a tool it
started, is never signalled.

## Success criteria

- No child the daemon starts for a run (the agent, and what it starts) has `COLLIDE_RUNTIME` in its environment,
  whatever the captured base environment holds.
- `collide stop` of a run whose agent ignores the interrupt and the closing of its standard input signals the agent's
  tree, and the run ends `stopped` within the grace.
- The daemon and the shims still carry the mark, and a stop or a sweep still never signals them.

## Tests

- Write first and see red at runtime: `for_child` of a capture that holds `COLLIDE_RUNTIME=1` gives an environment
  without it.
- A daemon test: the daemon is started with the mark (as the CLI starts it), the fake agent ignores the interrupt and
  its standard input, and `stop` signals it (red today: 0 signalled).

## Notes

- Recommendation: add `RUNTIME_MARKER` to `ALWAYS_STRIPPED`, so one list says what no child inherits. The shim sets the
  mark on its own command (`phases.rs`), which this does not touch; check that before choosing.
- The builder lists, in Built, any other variable the daemon's own environment hands to an agent through the capture
  (the CLI's start sets some), and says whether each is right.
- Build it first, before `ts-0137-pg`: a stop that does not stop is a safety gap for every real run.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers.
