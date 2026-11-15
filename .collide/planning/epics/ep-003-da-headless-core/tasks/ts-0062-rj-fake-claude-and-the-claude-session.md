---
id: ts-0062-rj
kind: task
status: active
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

### Built

- `host/src/adapters/claude/session.rs`: `ClaudeSession` is filled. `start` reports `SessionBound` at once for `Assign` and `Resume` and writes nothing. `feed` and `read_line` are the stream parser: `assistant` gives a `Message` per non-empty text block and a `ToolCall` per `tool_use` (kind by tool name; thinking gives nothing); `control_request` `can_use_tool` gives a `PermissionRequest` (kept until answered; `decision_reason` becomes `reason`; no options); `result` gives a `FinalReport` from `result.result` (never for an error result) and a `TurnEnd` (reason from `stop_reason` and `subtype`, `cancel_requested`, `empty` when no output and zero usage); `system/init` gives nothing; every other line is an `Other` named `type` or `type/subtype` with no content. `send_turn`, `answer` and `cancel` write a `user` message, a `control_response` (allow with `updatedInput`, deny with `message`) and an `interrupt` `control_request` (`collide-interrupt-N`). Each write goes through `note_written`, so a live session and a replayed one reach the same state. `finish` maps the last turn to a `PhaseOutcome` (`Lost` with no exit record). Unit tests: 3 (the stub tests are gone).
- `host/tests/adapters/claude/fixtures/fake_claude.py`: fake Claude. It reads a recording, one JSON object per line, and runs its steps in order: `{"say": {...}}` writes a stream line; `{"say_text": "..."}` writes a line that is not JSON; `{"expect": {...}}` reads one stdin line and requires it to contain the object (a subset match; lists must be the same length), else exit 97 with the line on stderr; `{"stderr": "..."}`; `{"wait_eof": true}`; `{"exit": N}`. Blank lines and `#` lines are skipped. Exit 96 is an unknown step, 98 is stdin closed while an `expect` waits.
- `host/tests/adapters/claude/fixtures/recordings/` (17 files, built for the test with stand-in ids, none copied from a spike): `report`, `permission-request`, `multi-turn`, `host-deny`, `cancelled`, `max-tokens`, `max-turns`, `refusal`, `limit`, `compaction`, `blocked-prompt`, `hook-deny`, `hook-failure`, `hook-block`, `error-result`, `disallowed-tool`, `exit-code`.
- `host/tests/adapters/claude/fake.rs`: the helper. `FakeClaude` is a plain child (reaped on drop); `play` runs fake Claude under the real `Shim` with a live session fed from the journal's `on_line` hook and returns the events, the journal bytes and the exit.
- Tests, in the `claude_adapter` target: `fake_claude.rs` 11; `session.rs` 55; `replay.rs` 11 (replay equals live for four recordings, a cut at every byte of two journals, the pending request rebuilt after a cut, a foreign `in` record rejected); `contract.rs` 19 (all of core's contract suite through a `ClaudeDialect`). Workspace: 1900 tests pass, twice.
- `checks/tests/real_tree.rs`: the four tags move to `FILLED_TAGS` (33) from `HOST_ADAPTER_EXECUTOR_TAGS` (18). `core/src/testing/contract.rs`: the handshake scenario widened (decision 9). `core/src/adapter.rs`: two doc lines no longer say the launch line carries the first turn.

### Decisions for the owner

1. `Other.raw` is always `None` and `raw_type` is `type` or `type/subtype`. Alternative: keep the line, which would put hook output and prompts in the event log. Recommend none: the journal holds the line whole.
2. A `system/init` line gives no event. The id is bound in `start` and `init` comes on every turn. Alternative: bind again from `init`. Recommend not.
3. `AgentAssigned` binds nothing (launch refuses it already).
4. A `can_use_tool` input that is missing or null becomes `None`, and an allow then leaves out `updatedInput`. Alternative: send `{}`, which would replace the call's input with nothing. Recommend leaving it out.
5. A deny with no message gets a default message, because Claude requires one.
6. A `Cancel` reply is a deny whose message says it was cancelled, with no interrupt; `cancel()` interrupts. Alternative: interrupt on `Cancel`. Recommend as built: one call, one effect.
7. An error or unknown result becomes `TurnEnd{Error{status}}` and no `FinalReport`, a safe default until `ts-0063-rw` reads the limit, the blocked prompt and the budget.
8. An empty `result.result` gives no `FinalReport`.
9. The core contract's handshake scenario is widened for every dialect (coordinator review, 2026-11-15), and renamed `the_session_is_bound_exactly_once_by_start_or_by_a_handshake_line`. The rule (0.3: "at once for Claude, on the `session/new` response for ACP"): across `start` and the handshake lines there is exactly one `SessionBound`; from a fed line it carries that line's stored time; from `start` it carries the id start asked for (`ChildDialect::bound_at_start`, default `None`), and a later `system/init` adds no second one. The two doubles (fake adapter, ACP) pass unchanged through the macro. Claude runs all 19 scenarios through `session_contract_tests!`.
10. The first turn goes through `send_turn` (as `ts-0061-bg` recommended). Proposed text for 0.3: replace "the launch line carries the first turn" with "the first turn is a `user` message on stdin".
11. `control_response` acknowledgements, hook events and `rate_limit_event` stay `Other` until `ts-0063-rw`, `ts-0064-hw` and `ts-0066-zb`.

- Owner decisions (2026-11-15): the recommendations above are accepted as built. On review, the core session contract was widened rather than skipped for Claude: a session is bound exactly once, by `start` or by a handshake line, so the Claude session runs every contract scenario. The proposed 0.3 sentence on the first turn was already applied with `ts-0060-mm`. Follow-up: `ts-0063-rw` refines error and unknown results, which end the turn as `Error` with no report until then.

### Not done, by design

- Hook and refusal parsing, and usage: `ts-0063-rw` and `ts-0064-hw`.
- The limit pause and the blocked-prompt reason: a blocked prompt is read as `FinalReport` plus `EndTurn` for now.
- Contract tests against the real `claude` (`ts-0070-mk`).
- Version, smoke and canary checks (their stubs stay).
