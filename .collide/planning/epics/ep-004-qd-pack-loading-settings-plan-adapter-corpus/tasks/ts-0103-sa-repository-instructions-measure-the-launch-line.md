---
id: ts-0103-sa
kind: task
status: done
title: "Repository instructions: measure the launch line"
owners: []
links:
  - { type: blocked-by, to: ts-0102-vu }
---

## Goal

Answer the question 0.3 leaves open before `ts-0104-gx` builds the change (revision 21; revision 22): **which launch
line loads the worktree's own instructions (its `CLAUDE.md` files and skills) while settings, hooks and MCP config stay
from the snapshot?** The plan says 4a measures it first: "with the fake and then once with the real `claude`".

Under today's line (`--setting-sources user`, `--settings`, `--mcp-config`, `--strict-mcp-config`) nothing from the
worktree loads (measured S1). The change must bring in only the instructions. No product code changes here: the output
is measurements, test harness code, and proposed plan text in this item.

**Part 1, with the fake** (a build agent; no `claude`, no network, no spend). Build the candidate launch lines as data
in a test and run each against the fake `claude` (`host/tests/adapters/claude/fake_claude.rs`), which records argv,
environment and working directory. Candidates, from `claude --help` and the plan's S1 and S9 notes; the real step
decides between them:

1. the current line with the working directory in the worktree (the baseline);
2. the baseline plus `--add-dir <worktree>`, and the environment variable that makes Claude Code read `CLAUDE.md` from
   added directories, if the installed `claude --help` or its documentation names one (the measurement finds out; do not
   assume it exists in 2.1.325);
3. the baseline with the instruction files' text read by Collide and passed as an appended system prompt, which
   loads `CLAUDE.md` text but neither skills nor `@` imports;
4. `--setting-sources user,project` with the snapshot's `--settings`: loads project settings and hooks, so it is here
   only to record that it fails the rule.

For each, a test records what Collide would pass and proves, for every candidate except 4, that the four snapshot flags
are still present and that no project settings path is named. The tests are the harness the real step reuses.

**Part 2, one real run, after the owner's approval** (see `### Owner approval`). In a temp repository (stand-in names
only), with canary text in each file below, run the candidate lines that Part 1 leaves with `claude` (haiku, one short
prompt each, the same gated harness as `ts-0070-mk`: `COLLIDE_REAL_CLAUDE=1`), and record from the stream's `system`
`init` message and the model's reply what loaded:

- a root `CLAUDE.md`, a nested `CLAUDE.md` in a subfolder, an `@import` inside one, and a project skill in
  `.claude/skills/<name>/SKILL.md`: should load;
- `.claude/settings.json` with a hook that writes a marker, `.claude/settings.local.json`, `.mcp.json` with a server
  that would write a marker, and `.claude/agents/<name>.md`: must not load, and the marker files must not appear;
- **a session started at the person's checkout** (revision 22): the root, with an untracked
  `.claude/settings.local.json` and `.mcp.json`, started with the snapshot's flags: loads none of them;
- **a `claude` that the agent starts itself**, inside a worktree it made with `git worktree add` (revision 22): which of
  the settings, hooks, MCP servers and instructions of that worktree it loads. The snapshot's flags reach only the
  session Collide starts, so the expected answer is "everything"; the measurement says so or not.

## Success criteria

- Part 1: each candidate has a test that names it, records the argv, environment and working directory the fake saw, and
  asserts the snapshot flags (`--setting-sources user`, `--settings <snapshot>/settings.json`, `--mcp-config
  <snapshot>/mcp.json`, `--strict-mcp-config`) are present in 1 to 3 and that no `<worktree>/.claude/` path is passed.
  Candidate 4 is asserted to fail that rule.
