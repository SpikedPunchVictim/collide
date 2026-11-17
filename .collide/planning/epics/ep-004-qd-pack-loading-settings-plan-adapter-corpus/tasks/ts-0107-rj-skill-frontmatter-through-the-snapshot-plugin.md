---
id: ts-0107-rj
kind: task
status: active
title: "Skill frontmatter through the snapshot plugin: measure"
owners: []
links:
  - { type: blocked-by, to: ts-0104-gx }
---

## Goal

`ts-0104-gx` built the `worktree` line and the skills plugin of `ts-0106-nr`, with the plugin off the line
(`PLUGIN_ON_THE_LINE` in `host/src/adapters/claude/instructions.rs`). Its gated session could not answer whether a
skill's own frontmatter takes effect through the plugin: the model's `Skill` calls were refused with a permission error,
because the session's harness closes stdin and answers no permission request (its Measured section). Plan 0.3 says that
what runs code or grants access without the model asking comes from the snapshot, so a repository skill's `hooks` (they
run code) and `allowed-tools` (they skip a permission request) matter.

**One gated session, approved** (see `### Owner approval`), with a harness that answers permission requests: the host
allows `Skill` and refuses every other tool. The fixture is the one of `ts-0104-gx` (a skill in the temp worktree's
`.claude/skills/` whose frontmatter has a `hooks` entry that writes a marker in the temp root and an `allowed-tools`
entry for `Write`), on the line `frontmatter_line` builds (Collide's line, with the plugin named by hand while the seam
is off). Record from the stream and the temp root:

- the skill's listing, and whether `Skill` ran (a `can_use_tool` request for it, answered allow);
- whether the hook's marker was written (the frontmatter hook ran);
- whether `Write` ran without a `can_use_tool` request (`allowed-tools` took effect), or a request for it came and
  was refused (it did not);
- the spend.

**The outcome.** If neither the hook nor `allowed-tools` took effect, set `PLUGIN_ON_THE_LINE` to `true` (the tests of
`ts-0104-gx` follow the seam) and propose the plan text that says so. If either took effect, the plugin stays off, the
finding is recorded, and the owner chooses: strip `hooks` and `allowed-tools` from a per-phase copy of the skills, or
accept them as plain `claude` would. If the session again cannot tell (the skill never runs), the plugin stays off and
the item says why.

## Success criteria

- The gated test, through a path that checks the opt-in (`real::unavailable()`) and then the ledger
  (`real::check_ledger_within`) before anything starts, as `run_session_within` does. It is skipped without them and
  says why.
- A table in this item of what the session showed, with the `claude` version, the stream line for each finding and
  the spend, under `### Measured`.
- `PLUGIN_ON_THE_LINE` set by the rule above; the item's tests and the full gates pass either way.
- Nothing from the owner's real home is read or changed by a test, except what `claude` itself writes under its own
  state (listed in the report, none deleted). The temp roots are removed. No new crate.

## Tests

- An offline test (no `claude`) that the session's answer function allows `Skill` and refuses every other tool, seen
  red at runtime.
- An offline test that the gated session's line is `frontmatter_line`'s, with haiku, the cap, config none, the plugin
  and the variable (`ts-0104-gx` has one; extend it if the harness changes).
- The gated test. It is seen red only with its gates in place, or reported as unseen.

## Notes

- IMPLEMENTATION_PLAN.md: 0.3, The repository's own instructions ("The line"); Stage 4a Status.
- Builds on `ts-0104-gx` (`repo_instructions_live.rs`: `frontmatter_line`, `add_frontmatter_skill`,
  `describe_frontmatter`, `FRONTMATTER_*`) and on `real.rs` `run`, whose `Case::answer` answers permission requests.
  `real::run` checks the ledger but not the opt-in: the session's path must check both.
- Added by the owner on 2026-11-17, after `ts-0104-gx`'s session could not tell. Built before `ts-0105-jv`, so the
  owner's contract run checks the line as it ships.

### Owner approval

Given by the owner on 2026-11-17: one gated session with `haiku`, `--max-budget-usd 0.05` on the line, and a fresh
ledger with a limit of $0.05, so there is no re-run ($0.005333 was spent in `ts-0104-gx`; both items together stay
inside its $0.10). The terms of `ts-0103-sa`: config none (`--setting-sources ""`), a fresh temp repository, the
`claude` child may use the real `HOME` for its login only, and nothing from the owner's account or home is written down.
No guard is stubbed.

### Built

Test harness and item text only. The one product constant, `PLUGIN_ON_THE_LINE`, is unchanged (`false`: see Measured).
Nothing was committed. All of it is in `host/tests/adapters/claude/repo_instructions_live.rs`.

- **The host's answer.** `skill_only`: allows `Skill`, refuses every other tool (or a request with no tool name) with
  the message "This measurement allows the Skill tool only."
