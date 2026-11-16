---
id: ts-0087-mf
kind: task
status: ready
title: "ACP session: handshake, a prompt turn and replay"
owners: []
links:
  - { type: blocked-by, to: ts-0086-tm }
---

## Goal

The ACP engine's `AgentSession`, in `host` (`host/src/adapters/acp/session.rs`): the second implementation of the
trait (0.3, Two core traits; Stage 3d). It starts from the scripted double in `core/src/testing/acp.rs`, which its own
doc calls the outline of this engine, and hardens it. Nothing ACP-shaped moves into `core`: the double stays there as
the test double of `core`'s own tests.

- `start` writes `initialize`: protocol version 1, and the client capabilities of 0.3 (Later adapters): **no** `fs`,
  `terminal` or `elicitation`, and the `session` flags `notices`, `compaction` and `configOptions`. S7's `driver.mjs`
  has the shape of the `session` flags.
- The `initialize` response makes it write `session/new` with the working folder and `mcpServers`. The servers come from
  the session's options and are `[]` by default. Stage 6b's ask box fills them with its endpoint; nothing here knows
  of it.
- The `session/new` response gives `SessionBound` with the agent's id, and makes the session write the first prompt
  (`LaunchRequest.prompt`, held as the session's own data, so a rebuilt session has it too).
- Request ids count from 0. A response is matched to its request by id. One that matches nothing changes nothing.
- `session/update`:
  - `agent_message_chunk` gives `Message`;
  - `tool_call` gives `ToolCall`, classified by `kind` and never by the title (S7, O12), with `input` `None` while the
    agent sends none or `{}` (S7, O5);
  - any other update (`tool_call_update`, `agent_thought_chunk`, `plan`, `available_commands_update`,
    `config_option_update`) gives `Other` named by its `sessionUpdate`.
- The `session/prompt` response ends the turn: `end_turn`, `max_tokens`, `max_turn_requests`, `refusal` and `cancelled`
  map to the `TurnEndReason` of the same meaning; any other word, or a JSON-RPC error, is `Error`. A `FinalReport`
  with source `Reconstructed` holds the message chunks after the last tool call.
- A request from the agent that the engine does not serve (`fs/*`, `terminal/*`, `elicitation/*`, any unknown method)
  is answered with the JSON-RPC error `-32601`, so the agent never waits on it, and is reported as `Other`.
- `send_turn`, `answer`, `cancel`, `finish` and `replay` are the double's, over the shared walk
  `collide_core::replay::replay_records`. Permission and cancel are hardened by `ts-0089-wv`; the model by
  `ts-0090-jd`; empty turns, lost children and usage by `ts-0091-pz`.

## Success criteria

- The session passes the whole shared contract (`collide_core::testing::contract`, the 19 scenarios) through
  `session_contract_tests!`, as Claude's session does in `host/tests/adapters/claude/contract.rs`.
- Over the fake ACP child under the real shim, one prompt turn gives, in order: `SessionBound` with the stored time of
  the `session/new` response, `Message`, `ToolCall` of kind `read`, `Message`, `FinalReport` holding only the words
  after
  the tool call, and `TurnEnd { EndTurn }`. `finish` says `Ended`.
- Replayed from the journal the real shim wrote, the session reaches the state of the live one, also for a journal cut
  at every byte (Stage 3d: the second `AgentSession` keeps the trait honest).
- `initialize` holds no `fs`, `terminal` or `elicitation`. `session/new` holds the working folder, and `mcpServers`
  as the options give them, `[]` when they give none.
- An agent request the engine does not serve gets `-32601` with its own id, and an `Other` with the method's name.
  Nothing of the request's params is kept in the event (0.3: a stream's content is untrusted and may be large; as
  `ts-0062-rj`, decision 1).
- An update of an unknown kind, and a stdout line that is not JSON, are each an `Other` with the kind or
  `unparsed_line` and no content. The journal holds the line whole.
- A JSON-RPC error in answer to `initialize` or `session/new` writes nothing more, is an `Other`, and makes `finish`
  say `Failed { cause: Api { status: None } }` (see the decision below).
- The `TurnEnd` of each stop reason in the list above, and `Error` for an unknown word and for an error response.
- No new crate is added.

