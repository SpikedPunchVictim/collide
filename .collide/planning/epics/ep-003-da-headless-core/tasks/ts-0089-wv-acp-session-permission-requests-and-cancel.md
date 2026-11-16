---
id: ts-0089-wv
kind: task
status: ready
title: "ACP session: permission requests and cancel"
owners: []
links:
  - { type: blocked-by, to: ts-0088-pk }
---

## Goal

The ACP session's permission and cancel paths, made as 0.3 says and as S7 measured. `ts-0087-mf` left them as the
double had them.

**Permission request.** The agent sends `session/request_permission` with its own request id, a `toolCall` and a list of
options. The session:

- gives `PermissionRequest` with the id the agent used (as text), the `toolCallId`, the tool's `kind`, and the options
  as the agent named them (id, name, kind). It reads the kind from the request's `toolCall`, and when that has none,
  from
  a `tool_call` it saw earlier with the same id. When it has neither, the kind is `other`. S7 measured that the request
  comes before the `tool_call` (O6) and that `rawInput` can be `{}` for bash (O5): `input` is `None` then;
- holds the request as pending, with the JSON-RPC id exactly as the agent wrote it (a number stays a number), so the
  answer carries the same id;
- answers on `answer(id, reply)`:
  - `Allow` picks the option of kind `allow_once`. **Never `allow_always`**, not even when it is the only allow option;
  - `Deny` picks `reject_once`. The message of a deny is dropped: ACP has no place for it (S7, G5);
  - `Cancel` writes the outcome `cancelled`;
  - an allow with no `allow_once` option writes `cancelled`, gives `PermissionUnanswerable` and nothing else, so the
    host can tell the person why (0.3, Permissions);
  - an id the session does not hold writes nothing;
- keeps the request in `pending_permissions` until it is answered, also after a replay: a request the agent asked
  before a restart and nobody answered is returned, oldest first, and a request the journal shows answered is not.

**Cancel.** `cancel` writes `session/cancel` for the session id, a notification with no id, and marks the turn as
cancelled by the host. It answers each pending request with `cancelled` first, as ACP requires. The turn ends when the
`session/prompt` response comes, and the session reports `TurnEnd { Cancelled }` whatever stop reason the agent gave: S7
measured that opencode answers `end_turn` (O1). A cancel before the session has an id, or after the turn ended, writes
nothing.

## Success criteria

- `Allow` writes the `allow_once` option's id; with `allow_once` and `allow_always` both present, the first is chosen
  and the second never is. `Deny` writes `reject_once`.
- A request with only `allow_always` and `reject_always`, answered with `Allow`, writes `cancelled` and gives
  `PermissionUnanswerable`; the same with `Deny` is also `cancelled`.
- A deny message appears nowhere in the bytes written.
- The answer's id is the agent's id, a number stays a number and a string stays a string.
- An id the session does not hold, and an id answered twice, write nothing.
- The request's `kind` comes from the request; when absent, from the earlier `tool_call`; else `other`. A request that
  comes before its `tool_call` is not an error, and a `tool_call` after it changes nothing already given.
- After a replay of a journal that holds one asked and one answered request, `pending_permissions` is the asked one
  alone, with its options.
- `cancel` writes one `session/cancel` with no `id`, after one `cancelled` outcome for each pending request. A second
  `cancel` writes nothing.
- A turn cancelled by the host ends `Cancelled` when the agent says `end_turn` (S7, O1), `cancelled`, or `refusal`.
  A turn the host did not cancel ends as the agent says.
- Under the real shim, a permission request answered `Allow` lets the child go on to the end of its turn, and a cancel
  mid-turn ends it `Cancelled`.
- No new crate is added.

## Tests

Write these first:

- the shared contract's permission and cancel scenarios (already run for the session by `ts-0087-mf`) stay green;
- `Allow` with each combination of options (`allow_once` alone; both allows; `allow_always` alone; no allow);
- `Deny` with `reject_once`, with `reject_always` only, and with a message that must not be written;
- the id kept as number and as string;
- an unknown id and a repeated id;
- the kind from the request, from the earlier `tool_call`, and defaulted; the request before its `tool_call`;
- `pending_permissions` live, after an answer, and after a replay with and without the answer in the journal;
- `cancel` with two pending requests, with none, twice, and before the session id;
- a cancelled turn that the agent ends `end_turn`, `cancelled` and `refusal`; the same stop reasons without a cancel;
- the fake child under the real shim: a recording that asks for permission and waits for the answer (`$id` in the
  `expect`), and one that waits for `session/cancel` and then answers the prompt `end_turn`.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 3d: ACP engine (permission request, `session/cancel`).
  - 0.3: Permissions (who answers; `allow_once` only; `PermissionUnanswerable`; a deny message is dropped); Normalized
    events; Two core traits (`pending_permissions`, `cancel`); The stream protocol.
  - 0.15: Dependency injection.
  - S7 record: P4, P7 and G5, G6, O1, O5, O6.
- **Decision for the owner, with a recommendation: `pending_permissions` for ACP.** The trait's doc says an ACP session
  "answers inside its own turn" and so holds none. That is wrong across a restart: the agent is a child of the shim, it
  outlives the daemon, and its request stays open. Recommend that the ACP session returns its pending requests, rebuilt
  from the journal like Claude's, and that the trait's doc is corrected to say so (a doc line in `core/src/adapter.rs`;
  no change to the signature). Alternative: leave the default and let a restart cancel the turn. Pro: no code. Con: a
  person's pending approval is lost on every daemon restart, which is what the Claude path was built to avoid.
- **Decision for the item, with a recommendation: a request with no usable option.** Recommend `cancelled` plus
  `PermissionUnanswerable`, as 0.3 says; the host then shows the person that the agent offered no one-time allow.
  Never pick `allow_always`: it changes the agent's own standing rules, outside Collide's reach.
- `PermissionRequest` carries no paths (`core/src/agent.rs`). ACP's `toolCall.locations` has them. Stage 6b's ask box
  does not need them. Leave them out, and say so in the session's doc.
- A request from the agent for a tool the host's `permission_host` allow-list already allows is not seen here: that is
  the daemon's job above the session (0.3, Permissions).
- The same-id rule: ACP request ids are numbers or strings. Keep the agent's value as a `serde_json::Value` inside the
  pending list and give the event the text form.
- Run `pnpm gen` if a wire type changes. None is expected.
- Area: `host` (`adapters/acp/session.rs`, `permissions.rs`), `core/src/adapter.rs` (a doc line), the test folder.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 3d item (revision 23 scope, split 2026-11-15). Items of Stage 3d run one at a time, in id order, because
  they share `host` and the generated files.
- Owner decisions (2026-11-15, at the split): the decisions for the owner above, taken as recommended under the
  owner's delegation; the owner may change them before the item is built. The ACP protocol is written by hand: no ACP
  crate is added.