- **The answering loop.** `react(session, line, at_ns, answer)` feeds one stdout line to the session, answers each
  permission request it raises, and returns the writes, the answers (tool and allowed or not) and whether the turn
  ended. It is pure of any process, so it is tested offline on the real `ClaudeSession`.
  `run_answering_session(fixture, name, request, line, prompt, cap, limit, answer)` is the live driver: it spawns the
  binary on `line` (`env_clear`, `child_env()`, the line's variables), writes the first turn, answers through `react`,
  closes stdin when the turn ends, and kills its own child at the timeout, charging the whole cap to the ledger.
- **The guards.** `guard_session(name, cap, limit)` is the opt-in (`real::unavailable()`), then the ledger
  (`real::check_ledger_within`). `run_session_within` and `run_answering_session` both call it first, before anything
  starts. `settle` is the shared tail (findings, ledger entry, asserts that the session ran).
- **`real::run` was not reused.** It builds its own request and snapshot, so it cannot take `frontmatter_line`; it
  also checks the ledger but not the opt-in. See Decision 1.
- **Constants.** `ANSWERED_CAP_USD` and `ANSWERED_LEDGER_LIMIT_USD`, both $0.05. No other item's constant changed.
  `frontmatter_line` takes the cap as a parameter now (the old callers pass `FRONTMATTER_CAP_USD`).
- **Findings.** `Findings` gains `permission_allowed`, `permission_refused` (set by the driver from its answers) and
  `evidence`: the first stream line, counted from 1 among the JSON lines, that shows `init`, `pre_tool_use_hook`, and
  for each named tool `<tool>_called`, `<tool>_asked`, `<tool>_result_ok`, `<tool>_result_error`. A result is put
  against its tool by the call's id. `describe_frontmatter` prints them.
- **The gated test.** `live_a_skills_own_frontmatter_when_the_host_answers_permission_requests`: skipped without the
  opt-in and a ledger, with the reason; otherwise one session on `frontmatter_line`, with `skill_only`.

Tests, in the `claude_adapter` target:

- `the_host_allows_the_skill_tool_and_refuses_every_other_tool_with_a_message`
- `a_skill_request_is_answered_allow_and_a_write_request_deny_with_a_message_on_the_wire`
- `a_line_that_asks_for_nothing_writes_nothing_and_the_result_line_ends_the_turn`
- `findings_give_the_stream_line_of_the_first_call_request_and_result_of_each_named_tool`
- `the_description_says_what_the_host_allowed_and_refused_and_where_the_stream_showed_it`
- `the_answering_session_is_not_run_without_the_opt_in_and_says_why`
- `the_answering_session_is_the_built_line_haiku_five_cents_config_none_the_plugin_and_the_variable`
- `the_answering_ledger_has_room_for_one_session_and_none_for_a_second`
- gated: `live_a_skills_own_frontmatter_when_the_host_answers_permission_requests`

**Seen red at runtime**, against stubs (`skill_only` allowing everything, `react` returning nothing, `findings` with no
evidence, `run_answering_session` as `todo!()`), each on an assertion before the code was written: the first seven
tests above. The line test and the ledger test were red with the constants set to $0.10 (the values of
`ts-0104-gx`), and green at $0.05. No guard was stubbed, disabled or bypassed for any of it. The opt-in test went red
against the `todo!()` stub of the function, not by touching a guard.

**The gated test with its gates in place, before the real session.** Without the opt-in it skips. With
`COLLIDE_REAL_CLAUDE=1`, a throwaway ledger and `COLLIDE_REAL_CLAUDE_BIN` pointing at a 15-line Python fake in a
scratch folder (it writes an `init`, two `can_use_tool` requests and a `result`, and waits for stdin to close), it ran
the whole loop: `Skill` answered allow, `Write` answered refuse, stdin closed at the turn's end, exit 0, the ledger
took the entry. A second run against the same ledger was refused by the ledger guard ("$0.0010 spent and a cap of
$0.05 would pass $0.05"). No `claude` ran in either. See Decision 3.

**Checks, last run, foreground:** `cargo build --workspace`, `cargo test --workspace` (3991 passed, no failures),
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`, `pnpm test`, `pnpm typecheck`,
`pnpm --filter @collide/app build`, `pnpm build:packs` and `pnpm check` all pass. No flaky test failed. No wire type
changed, so `pnpm gen` was not run.

### Decisions for the owner

1. **Plugin off the line; the owner chooses.** Both frontmatter keys took effect (Measured). The rule of the item leaves
   `PLUGIN_ON_THE_LINE` at `false`. The choices:
   - **Strip** `hooks` and `allowed-tools` from a per-phase copy of the skills. Pro: keeps the plan's rule, that what
     runs code or grants access without the model asking comes from the snapshot; a repository skill cannot run a
     shell command or skip Collide's permission step. Con: more code (a copy at each phase start, a frontmatter
     parser, other keys to review), and the skills are a copy as of the phase's start, not a link.
   - **Accept** them as plain `claude` would. Pro: no code; the repository's skills do what they say. Con: a skill's
     hook runs a command, and `allowed-tools` skips the host's permission request, so a repository can grant itself
     a tool in a Collide run.
   - **Leave the plugin off** and say on the Roles page that skills do not load. Pro: nothing to build or review.
     Con: a team's skills do not work in a run.
   - **Recommendation: strip.** The permission step is the product's core promise, and `allowed-tools` skipped it for
     `Write` here with the host's rules in force. If the owner wants the least work, leave the plugin off until a
     later item builds the copy. Unmeasured and worth a check in that item: other frontmatter keys (`model`,
     `context`, `agent`) and `allowed-tools` for `Bash`.
2. **The `Write` finding has no control run.** The snapshot's settings hold no allow rule and the line sets no
   permission mode, so a `Write` is asked about by default (as `ts-0104-gx` assumed), and none was asked. That points at
   `allowed-tools`. A session on a skill without `allowed-tools` was not run: the terms allow one session. Alternative:
   one more approved $0.05 session as a control. Recommendation: not needed if the owner strips both keys anyway.
3. **A scratch fake binary through the supported override.** To exercise the driver before spending the one session,
   `COLLIDE_REAL_CLAUDE=1` was set twice, with a throwaway ledger and `COLLIDE_REAL_CLAUDE_BIN` at a script that is not
   `claude` (as `ts-0106-nr` Decision 4 did with `/usr/bin/true`). Both guards were in place and one refused the second
   run. Recommendation: accept; a driver fault would have used the only session.
4. **A new driver, not `real::run`.** Chosen: `run_answering_session` beside `run_session_within`, sharing
   `guard_session` and `settle`. Alternative: give `real::run` a way to take a ready line. Recommendation: as built;
   `real::run` is the contract tests' harness and builds its own snapshot.
5. **Plan text to propose** (0.3, "The line", in place of the last two sentences, which say the plugin is off until a
   session shows whether frontmatter takes effect): "Measured `ts-0107-rj` (`claude` 2.1.326, host answering
   permission requests): a skill's own frontmatter takes effect through the plugin. Its `hooks` ran (a `PreToolUse`
   hook wrote its marker) and its `allowed-tools` skipped the permission request for `Write`. So the plugin stays off
   the line until [the owner's choice: a per-phase copy without those keys, or accepting them]." Stage 4a status: add
   "`ts-0107-rj` measured: frontmatter takes effect through the plugin; the plugin is off."

- Owner decisions (2026-11-17, at the build): 1, accept, by the owner: a repository skill's own `hooks` and
  `allowed-tools` take effect as in plain `claude`, so `PLUGIN_ON_THE_LINE` is `true` and plan 0.3 says so; each
  `Skill` call still asks the host first (Measured), so the person sees the skill before its frontmatter acts. 2, no
  control run: not needed once the keys are accepted. 3 and 4 as recommended, on delegation: the scratch binary left
  both guards in place. 5 applied with the owner's choice. The folder `claude` wrote under `~/.claude/projects/` was
  removed by exact name with the owner's OK.

### Measured

One gated session on the terms of `### Owner approval`: `claude` 2.1.326, `haiku`, `--max-budget-usd 0.05`, config none
(`--setting-sources ""`), `child_env()`, Collide's line with the plugin named by hand, a fresh ledger with a limit of
$0.05, through `run_answering_session` with `skill_only`. Both guards were in place. No re-run. Exit 0, result
`success`. Stream lines are counted from 1 among the JSON lines.

| question | finding | evidence |
|---|---|---|
| the skill's listing | listed as `repo-skills:frontmatter-canary-skill` | `init` line 3, `skills` |
| the model called `Skill` | yes, by the prefixed name | `tool_use` line 4 |
| `Skill` asked the host | yes, answered allow | `can_use_tool` line 5 |
| `Skill` ran | yes | non-error `tool_result` line 6 |
| the frontmatter hook ran | yes: marker present, one `PreToolUse` hook started | `hook_started` line 10; marker file |
| `Write` called | yes | `tool_use` line 9 |
| `Write` asked the host | no: the only request was for `Skill` | no `can_use_tool` for `Write`; refused `[]` |
| `Write` ran | yes, with no request: `allowed-tools` took effect | `tool_result` line 12 (ok); output file present |
| the snapshot's canary hook | ran | one `SessionStart` hook, the canary text |
| tool errors | none | no error result in the stream |

The hook started at line 10, after the `Write` call (line 9) and before its result (line 12): the hook is the skill's
own `PreToolUse` entry on `Write`. No settings, project or local hook, no MCP server and no marker of ts-0104's
fixture appeared.

Result by the rule: both the hook and `allowed-tools` took effect, so `PLUGIN_ON_THE_LINE` stays `false` and the owner
chooses (Decision 1).

Spend (US dollars, `total_cost_usd`): $0.003419 of the $0.05 cap and the $0.05 limit. The ledger holds that one entry.
Nothing from the owner's account or home is in this item; the folders `claude` wrote under the real home are in the
report to the caller.

### Not done, by design

- A re-run, or a control session on a skill without `allowed-tools` (Decision 2): the terms allow one session.
- A per-phase copy of the skills with `hooks` and `allowed-tools` removed: it waits for the owner's choice.
- `PLUGIN_ON_THE_LINE` is not set to `true`: the rule says to leave it `false` when either key took effect.
- Other frontmatter keys, `allowed-tools` for other tools, and skills in subfolders.
- `IMPLEMENTATION_PLAN.md` is unchanged; the text is proposed in Decision 5.