- The fake run proves the harness: the canary files exist in the worktree and the fake sees the same working directory.
- Part 2, only with the owner's approval recorded below: a table in this item with, for each candidate and each file
  above, `loaded`, `not loaded` or `not run`, with the `claude` version and the stream line that shows it; the spend
  against the cap; the marker files' absence; and the three questions of revision 22 answered (the person's checkout; an
  agent's own `claude` in its worktree; whether auto memory stays off under the chosen line).
- A recommendation for the line `ts-0104-gx` builds, or the statement that no tested line does it and what that means
  for the plan (for example "skills do not load, CLAUDE.md does"), with proposed text for 0.3 ("Unmeasured: which launch
  line loads ...") and for the Context options. Proposed, not applied.
- Nothing from the owner's real home is read or changed by a test, except what `claude` itself writes under its own
  state (reported, as `ts-0070-mk` did); the temp repository and the harness are removed at the end.
- No product code or plan text changes. No new crate.

## Tests

Write these first (Part 1), in a new test file under `host/tests/adapters/claude/` using the existing fake and canary
helpers:

- one test per candidate, as above; the baseline is first and equals `launch_spec`'s own output for a role with no
  options;
- a test that the harness' worktree holds the canary files and that the fake's recorded working directory is that
  worktree;
- a test that candidate 4 is refused by the rule (it names `user,project`);
- the Part 2 steps as gated tests (`COLLIDE_REAL_CLAUDE=1`), written so that without the variable they say they are
  skipped and why, as `real.rs` does. They are not run until the owner approves.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4a: "The repository's own instructions" (measure with the fake, then once with the real `claude`; the revision
    22 measurements).
  - 0.3: The tool's config paths in a repo (the repository's own instructions; "Unmeasured"); Context options
    (`repo_instructions`); Acknowledgement. 0.4: where an agent starts (the root session, revision 22). Revision 21 and
    revision 22: the worktrees an agent makes for itself.
  - S1 record: what loads under the snapshot line. `ts-0070-mk`: the gated real harness and its approval terms.
- The tested version range and the `claude` binary choice (`COLLIDE_REAL_CLAUDE_BIN`) are as `ts-0070-mk` left them.
- **Decision for the owner, with a recommendation: the real run.** It spends money and runs `claude`. Recommend the
  terms of `ts-0070-mk`: haiku, a per-step cap and a total of $0.50 (two to four short sessions), run once, in temp
  repositories made for it, with the `claude` child allowed the real `HOME` for its login only and Collide's own `HOME`
  and daemon root in a temp folder. The agent that builds this item does not run Part 2 until `### Owner approval` holds
  the owner's yes, the cap and the date.
- **Decision for the item:** if the candidates fail (nothing loads instructions without loading project settings),
  record that and stop. The recommended fallback, which `ts-0104-gx` then builds, is candidate 3 (CLAUDE.md text as
  appended system prompt) with skills not loaded, and a note on the Roles page later.
- **Decision for the item:** the temp repository's files use stand-in names (Orchard, a folder `docs/guide.md`), never
  the real repository's.
- The measurement can show that the chosen line also loads the user's own `CLAUDE.md` or auto memory; record it, since
  0.3 says the user's own stay on and auto memory is off by the snapshot's `autoMemoryEnabled: false`.

### Owner approval

Given by the owner on 2026-11-16, on the terms of `ts-0070-mk`: `haiku`, a cap on each step, $0.50 in all, a fresh
temp repository, and the `claude` child may use the real `HOME` for its login only. The build agent records here
which candidates were run and what was spent.

- Area: `host/tests/adapters/claude/` (the harness and the gated steps), the item's own Notes. No change in `host/src`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 4a item (revision 23 scope, split 2026-11-16). Items of Stage 4a run one at a time, in id order, because
  they share `core`, `host` and the generated files.
- Owner decisions (2026-11-16, at the split): the recommendations above are taken under the owner's delegation; they
  stay overridable until the item is built. Part 2's real run is approved (see `### Owner approval`).

### Built

Test harness only. `host/src`, `core/src` and `app/src` are unchanged, and so is the plan.

