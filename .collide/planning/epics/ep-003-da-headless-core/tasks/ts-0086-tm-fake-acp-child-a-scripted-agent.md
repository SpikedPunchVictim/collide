---
id: ts-0086-tm
kind: task
status: active
title: "Fake ACP child: a scripted agent and its harness"
owners: []
links: []
---

## Goal

A fake ACP child, and the harness that starts it, for Stage 3d's engine tests. It is the ACP twin of fake Claude
(`host/tests/adapters/claude/fixtures/fake_claude.py`, `ts-0062-rj`). No session code is written here.

**The child** is a python3 script, `host/tests/adapters/acp/fixtures/fake_acp.py`. It replays a recording: a JSON Lines
file whose lines are steps, run in order. A blank line and a line that starts with `#` are skipped. The steps:

- `{"say": {...}}` writes the object to stdout as one line of JSON-RPC;
- `{"say_text": "..."}` writes a line that is not JSON;
- `{"stderr": "..."}` writes a line to stderr;
- `{"expect": {...}, "as": "name"}` reads one line of stdin and requires it to hold the object (a subset match, as in
  fake Claude: a list must have the length it names). When the line is a request and `as` is given, the child
  remembers the request's `id` under that name;
- `{"$id": "name"}` anywhere inside a later `say` is replaced by the remembered id, so a recording can answer a request
  whatever id the host gave it (an ACP client numbers its own requests);
- `{"wait_eof": true}` reads stdin until it closes; `{"sleep": SECONDS}`; `{"exit": N}`.

Exit 96 is a step it does not know, 97 a line that did not match (with the line and the wanted object on stderr), 98
stdin closed while an `expect` waited, and 99 an `$id` that was never remembered. After the last step the child waits
for stdin to close and exits 0, as an ACP agent does when its client goes away while it is idle (measured S7).

**The harness** is `FakeAcp`, a plain child of the test with stdin and stdout piped, reaped on drop, like `FakeClaude`
in `host/tests/adapters/claude/fake.rs`: start on a recording in a temp root, write a line, read a line with a
patience limit, close stdin, wait for the exit and the stderr text. A new test target, `acp_adapter`, holds it
(`host/tests/adapters/acp/main.rs`), as `claude_adapter` holds the Claude tests.

**Recordings** here are only what the child's own tests need: the handshake in its three forms (an agent with
`configOptions`, one with only the draft `models`, one with neither), one prompt turn and a permission request. Each
later item writes the recordings it needs, in the same folder, before its code. Where an item needs a step the child
lacks, it adds the step and a test for it in the same commit.

## Success criteria

- One recording runs line for line under a plain child, with ids the host chose: the child's response to `initialize`
  carries the id of the request it read.
- `expect` is a subset match, and a list in it must have the length it names.
- A line that does not match ends the child with 97 and says what it wanted and what it got. Stdin closing while an
  `expect` waits is 98. An unknown step is 96. An `$id` never remembered is 99.
- After its last step the child waits for stdin to close and exits 0.
- Every recording in the folder is JSON on each line, says something, and holds only steps the child knows. Each is
  built for the test with stand-in ids and paths. None is copied from a spike's `out/` folder, which is gitignored and
  may hold paths, prompts and ids.
- The three handshake recordings use the shapes the ACP schema gives (`agentCapabilities`, `mcpCapabilities`,
  `configOptions` with a `category: "model"` option, the draft `models` with `currentModelId` and `availableModels`),
  and `session/update` and `session/request_permission` as S7's record measured them.
- `host/Cargo.toml` names the `acp_adapter` target. No new crate is added.

## Tests

Write these first:

- a recording replayed under a plain child, with the host's own request ids, no daemon;
- nothing is written before the line the recording waits for arrives;
- `expect` as a subset, and a list of the wrong length;
- a mismatch ends it with 97 and the line on stderr; stdin closing first with 98; an unknown step with 96; an `$id`
  never remembered with 99;
- a `say_text` step writes a line that is not JSON;
- an `exit` step ends it with the recorded code; a `stderr` step is written first;
- a comment and a blank line are skipped;
- every recording in the folder parses and holds only known steps.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 3d: ACP engine (the fake ACP child and its scenarios).
  - 0.3: Two core traits (an ACP session holds its handshake, request ids and pending requests); Later adapters
    (an ACP adapter is one engine plus a profile per agent).
  - 0.15: Builders are Sonnet agents; Dependency injection.
  - S7 record (`.collide/planning/spikes/sp-0006-kq-agentadapter-acp/record.md`): the mapping tables, the live-run
    observations O1 to O12, and `driver.mjs` for the shapes the client sent and read.
- **Decision for the item, with a recommendation: where the child lives.** Recommend
  `host/tests/adapters/acp/fixtures/`, beside a test folder of its own, like fake Claude. The Claude names check does
  not apply, but one folder per adapter keeps each adapter's fakes and recordings together.
- **Decision for the item: how a recording names an id.** Recommend `as` on an `expect` and `{"$id": "name"}` in a
  `say`, as above. The agent's own ids (a permission request's `id`) are written in the recording as numbers.
- The shapes of `session/set_config_option` and of the flattened value in `configOptions` are in the ACP schema (S7
  read `schema-v1.31.0`; the crate `agent-client-protocol-schema` 1.18.0 holds the same types). Read the schema before
  pinning a recording to a shape, and say in the recording's first comment which part of it the line follows.
- Core already has a scripted ACP double with helper functions that write the agent's side
  (`core/src/testing/acp.rs`: `initialize_response`, `new_session_response`, `message_chunk`, `tool_call`,
  `permission_request`, `prompt_response`). The recordings may follow their shapes. Tests in `host` may call those
  functions, as `collide-core` is a dev-dependency with the `testing` feature.
- Use `/usr/bin/python3`, as the other real-process tests do (`host/tests/common/mod.rs` `PYTHON`).
- Area: `host/tests/adapters/acp/` (new), `host/Cargo.toml` (one test target).
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 3d item (revision 23 scope, split 2026-11-15). Items of Stage 3d run one at a time, in id order, because
  they share `host` and the generated files.
