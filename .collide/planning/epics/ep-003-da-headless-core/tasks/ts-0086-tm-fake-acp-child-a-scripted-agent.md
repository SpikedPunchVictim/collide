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

### Built

A fake ACP child, its harness and five recordings. No session code, no wire type and no crate: `pnpm gen` had nothing to
regenerate. The names check needed no change: nothing here names a Claude environment variable.

- `host/tests/adapters/acp/fixtures/fake_acp.py` (new): the child, a python3 script run as `fake_acp.py <recording>`.
  Steps `say`, `say_text`, `stderr`, `expect` (with `as`), `wait_eof`, `sleep` and `exit`; `{"$id": name}` in a `say` or
  an `expect`; exits 96, 97, 98 and 99; after the last step it waits for stdin to close and exits 0. The step format is
  in the file's docstring. `expect` is a subset match, as in fake Claude (`matches`), and a list must have the length
  it names. `resolve` replaces an `$id`; `expect` remembers the `id` of a line with a `method` and an `id`.
- `host/tests/adapters/acp/fake.rs` (new): `FakeAcp`, a plain child with piped stdin and stdout, an environment of its
  own (`env_clear()`, `PATH`, `HOME` and `COLLIDE_DAEMON_ROOT` in a `TempRoot`), reaped on drop. `start`, `send`,
  `send_bytes`, `next_line` and `next_value` (a patience limit of 20 seconds), `quiet_for`, `is_running`,
  `close_stdin`, `finish` (the exit code and the stderr text). Helpers `recording`, `steps`, `said` and
  `value_of_line`.
- `host/tests/adapters/acp/main.rs` (new) and `host/Cargo.toml`: the test target `acp_adapter`.
- `host/tests/adapters/acp/fixtures/recordings/` (new), each built for the tests with stand-in ids, models and paths,
  and a first comment that names the part of the schema it follows:
  - `handshake-config-options`: `configOptions` with a `mode` option and a `model` option (`category: "model"`).
  - `handshake-models`: the draft `models` (`currentModelId`, `availableModels`).
  - `handshake-bare`: neither.
  - `prompt-turn`: the handshake (with capabilities), one prompt, words, a `tool_call` of kind `read`, words and the
    response with `usage`.
  - `permission-request`: the handshake, one prompt, a `session/request_permission` with the agent's own id 7, the
    expected `selected` answer, then the turn's end. It follows S7: `rawInput: {}` (O5), the request before its
    `tool_call` (O6), and the options `once`, `always` and `reject`.
- None is copied from a spike's `out/` folder. The shapes come from the schema (`schema.json` and
  `schema.unstable.json` under the spike's gitignored `work/spec`), the S7 record and `driver.mjs`.

**Tests**, all in `host/tests/adapters/acp/fake_acp.rs`, none touching the real home, launchd or a daemon.

- Seen red before the child and the recordings existed: all 26 (they compiled and failed on the missing script and
  files). After the first green run, two of them,
  `the_response_to_initialize_carries_the_id_of_the_request_it_read` and
  `nothing_is_written_before_the_line_the_recording_waits_for_arrives`, still failed with 98: the test ended its stdin
  before the handshake's `session/new` that the recording also waits for. The tests were fixed, not the child.
- Names: `a_recording_is_replayed_line_for_line_with_the_ids_the_host_chose`,
  `the_response_to_initialize_carries_the_id_of_the_request_it_read` (ids 0, 41 and a string),
  `nothing_is_written_before_the_line_the_recording_waits_for_arrives`,
  `an_expectation_is_a_subset_so_a_line_may_hold_more`, `a_list_in_an_expectation_must_have_the_length_it_names`
  (shorter, longer and equal), `a_line_that_does_not_match_ends_it_with_97_and_says_what_it_wanted_and_what_it_got`,
  `a_line_that_is_not_json_where_one_is_expected_ends_it_with_97_and_is_quoted`,
  `stdin_closing_while_an_expectation_waits_ends_it_with_98`, `a_step_it_does_not_know_ends_it_with_96_and_names_it`,
  `a_recording_line_that_is_not_json_ends_it_with_96`,
  `an_id_never_remembered_ends_it_with_99_in_a_say_and_in_an_expectation`, `a_notification_has_no_id_to_remember`,
  `each_name_keeps_the_id_it_was_given`, `a_remembered_id_can_be_matched_in_a_later_expectation`,
  `a_say_text_step_writes_a_line_that_is_not_json`,
  `an_exit_step_ends_it_with_the_recorded_code_and_a_stderr_step_is_written_first`,
  `a_comment_and_a_blank_line_are_skipped`, `after_its_last_step_it_waits_for_stdin_to_close_and_then_exits_0`,
  `a_wait_eof_step_holds_the_steps_after_it_until_stdin_closes`, `a_sleep_step_holds_the_steps_after_it_for_that_long`,
  `every_recording_is_json_on_each_line_says_something_and_holds_only_steps_the_child_knows` (also: each `$id` is named
  by an earlier `as`; five recordings),
  `the_handshake_with_config_options_offers_a_model_option_in_the_shape_the_schema_gives`,
  `the_handshake_with_only_models_has_the_draft_shape`, `the_handshake_with_neither_names_no_model`,
  `the_turn_and_the_permission_request_follow_the_shapes_of_the_double_in_core` (equal to `message_chunk`,
  `tool_call`, `new_session_response`, `prompt_response`, `permission_request` and `standard_options` of
  `collide_core::testing::acp`), `the_permission_recording_waits_for_the_answer_with_the_id_the_agent_used`.