- **`host/tests/adapters/claude/repo_instructions.rs`** (Part 1, fake only):
  - `Fixture`: a git repository in a temp root with stand-in files only. The text of the files is in
    `fixtures/repo-instructions/` (Orchard, `docs/guide.md`): a root `CLAUDE.md` with an `@docs/guide.md` import, a
    nested `sub/CLAUDE.md`, a skill, an agent, project settings with a hook, and untracked local settings with a
    hook and an `.mcp.json` with a server (`Fixture::add_local_files`). Every hook and the server write a marker
    file in the root's `markers/` folder. `Fixture::worktree` makes a real `git worktree add`.
  - `Candidate`, `Line` and `candidate_line`: the candidate lines as data (1 baseline, 2 add-dir, 3 appended prompt
    file, 3b appended prompt inline, 4 `user,project`). The baseline is `launch_spec`'s own output.
  - `keeps_the_snapshot_line`: the rule. `--setting-sources` is `user` or empty, `--settings` and `--mcp-config` are
    the snapshot's files, `--strict-mcp-config` is present, and no word is under `<worktree>/.claude/`.
  - `instruction_text`: the text of every `CLAUDE.md` below a folder, which candidate 3 passes.
  - `through_the_fake`: starts fake Claude on a line and returns the argv, environment and working directory it
    recorded.