## Tests

Write these first:

- the shared contract (a `ChildDialect` for the ACP session, as the Claude one: the handshake lines, a message, a
  permission request, a turn end);
- the fake ACP child under the real shim, with a live session fed from the journal (a `play` helper next to the child's,
  as `host/tests/adapters/claude/fake.rs` has): one prompt turn, its events in order and with their stored times;
- replay equal to live, a journal cut at every byte, a journal that is not this session's rejected with the record;
- `initialize` and `session/new` as written bytes, with and without servers;
- ids: the first requests are 0, 1 and 2; a response that matches no request changes nothing;
- each stop reason, an unknown one, an error response to the prompt;
- an unserved agent request, answered `-32601` and reported;
- an unknown update, a line that is not JSON;
- an error response to `initialize`, and to `session/new`;
- `ToolCall` classification by `kind` with a misleading title, and `input` `None` for `{}` (S7, O5, O12).

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 3d: ACP engine.
  - 0.3: Two core traits; Normalized events; A session starts as; Later adapters (an ACP adapter is one engine plus a
    profile; no `fs`, `terminal` or `elicitation`); The stream protocol (a phase's outcome is its prompt response).
  - 0.15: Dependency injection (the shared contract keeps the traits honest).
  - S7 record: P1, P2, P6, P8, P9 and O6, O12.
  - Stage 6b: The in-app agent (what it will later use: `mcpServers` in `session/new`, `ToolCall` kinds).
- **Decision for the item, with a recommendation: copy, do not share.** `core` may not hold the engine, and `host` may
  not use `core`'s `testing` module outside tests. Recommend copying the double's logic into `host` and hardening it,
  and keeping the double as is. The shared contract keeps the two in step. Update the double's module doc so it points
  at the engine in `host` and no longer calls itself the outline of a later stage.
- **Decision for the owner, with a recommendation: who writes the first prompt.** The double writes it as the
  handshake completes, from data given at construction; Claude's first turn comes through `send_turn`. Recommend
  keeping the double's way for ACP: it is replay-safe, because a rebuilt session has the prompt in its own data, where
  a turn queued by `send_turn` before the handshake ended would be lost by a restart in that window. The daemon then
  needs to know which of the two a session is. `ts-0092-tw` takes that up. Alternative: one rule for every adapter
  (`start`, then `send_turn` for the first turn, ACP holds it until ready), which needs a change to the contract and has
  the window above.
- **Decision for the item: the options type.** Recommend a small struct of the session's data (working folder, first
  prompt, `mcpServers`, and the model that `ts-0090-jd` adds), built from a `LaunchRequest` by the adapter of
  `ts-0088-pk`. Give each server a typed form for the `http` and `stdio` kinds ACP defines, not a raw `Value`, so a
  caller cannot send a shape the agent rejects. `sse` is left out until a use for it exists.
- **Decision for the item, with a recommendation: a failed handshake.** ACP has an `auth_required` error
  (`-32000`) for an agent whose login has lapsed. Recommend `Failed { cause: Api { status: None } }` here, with the
  error's text kept as evidence by `ts-0091-pz`. A new `PhaseFailure` variant for it is a larger change than this
  stage needs.
- `tool_call_update` stays an `Other`. Reading `rawOutput` for a `ToolResultSize` is a metrics item, and S7 measured
  that the update arrives late (the agent's own bookkeeping).
- The agent's `session/request_permission` may arrive before the `tool_call` it names (S7, O6). Hardened in
  `ts-0089-wv`; here the session must not fail on it.
- Run `pnpm gen` if any wire type changes. None is expected.
- Area: `host` (`adapters/acp/` new, `adapters/mod.rs`), `core/src/testing/acp.rs` (a doc line), the test folder of
  `ts-0086-tm`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 3d item (revision 23 scope, split 2026-11-15). Items of Stage 3d run one at a time, in id order, because
  they share `host` and the generated files.
- Owner decisions (2026-11-15, at the split): the decisions for the owner above, taken as recommended under the
  owner's delegation; the owner may change them before the item is built. The ACP protocol is written by hand: no ACP
  crate is added.
