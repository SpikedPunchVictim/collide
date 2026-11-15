---
id: ts-0062-rj
kind: task
status: ready
title: "Fake Claude and the Claude session: turns, permissions, replay"
owners: []
links:
  - { type: blocked-by, to: ts-0061-bg }
---

## Goal

Fake Claude, and the main path of the Claude `AgentSession` (`host/src/adapters/claude/session.rs`).

**Fake Claude** is a small executable that replays a recorded stream-json script. It writes the recorded lines, waits for the stdin messages the script expects (a turn, a permission answer, an interrupt), and exits with the recorded code. The recordings cover every case in Stage 3b's Fake Claude bullet:

- tool calls, a permission request and a final report;
- a limit message and a compaction;
- a prompt blocked by a hook, hook denies, failures and blocks, and a host deny;
- an error result, a call to a disallowed tool, and an exit code.

**The session:**

- `start`: `Assign` gives `SessionBound` at once, and `Resume` is supported.
- `feed` reads:
  - `system/init`;
  - assistant text and tool calls;
  - `can_use_tool` permission requests;
  - the `result`, as `TurnEnd` and as `FinalReport` from `result.result`;
  - any unknown event, as `Other`.
- `send_turn` writes a turn. `answer` writes a `control_response`: allow, or deny with the message. `cancel` writes a `control_request` `interrupt`.
- `finish`, and replay from the journal.

Replaces the stubs tagged `claude: session start`, `claude: stream parser`, `claude: turn input` and `claude: replay`. The next two items extend the parser (`ts-0063-rw`, `ts-0064-hw`).

## Success criteria

- Over a recorded session (the permission-request and multi-turn recordings), the Claude `AgentSession` replayed from the journal reaches the same state as the live one (Stage 3b criterion).
- `answer` maps `Allow` and `Deny { message }` to the `control_response` `behavior`, with the message (S1). Answering an id the session does not hold writes nothing.
- `TurnEnd` reasons: `EndTurn`, `MaxTokens`, `MaxTurns` and `Refusal` from the recording, and `Cancelled` whenever a cancel was sent, whatever the agent's own reason (0.3). `cancel_requested` and `empty` follow 0.3.
- An event type the parser does not know becomes `Other`, with its type name.
- `FinalReport` comes from `result.result`, with source `ResultField`.
- Each recording is a fixture file under `host/tests/adapters/claude/fixtures/`, either built for the test or redacted. A spike's `out/` recording is never copied raw: those files are gitignored and may hold paths, prompts and ids.
- No stub tagged `claude: session start`, `claude: stream parser`, `claude: turn input` or `claude: replay` remains. The four tags move to `FILLED_TAGS`.

## Tests

Write these first:

- fake Claude replaying one script under a plain child, with no daemon;
- the session over each recording's main path;
- `TurnEnd` reasons: end of turn, max tokens, max turns, refusal, and cancelled after `cancel`;
- a permission request whose input is unknown, which carries `input: None`;
- replay equal to live over the permission and multi-turn recordings, including a journal cut short.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 3b: Fake Claude; the Claude adapter is the first `AgentSession`; `feed` takes `wall_ns`.
  - 0.3: Two core traits; Normalized events; Permissions; A turn ends with; The stream protocol.
  - 0.15: engine tests use the normalized-event fake adapter (`core/src/testing/adapter.rs`), not fake Claude.
  - S1 record: the stdin messages and their shapes. S7 record: P1–P9.
- **Where fake Claude lives.** Under `host/tests/adapters/claude/fixtures/`. Recommend a python3 script, like Stage 3a's test children. The names check rejects any line outside the adapter paths that spells `CLAUDECODE`, `CLAUDE_` or `ANTHROPIC_` (`checks/src/names.rs`, `names_a_claude_env_var`). So every test that starts fake Claude lives under `host/tests/adapters/claude/`, or gets fake Claude's path from a helper there.
- The first turn goes on stdin, as S1 measured (`ts-0061-bg`'s decision).
- Area: `host` (`adapters/claude/session.rs`, and the adapter's test folder).
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot` does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 3b item (revision 18 scope, split 2026-11-15). Items of Stage 3b run one at a time, in id order, because they share `host` and the generated files.