- **`host/tests/adapters/claude/repo_instructions_live.rs`** (Part 2, gated): `run_session` (one real session through
  `run_bounded`, the ledger asked first and told after), `Findings` (booleans and counts only, so nothing from the
  owner's account can be printed), `collide_request`, `agents_own_line`, five `live_*` tests, and offline tests of
  the harness itself.
- **Fake Claude** (`fixtures/fake_claude.py`, `fake.rs`): a `dump_launch` step writes the words, environment and
  working folder it was started with. The brief said the fake records these; it recorded only the environment, on
  request. `FakeClaude::start_on_line` starts it in a folder with extra variables.
- **`real.rs`**: `ledger_refusal` (the check as a pure function) and `check_ledger_within(name, cap, limit)`, which
  needs a ledger and takes the item's own limit. The ts-0070 limit ($1.50) and `check_ledger` are unchanged.
- **`main.rs`**: the two new modules.

Tests, all in the `claude_adapter` target:

- `fake_claude::a_dump_launch_step_writes_the_words_the_environment_and_the_working_folder_it_was_started_with`
- one per candidate, each through the fake:
  `repo_instructions::candidate_1_is_the_current_line_started_in_the_worktree`,
  `candidate_2_adds_the_worktree_and_the_variable_and_keeps_the_snapshot_flags`,
  `candidate_3_passes_the_claude_md_text_as_an_appended_prompt_file_and_keeps_the_snapshot_flags`,
  `candidate_3b_puts_the_same_text_on_the_line_which_is_why_the_file_form_is_the_one`,
  `candidate_4_loads_project_settings_so_the_rule_refuses_it_and_the_message_names_user_project`
- `candidate_3_text_holds_every_claude_md_and_neither_the_imports_nor_the_skills`
- the rule: `the_rule_names_each_snapshot_flag_a_line_loses`,
  `the_rule_refuses_a_line_that_names_a_project_settings_path_and_one_that_names_local_as_a_source`,
  `the_rule_accepts_config_none_which_is_an_empty_setting_sources`
- the harness: `the_worktree_holds_the_canary_files_and_the_untracked_local_ones` (the fake's working folder is the
  worktree, in each candidate test),
  `the_worktree_is_a_git_worktree_of_the_checkout_and_the_local_files_are_untracked_in_it`,
  `the_persons_checkout_starts_with_no_local_files_and_takes_them_untracked`,
  `the_markers_are_made_by_the_hooks_and_the_server_and_cleared_one_by_one`
- Part 2, offline: `repo_instructions_live::findings_read_the_init_lists_the_hooks_the_reply_and_the_cost`,
  `findings_say_not_loaded_for_a_name_the_stream_does_not_hold`,
  `what_findings_print_holds_no_name_from_the_stream_but_the_fixtures`,
  `every_collide_line_of_part_2_is_haiku_capped_at_ten_cents_and_config_none`,
  `an_agents_own_line_has_project_and_local_sources_and_no_user_source_and_no_mcp_config`,
  `the_first_turn_is_one_stream_json_user_message_with_the_prompt`, `the_prompt_does_not_hold_the_words_it_asks_for`,
  `the_items_ledger_refuses_a_session_that_could_pass_fifty_cents`,
  `the_part_2_tests_are_skipped_without_the_opt_in_and_say_why`, `a_session_is_not_run_when_no_ledger_is_named`
- Part 2, gated (`COLLIDE_REAL_CLAUDE=1` and a ledger; skipped with the reason otherwise):
  `live_candidate_1_the_current_line_in_the_worktree`, `live_candidate_2_add_dir_and_the_variable_in_the_worktree`,
  `live_candidate_3_the_instruction_text_as_an_appended_prompt_file`,
  `live_the_persons_checkout_with_untracked_local_settings_and_mcp_config`,
  `live_an_agents_own_claude_in_a_worktree_it_made_with_git_worktree_add`

**Red at runtime.** Each test above was run against a stub with the old behavior, and failed on an assertion:
- the step missing from the fake (the unchanged `fake_claude.py`: exit 96, "not a step");
- the rule stubbed to accept every line (the candidate 4 test and the two rule tests that expect a refusal), and
  stubbed to accept only `user` (the config none test);
- `candidate_line` stubbed to return the baseline in the checkout (candidates 1, 2, 3 and 3b), `instruction_text` to
  return nothing (the text test), `add_local_files` to do nothing and `worktree` to return the checkout (the fixture
  tests);
- `findings`, `first_turn`, `collide_request` (no config none, model or cap), `agents_own_line` (source `user`),
  `ledger_refusal`, `check_ledger_within` and `unavailable` stubbed, and the prompt given a canary word (all the
  Part 2 offline tests). The stubs of the last group were run one test at a time by exact name (see the incident
  below).

Checks: `cargo build --workspace`, `cargo test --workspace` (3901 passed, 0 failed, before the last two edits;
the `claude_adapter` target alone, 565 passed, after them), `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo fmt --all --check`, `pnpm test`, `pnpm typecheck`, `pnpm --filter @collide/app build`, `pnpm build:packs` and
`pnpm check` all pass. No flaky test failed.

### Measured

`claude` 2.1.326 (installed; the tested range in 0.3 ends at 2.1.325), model haiku, 2026-11-16. Run once through the
five gated tests, `--test-threads=1`. Two steps were run a second time (see Decisions, 5).

What each session did: the line of the candidate in a fixture worktree (`git worktree add`), config none
(`--setting-sources ""`, see Decisions, 3), the snapshot's `--settings`, `--mcp-config` and `--strict-mcp-config`,
`--max-budget-usd 0.1`, and one prompt: read `sub/notes.txt`, then name the fixture's words and say whether the
instructions describe an auto memory directory. The stream was read in memory and not kept.

| file | 1 baseline | 2 add-dir | 3 prompt file | 4 | person's checkout | agent's own |
|---|---|---|---|---|---|---|
| root `CLAUDE.md` | no | yes | yes | not run | no | yes |
| nested `sub/CLAUDE.md` | no | no | yes | not run | no | yes |
| `@docs/guide.md` import | no | yes | no | not run | no | yes |
| skill `.claude/skills/` | no | no | no | not run | no | yes |
| `.claude/settings.json` hook | no | no | no | not run | no | yes |
| `.claude/settings.local.json` hook | no | no | no | not run | no | yes |
| `.mcp.json` server | no | no | no | not run | no | yes |
| `.claude/agents/` agent | no | no | no | not run | no | yes |
| auto memory (model's answer) | off | off | off | not run | off | on |

"Person's checkout" is the baseline line started at the checkout, which has the committed files and untracked
`settings.local.json` and `.mcp.json`. "Agent's own" is `claude -p --setting-sources project,local` with no
`--mcp-config`, in a second worktree of the same repository.

Evidence, per kind of file:

- A `CLAUDE.md` file, an import: the model's reply held the word in the file (`plum-4417` root, `quince-8302`
  nested, `fig-6159` import). A reply that held none said `none`.
- The skill and the agent: the `system` `init` line's `skills` and `agents` lists held `repo-canary-skill` or
  `repo-canary-agent`. They did only for the agent's own `claude`.
- A hook: the marker file `markers/project-hook` or `markers/local-hook` was written, and a `SessionStart`
  `hook_response` held `project-hook-ran` or `local-hook-ran`. In lines 1 to 3 and the checkout the only `SessionStart`
  hook was the snapshot's canary (`collide-settings-canary`: it shows `--settings` was honored). The agent's own run
  had two, both the fixture's.
- The MCP server: `markers/mcp-started` was written and the `init` line's `mcp_servers` held `marker-server`. Both only
  for the agent's own `claude`.
- Auto memory: the reply's `MEMORY=`. This is the model's own answer, not a file. `claude` wrote a project folder
  named for the main checkout for the agent's own run, which is where its auto memory lives.
- In every session the `init` line had 7 agents, 4 plugins and 56 slash commands, so the lines differ in what the
  fixture added and nothing else the counts show. `additional_directories` was empty under line 2: `--add-dir` of the
  folder `claude` already starts in is not listed, and its `CLAUDE.md` loaded anyway.

Marker files: none was written under lines 1, 2, 3 or the person's checkout. All three were written under the agent's
own `claude`.

The three questions of revision 22:

1. **A session at the person's checkout, with the snapshot's flags**, loads none of the untracked
   `settings.local.json` and `.mcp.json`: no marker, no server in the `init` line, no extra hook. It loads none of the
   checkout's committed instructions either, as in a worktree.
2. **A `claude` that the agent starts itself in a worktree it made** loads everything of that worktree: project and
   local settings with their hooks, the `.mcp.json` server, the agent, the skill, and `CLAUDE.md` with its nested file
   and its import. Auto memory is on. The snapshot's flags reach only the session Collide starts, as expected.
3. **Auto memory stays off** under lines 1, 2 and 3 and at the checkout (the snapshot's `autoMemoryEnabled: false`
   still holds with `--add-dir` and the variable). It is on for the agent's own `claude`.

Not measured: the user's own `CLAUDE.md` under line 2 (every real session had config none, by the owner's terms), and
a nested `CLAUDE.md` under line 2 when the folder `claude` starts in is not the added one.