- Checks: `cargo build --workspace`, `cargo test --workspace` (2613 passed), `cargo clippy --workspace --all-targets
  -- -D warnings`, `cargo fmt --all --check`, `pnpm test`, `pnpm typecheck`, `pnpm --filter @collide/app build`,
  `pnpm build:packs` and `pnpm check` all pass. The `acp_adapter` target ran four times in a row with no failure.

### Decisions for the owner

1. **`$id` also works inside an `expect`.** The item names it for a `say`. `ts-0089-wv` says "`$id` in the `expect`",
   so the child resolves it in both, and an unknown name is 99 before the child reads a line. Recommend keeping: it is
   a few lines, and it lets a recording check that the host echoed an id the child remembered. Alternative: `say` only,
   and `ts-0089-wv` adds it.
2. **`as` on a line that is not a request remembers nothing.** A `session/cancel` notification has no `id`, so a later
   `$id` of its name is 99, not a silent `null`. Recommend keeping.
3. **A recording line that is not JSON is exit 96.** The item says nothing; python's own traceback would be exit 1 and
   look like a crash. Recommend keeping.
4. **The harness is a copy of `FakeClaude`'s plain child, not shared.** `FakeClaude` sits in the `claude_adapter` target
   and cannot be reached from `acp_adapter`. The alternative is a plain-child helper in `host/tests/common/mod.rs` that
   both use, which changes the Claude tests. Recommend the copy now, and the shared helper when a third fake exists.
5. **The recordings pin no working folder, no `mcpServers` and no prompt text.** They expect the method (and the session
   id on `session/prompt`). The session's tests of `ts-0087-mf` check the bytes it writes. Recommend keeping, so the
   recordings serve every later item whatever options it builds the session from.
6. **The draft `models` shape is from S7, not from a schema file.** Neither `schema.json` nor `schema.unstable.json`
   of `schema-v1.31.0` has a model-state type. `handshake-models` follows `currentModelId`, `availableModels`, `modelId`
   and `name` as the spike's `driver.mjs` read them from the live agent. The recording says so in its first comment.
   `configOptions` and the set-option request follow the schema.
7. **The permission recording answers with the one-time allow only.** `ts-0089-wv` writes its own recordings for deny,
   cancel and the unanswerable case, as the item says each later item does.

- Owner decisions (2026-11-15, at the build): decisions 1 to 7 taken under the owner's delegation, as recommended.
  Decision 4 (a plain child of its own rather than a helper shared with fake Claude's) is kept: the shared part is
  process plumbing, and moving it would change the Claude tests for no gain in this stage. Two tests were fixed after
  the first green run; the child was not changed for them.

### Not done, by design

- No session, adapter or profile code, and no `play` helper that runs the child under the real shim: `ts-0087-mf` to
  `ts-0092-tw` own them.
- No recordings beyond what the child's own tests need: error responses, unknown updates, other stop reasons, usage
  cases, a cancel, a child that exits first and the stderr cases are written by the items that test them.
- No `dump_env`, `touch`, `write_file` or `git` steps, which fake Claude has for its own scenarios. A later item adds a
  step with a test, in the same commit.
- No test of the child's environment: the harness sets it, and nothing in the child reads it.
- No real ACP agent and no ACP crate. `/tmp/cdt-*` folders from other test targets of the workspace run were left where
  they were; `TempRoot` removes the ones this target makes.
