---
id: ts-0041-xf
kind: task
status: active
title: "host outlines: adapter, executors, supervisor, CLI"
owners: []
links:
  - { type: blocked-by, to: ts-0019-vm }
---

## Goal

Outlines in `host` as signatures with doc comments and tagged stubs: the Claude adapter (an `AgentAdapter` and its `AgentSession`, the launch line module, the version range, smoke check and settings canary signatures, the list of variables it strips), the hook executor, the provider supervisor (restart with backoff), the sampler, and the CLI command tree (`collide run`, `collide stop`, `collide tasks`, `collide task`, `collide schema`, `collide report`, `collide config`, `collide pack`). `collide schema`, `collide tasks` and `collide task` are real in their own items and appear here as the commands they are. No ACP engine and no per-agent profile: they are Stage 3d and are not outlined here.

## Success criteria

- Every stub carries a tag that belongs to a stage.
- `collide --help` lists the command tree above, and a stubbed command exits with a message naming its tag.
- The Claude adapter's stripped variables (`CLAUDECODE` and the `CLAUDE_*` family) appear only inside the adapter module, and the no-names check of `ts-0013-nz` passes.
- `cargo build` and `cargo clippy` pass.

## Tests

Write first: the stub-count check over these tags; a `--help` output test; a stubbed command's exit message.

## Notes

- IMPLEMENTATION_PLAN.md, 0.3 (the Claude adapter, what the adapter declares), 0.4 (providers and the supervisor), 0.5 (hook execution), 0.13 (sampling), 0.15 (Tagged stubs), Stage 2: Skeleton (`host` outlines: the Claude adapter, the hook executor, the provider supervisor, the sampler and the CLI commands).
- Area: `host`.
- Done when the criteria hold.