Spend (US dollars, `total_cost_usd`; every cap `--max-budget-usd 0.1`; a fresh ledger with a limit of $0.50):

| # | session | cost |
|---|---|---|
| 1 | agent's own `claude`, first run (the import line of the fixture was faulty, see Decisions, 5) | 0.002758 |
| 2 | candidate 1, baseline | 0.003224 |
| 3 | candidate 2, add-dir (first run, same fault) | 0.001909 |
| 4 | candidate 3, prompt file | 0.003351 |
| 5 | person's checkout | 0.001407 |
| 6 | agent's own `claude`, second run | 0.002812 |
| 7 | candidate 2, second run | 0.001574 |

Total $0.017035 of the $0.50 limit, seven sessions. Candidate 3b and candidate 4 were not run with `claude`.

**One session outside the ledger.** While I checked that the new offline tests fail against a stub, I stubbed the
function that skips the gated tests. The five gated tests then ran for a moment against the real `claude`, on stubbed
lines with `--setting-sources user`. Four had no first turn, so no model call. One, the agent's own line, ran its prompt
(haiku, `--max-budget-usd 0.1`) in the fixture checkout with the owner's user settings. Its cost is not known (at most
$0.10). I did not read its stream: the test printed only booleans and counts to a terminal I filtered, and nothing was
written. The new ledger counts it at its cap. Since then the stubs of the skip function and of the ledger check are only
used on tests that cannot reach `claude`, one test at a time by exact name. The real code has two guards before a
session (`unavailable` and `check_ledger_within`), and the stubs removed both.

Folders `claude` wrote under the real home (left in place; none deleted):

- `~/.claude/projects/-private-tmp-cdt-70167-0-work` (the agent's own run: the main checkout's key),
  `-private-tmp-cdt-70167-1-wt`, `-private-tmp-cdt-70167-2-wt`, `-private-tmp-cdt-70167-3-wt` and
  `-private-tmp-cdt-70167-4-work` (the person's checkout);
- `~/.claude/projects/-private-tmp-cdt-71478-0-work` and `-private-tmp-cdt-71478-1-wt` (the two second runs);
- `~/.claude/projects/-private-tmp-cdt-85716-16-work` (the session outside the ledger);

The temp roots are removed. Nothing under `~/.collide` or `~/Library/Application Support/Collide`.

**Recommendation for `ts-0104-gx`.** Build candidate 2: `--add-dir <the folder the phase starts in>` and
`CLAUDE_CODE_ADDITIONAL_DIRECTORIES_CLAUDE_MD=1`, added to the line when `repo_instructions` is `worktree`. It is the
only line that loads the root `CLAUDE.md` and its `@` imports with `claude`'s own loader, and it keeps every snapshot
flag, loads no project settings, hook, MCP server or agent, and leaves auto memory off. It has two gaps: a nested
`CLAUDE.md` and skills do not load. Candidate 3 loads nested files too (whole, at launch) and no imports. No tested
line loads skills without project settings.

Proposed plan text (not applied):

- 0.3, in place of "Unmeasured: which launch line loads ...": "Measured ts-0103-sa (claude 2.1.326): `--add-dir <the
  folder the phase starts in>` with `CLAUDE_CODE_ADDITIONAL_DIRECTORIES_CLAUDE_MD=1` makes a phase read that folder's
  `CLAUDE.md` and its `@` imports, while settings, hooks, MCP servers and agents stay from the snapshot and auto memory
  stays off. A nested `CLAUDE.md` and skills do not load. The variable is not in `claude --help`; the contract test
  pins it."
- 0.3, Context options, `repo_instructions`: "`worktree` adds `--add-dir <the folder the phase starts in>` and the
  variable above; `none` is the line without them. A phase does not load the repository's skills or nested `CLAUDE.md`
  files; the Roles page says so."
- 0.3, the person's checkout and an agent's own `claude` (revision 22): "Measured ts-0103-sa: a session at the person's
  checkout with the snapshot's flags loads none of its untracked local settings or MCP servers. A `claude` that an
  agent starts itself in a worktree loads all of that worktree's settings, hooks, local settings, MCP servers, agents,
  skills and instructions, with auto memory on."
- 0.4, the root session: "Stage 4a measured it (ts-0103-sa)."

### Decisions for the owner

1. **Which line `ts-0104-gx` builds.** Chosen: recommend candidate 2. Alternative: candidate 3.
   - Candidate 2: one flag and one variable, the loader of `claude` itself (imports, precedence). Cost: the variable is
     undocumented (in the binary's strings, not in `--help`; `--help` names `--add-dir` as the `CLAUDE.md` channel
     under `--bare`), so a release may drop it; the contract test would fail then. No nested `CLAUDE.md`, no skills.
   - Candidate 3: documented flag family, nested files included. Cost: Collide reads and passes the text (a second
     loader), every nested file is in context at launch whether or not the agent works there (more tokens), no
     imports, and `--append-system-prompt-file` is in the binary's strings and in `--help` only under `--bare`.
   - Neither loads skills. Carrying the repository's skills in a snapshot plugin (`--plugin-dir`) is an idea; it is
     unmeasured and it freezes them at the line's commit, which revision 21 chose not to do.
   Recommendation: 2, and re-measure on every `claude` upgrade through the gated test.
2. **The form of candidate 3.** Chosen: a file in the snapshot (`--append-system-prompt-file`), because the plan keeps
   prompt text out of argv and `ps`. The inline form (3b) is tested offline and puts the text in argv. It was not run
   with `claude`. Recommendation: the file form, if 3 is ever built.
3. **Config none in every real session**, as `ts-0070-mk` did: the role's `config: none` gives `--setting-sources ""`,
   so the owner's settings, hooks, plugins and `CLAUDE.md` did not load. Project files are in neither `user` nor `""`,
   so what loads from the worktree is the same. The user's own `CLAUDE.md` under the chosen line is unmeasured.
   Recommendation: keep; measure it in `ts-0105-jv`, which the owner runs.
4. **Candidate 4 was not run with `claude`.** It fails the rule by construction (`user,project` loads project settings
   and hooks); the item asked only to record that, which the test does. Recommendation: leave it.
5. **A fixture fault and two re-runs.** The first fixture wrote the import as `@docs/guide.md.` at the end of a
   sentence. The agent's own `claude`, which loads imports, did not load it, so the fault was the fixture's. I put the
   import on a line of its own and ran the two sessions it matters for (the agent's own, and candidate 2) once more.
   Candidates 1, 3 and the checkout were not re-run: nothing there can load an import. Recommendation: accept.
6. **How a hook and a skill are told apart.** Markers and the `init` line's lists show what loaded; the words in the
   model's reply show the `CLAUDE.md` text. The model's answer on auto memory is its own account of its instructions.
   An error in a reply would read as "not loaded", so a "no" here is weaker than a marker. Each "no" for a
   `CLAUDE.md` file has the matching "yes" in another column from the same prompt. Recommendation: accept.
7. **The tested range.** The installed `claude` is 2.1.326, above the range's 2.1.325. Nothing broke. Recommendation:
   `ts-0104-gx`'s contract step raises the range, with the add-dir pair pinned.
8. **The session outside the ledger** (see Measured): one haiku turn, at most $0.10, with the owner's user settings
   loaded and no stream read. Recommendation: accept, and note that a stub in a test must never remove the guards of a
   gated test. If the owner wants the stub-free version of the guard, a gated test could also refuse to run unless
   its own call site checks the opt-in (a second guard in `run_session`).

- Owner decisions (2026-11-16, at the build): 1 answered by the owner: candidate 2 is the line `ts-0104-gx` builds,
  and skills are measured first through a snapshot plugin in `ts-0106-nr` (one real session, $0.10), which blocks
  `ts-0104-gx`. The proposed 0.3 text is applied to the plan, adjusted for `ts-0106-nr`. 2 to 7 as recommended, on
  delegation. 8 disclosed to the owner; the eight folders `claude` wrote under `~/.claude/projects/` were removed by
  exact name with the owner's OK. At the review, `run_session` checks the opt-in itself (`real::unavailable`) before
  the ledger, so a test that skips `live!` or a stub of the file's `unavailable` cannot reach `claude` (test
  `a_session_is_not_run_without_the_opt_in_even_when_its_caller_does_not_check`, seen red); briefs now forbid
  stubbing a guard of a gated test.

### Not done, by design

- Candidate 4 and candidate 3b with `claude`.
- The user's own `CLAUDE.md` and auto memory under config user; a nested `CLAUDE.md` under line 2 with a folder
  `claude` starts in that differs from the added one; skills carried through a snapshot plugin.
- Anything in `host/src`, `core/src`, `app/src` or the plan. Nothing was committed.
- A stream was never kept on disk; only the findings above are written down.
- No test touches the real home, launchd or a daemon; the gated tests run `claude` with the owner's `HOME` for the
  login only.
