---
id: ts-0106-nr
kind: task
status: done
title: "Repository skills: measure a snapshot plugin"
owners: []
links:
  - { type: blocked-by, to: ts-0103-sa }
---

## Goal

Revision 21 has a phase read the repository's skills from the folder it starts in. `ts-0103-sa` found that no tested
line loads `.claude/skills/` without also loading the worktree's settings and hooks. The owner chose (2026-11-16) to
measure one more route before `ts-0104-gx` builds the line: **a plugin in the snapshot whose skills are the worktree's
own, live.**

The route: Collide writes a plugin folder in the snapshot (a `.claude-plugin/plugin.json` and nothing else of its
own) whose `skills` entry is a symlink to `<the folder the phase starts in>/.claude/skills`, so the skills are read
from the worktree and are not frozen at the line's commit. The phase's line is the one the owner chose in
`ts-0103-sa` (candidate 2: `--add-dir` and `CLAUDE_CODE_ADDITIONAL_DIRECTORIES_CLAUDE_MD=1`) plus `--plugin-dir
<that plugin folder>`. The plugin has no hooks, MCP servers, agents or commands, so it adds nothing that runs code.

No product code changes here: the output is test harness code, measurements and proposed plan text in this item.

**Part 1, with the fake.** In the `ts-0103-sa` harness (`host/tests/adapters/claude/repo_instructions.rs`), build the
plugin folder and the line as data and run it against the fake `claude`. Assert the four snapshot flags are still
present, no `<worktree>/.claude/` settings path is passed, the plugin folder holds only the manifest and the symlink,
and the symlink resolves to the worktree's `.claude/skills`. If the installed `claude --help` names no
`--plugin-dir`, record that and stop: the route is not open.

**Part 2, one real session, approved** (see `### Owner approval`). In the `ts-0103-sa` fixture (stand-in names), run
the line once with `claude` (haiku) and record from the `system` `init` line and the reply:

- whether `repo-canary-skill` is in the `init` line's `skills`, and under what name (a plugin may prefix it);
- whether the model can give the skill's word from its description;
- that the root `CLAUDE.md` and its import still load, and the nested one does not (as in `ts-0103-sa`);
- that no settings hook, local hook, MCP server or agent of the worktree loads, and no marker file appears;
- that auto memory stays off.

## Success criteria

- Part 1: a test for the plugin line that records what the fake saw and asserts the rules above; a test that the
  plugin folder holds nothing but the manifest and the symlink.
- Part 2: a table in this item like `ts-0103-sa`'s for the plugin line, with the `claude` version, the stream line that
  shows each finding, the spend against the cap and the marker files' absence.
- A recommendation: either `ts-0104-gx` adds the plugin to the `worktree` line, or skills do not load and the Roles
  page says so. Proposed text for 0.3 (the line) and Context options. Proposed, not applied.
- Nothing from the owner's real home is read or changed by a test, except what `claude` itself writes under its own
  state (listed in the item, none deleted). The temp roots are removed.
- No product code or plan text changes. No new crate.

## Tests

Write these first, in the `ts-0103-sa` test files:

- the plugin line through the fake, as above;
- the plugin folder's contents and the symlink's target;
- the Part 2 step as one gated test (`COLLIDE_REAL_CLAUDE=1` and a ledger), skipped without them and saying why, as
  the `ts-0103-sa` live tests are, through `run_session` (which checks the opt-in itself).

## Notes

- IMPLEMENTATION_PLAN.md: 0.3, the repository's own instructions (the line, measured `ts-0103-sa`) and Context
  options (`repo_instructions`); Stage 4a, "The repository's own instructions"; revision 21.
- `ts-0103-sa`: the harness, the fixture, the findings table, and its decisions 3 (config none in every real session)
  and 8 (the session outside the ledger).
- **The guards.** No test, stub or check may remove the opt-in check or the ledger of a gated test, not even to see a
  test fail. A gated test is seen red with its gate in place, or reported as unseen.
- Area: `host/tests/adapters/claude/` and the item's own Notes. No change in `host/src`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root. Settings files are fixtures.
- Stage 4a item, added 2026-11-16 by the owner after `ts-0103-sa`. It is built before `ts-0104-gx`, which it blocks.

### Owner approval

Given by the owner on 2026-11-16: one real session with `haiku`, `--max-budget-usd 0.05` on the line and $0.10 in all
(a ledger with that limit; one re-run if a fixture fault needs it), on the terms of `ts-0103-sa`: config none
(`--setting-sources ""`), a fresh temp repository, and the `claude` child may use the real `HOME` for its login only.
Nothing from the owner's account or home is written down.

### Built

Test harness only. `host/src`, `core/src` and `app/src` are unchanged, and so is the plan. Nothing was committed.

`claude --help` (2.1.326) names `--plugin-dir <path>` ("Load a plugin from a directory or .zip for this session
only"), so the route is open.

- **`host/tests/adapters/claude/repo_instructions.rs`** (Part 1, fake only):
  - `Candidate::AddDirSkillsPlugin` (candidate 5), in `candidate_line`: candidate 2's words and variable plus
    `--plugin-dir <snapshot>/plugins/repo-skills`.
  - `write_skills_plugin(snapshot, start_dir)`: writes the plugin in the snapshot (`PLUGIN_NAME`, `PLUGINS_DIR`): a
    `.claude-plugin/plugin.json` with a name and a description, and `skills`, a symlink to
    `<start_dir>/.claude/skills`. Nothing else.
  - `holds_only_the_skills_link(folder, start_dir)`: the plugin rule. The folder holds exactly the manifest folder and
    `skills`; the manifest folder holds only `plugin.json`; the manifest has no key but `name`, `description` and
    `version` (so no `hooks`, `mcpServers`, `agents` or `commands`); `skills` is a link that resolves to the start
    folder's `.claude/skills`.
  - `keeps_the_snapshot_line` gains one clause: a `--plugin-dir` must be a folder of the snapshot.
- **`host/tests/adapters/claude/repo_instructions_live.rs`** (Part 2, gated):
  - `PLUGIN_CAP_USD` ($0.05) and `PLUGIN_LEDGER_LIMIT_USD` ($0.10), this item's own constants. Another item's limit is
    unchanged.
  - `run_session_within(fixture, name, line, stdin, cap, limit)`: the body of `run_session`, with the cap and the
    limit as parameters. Both guards (the opt-in, then the ledger) are in it, so every item's session passes them.
    `run_session` calls it with ts-0103's constants. `collide_request_capped` and `unavailable_within` are the same
    kind of split.
  - `Findings` gains `skill_listed_as`: the entry of the `init` line's `skills` that is the fixture's skill, as listed.
    It matches the skill's name or `<prefix>:<name>`, so only the fixture's own name can be returned. `init_has`
    gains `plugins` (true for the fixture's plugin name only).
  - The gated test `live_the_snapshot_plugin_with_the_worktrees_skills_in_the_worktree`, through
    `run_session_within`.

Tests, all in the `claude_adapter` target:

- Part 1, through the fake:
  `repo_instructions::candidate_5_adds_the_skills_plugin_to_candidate_2_and_keeps_the_snapshot_flags` (the four
  snapshot flags, no `<worktree>/.claude/` word, candidate 2's words unchanged, the fake saw the same argv, variable and
  folder).
- The folder: `the_skills_plugin_folder_holds_only_the_manifest_and_the_link_to_the_worktrees_skills`,
  `the_skills_plugin_shows_the_worktrees_skills_as_they_are_now_not_as_committed` (an edit and a new skill made after
  the plugin was written show through the link),
  `the_plugin_check_refuses_more_than_the_manifest_and_the_link` (an extra folder, an extra manifest file, each of the
  four code-adding keys, a real folder in place of the link, a link elsewhere, a link to another start folder).
- The rule: `the_rule_refuses_a_plugin_folder_that_is_not_in_the_snapshot`.
- Part 2, offline: `repo_instructions_live::findings_find_the_skill_under_a_plugin_prefix_and_the_plugin_itself`,
  `findings_say_the_skill_and_the_plugin_are_not_listed_when_the_stream_has_neither`,
  `the_plugin_session_is_haiku_capped_at_five_cents_config_none_and_has_the_plugin`,
  `the_plugin_ledger_allows_one_session_and_refuses_a_second_that_could_pass_ten_cents`.
- Part 2, gated: `live_the_snapshot_plugin_with_the_worktrees_skills_in_the_worktree` (skipped with the reason without
  `COLLIDE_REAL_CLAUDE=1` and a ledger).

**Red at runtime.** Each offline test was run against a stub and failed on an assertion:
- `candidate_line` returning candidate 2's line for candidate 5: the candidate 5 test and the plugin session test;
- `write_skills_plugin` writing a copy of the skills in place of the link: the folder test, the live-link test and the
  plugin session test;
- `holds_only_the_skills_link` accepting everything: the refusal test;
- the `--plugin-dir` clause of the rule off: the rule test;
- `listed_as` matching the exact name only: the prefix test;
- `PLUGIN_LEDGER_LIMIT_USD` at $0.50: the ledger test; `PLUGIN_CAP_USD` at $0.10: the ledger test and the session test.
  The stubs were one at a time, and the files restored byte for byte after each. The negative test of the findings
  (`findings_say_the_skill_and_the_plugin_are_not_listed...`) and the folder tests' first assertions were not given a
  stub of their own: they guard the tests above from passing for the wrong reason.
- The gated test, with its gate in place: run by exact name with `COLLIDE_REAL_CLAUDE=1`, a throwaway ledger in my
  scratch folder, and `COLLIDE_REAL_CLAUDE_BIN` set to `/usr/bin/true` (the supported override), so no real `claude`
  ran. The opt-in and the ledger both passed, `run_session_within` ran `true`, and the test failed with "claude wrote
  no stream line". No guard was stubbed, disabled or bypassed. See Decisions, 4.

Checks (foreground): `cargo build --workspace`, `cargo test --workspace` (3912 passed, 0 failed; the `claude_adapter`
target alone, 576 passed), `cargo clippy --workspace --all-targets -- -D warnings` (after the last edit),
`cargo fmt --all --check`, `pnpm test`, `pnpm typecheck`, `pnpm --filter @collide/app build`, `pnpm build:packs` and
`pnpm check` all pass. No flaky test failed.

### Measured

`claude` 2.1.326 (installed; the tested range in 0.3 ends at 2.1.325), model haiku, 2026-11-16. One real session,
through the one gated test, `--test-threads=1`, with a fresh ledger of limit $0.10. No re-run: the fixture had no
fault.

What the session did: candidate 5 in a fixture worktree (`git worktree add`, stand-in names), config none
(`--setting-sources ""`), the snapshot's `--settings`, `--mcp-config` and `--strict-mcp-config`, `--add-dir
<worktree>`, `CLAUDE_CODE_ADDITIONAL_DIRECTORIES_CLAUDE_MD=1`, `--plugin-dir <snapshot>/plugins/repo-skills`,
`--max-budget-usd 0.05`, and the prompt of `ts-0103-sa` (read `sub/notes.txt`, then name the fixture's words and say
whether the instructions describe an auto memory directory). The stream was read in memory and not kept.

| file or question | candidate 5 | evidence |
|---|---|---|
| root `CLAUDE.md` | loaded | the reply held `plum-4417` |
| `@docs/guide.md` import | loaded | the reply held `fig-6159` |
| nested `sub/CLAUDE.md` | not loaded | the reply said `none` for the sub word |
| skill `.claude/skills/repo-canary-skill` | loaded | `init` `skills` held `repo-skills:repo-canary-skill` |
| the model gives the skill's word | yes | the reply held `lime-2284`, which is only in the skill's description |
| the plugin | loaded | `init` `plugins` held `repo-skills` (5 plugins; 4 in `ts-0103-sa`) |
| `.claude/settings.json` hook | not loaded | marker `project-hook` absent; no `project-hook-ran` text |
| `.claude/settings.local.json` hook | not loaded | marker `local-hook` absent; no `local-hook-ran` text |
| `.mcp.json` server | not loaded | marker `mcp-started` absent; `init` `mcp_servers` empty |
| `.claude/agents/` agent | not loaded | `init` `agents` lacked `repo-canary-agent` (7 agents, as before) |
| the snapshot's `--settings` | honored | one `SessionStart` hook ran: the snapshot's canary |
| auto memory (model's answer) | off | the reply said `MEMORY=no` |

The skill is listed with the plugin's name as a prefix: `repo-skills:repo-canary-skill`. The `init` line's counts were
the same as in `ts-0103-sa` except one more plugin (5, was 4) and one more slash command (57, was 56): the skill's
command. `additional_directories` was empty, as under line 2. The three marker files were absent.

Spend (US dollars, `total_cost_usd`): one session, $0.003288 of the $0.05 cap and the $0.10 limit.

Folders `claude` wrote under the real home (left in place; none deleted):
`~/.claude/projects/-private-tmp-cdt-51887-0-wt` (the session's project folder, keyed to the worktree). I did not look
for anything else it may have written. The temp roots are removed. Nothing under `~/.collide` or
`~/Library/Application Support/Collide`. No `claude` ran apart from `claude --version`, `claude --help` (each with
`env -i PATH=/usr/bin:/bin HOME="$HOME"`, by its full path: `claude` is not in that `PATH`) and the one session.

**Recommendation for `ts-0104-gx`.** Add the plugin to the `worktree` line, as
`--plugin-dir <snapshot>/plugins/repo-skills`, beside `--add-dir <the folder the phase starts in>` and the
variable. It loads the
repository's skills, read from the folder at launch and not frozen at the line's commit, and it keeps everything else
of candidate 2: no project setting, hook, MCP server or agent loads, the snapshot's hook still runs, auto memory stays
off. The cost is one plugin in the session, and the skill's name as `repo-skills:<skill>`. Unmeasured: see Not done.

Proposed plan text (not applied):

- 0.3, the repository's own instructions, "The line", in place of its last two sentences: "Measured `ts-0106-nr`
  (`claude` 2.1.326): adding `--plugin-dir <snapshot>/plugins/repo-skills`, a plugin that holds a manifest and one
  link, `skills`, to `<the folder the phase starts in>/.claude/skills`, makes the phase list the repository's skills
  as `repo-skills:<skill>` and show their descriptions to the model. The skills are read through the link when the
  session starts, so they are the worktree's own and not frozen at the line's commit. The plugin has no hooks, MCP
  servers, agents or commands, so it adds nothing that runs code. The contract test pins `--plugin-dir` (in `claude
  --help`) and the variable (not in it)."
- 0.3, Context options, `repo_instructions`: "`worktree` adds `--add-dir <the folder the phase starts in>`, the
  variable above and `--plugin-dir <snapshot>/plugins/repo-skills`; `none` is the line without them. A phase does not
  load the repository's nested `CLAUDE.md` files; the Roles page says so."
- Stage 4a, "The repository's own instructions": "Skills through a snapshot plugin were measured in `ts-0106-nr`: they
  load."

### Decisions for the owner

1. **Add the plugin to the `worktree` line.** Chosen: recommend yes. Alternative: leave skills out and say so on the
   Roles page.
   - Plugin: skills work as in plain `claude`, live from the worktree, and nothing that runs code comes in through the
     plugin. Cost: one more plugin in every phase; the skill is named `repo-skills:<skill>` (a `CLAUDE.md` that says
     "use the skill `foo`" still matches by its description, the model found the word from it); a skill's own
     content is the worktree's, so a skill can tell the model to run something, as a `CLAUDE.md` can.
   - No plugin: the line stays at candidate 2, with no link to maintain. Cost: a team's skills do not work in a run.
   Recommendation: the plugin, with the revision 21 warning covering `.claude/skills/**`, and the unmeasured items
   below checked in `ts-0105-jv` (the owner's run) or in a later gated test.
2. **The plugin's name.** Chosen: `repo-skills`, as measured; it is the prefix the model sees. Alternative:
   `collide-repo-skills`, which cannot collide with a plugin the person has, under config user. Recommendation: keep
   the measured name for now. Nothing in `claude` depends on it; `ts-0104-gx` can rename it with the fake test and
   one line in the contract test.
3. **The link is the default `skills` folder.** Chosen: the manifest holds a name and a description and no `skills`
   key; `claude` found `skills/` next to the manifest and followed the link. Alternative: a `skills` key in the
   manifest pointing at the folder. Recommendation: the default, because a manifest with only a name is the smallest
   thing that cannot add code, and `holds_only_the_skills_link` checks that.
4. **The gated test was seen red with its gate in place by pointing `COLLIDE_REAL_CLAUDE_BIN` at `/usr/bin/true`.**
   Chosen: it keeps the opt-in and the ledger as they are and runs no `claude`. Alternative: report the gated test as
   unseen. Recommendation: accept; it shows the test goes through `run_session_within` and fails when no stream
   comes. It does mean `COLLIDE_REAL_CLAUDE=1` was set once outside Part 2, with a throwaway ledger, for that one
   test by exact name.
5. **`.claude/commands/**` is not carried.** The plugin has no `commands`, as the item says, so a phase does not load
   the repository's custom commands. `ts-0104-gx` lists `.claude/commands/**` among the instruction paths of the
   warning. Chosen: nothing built. Alternative: link `commands` too (a command is a prompt, not code, but it is
   unmeasured, and it can name tools). Recommendation: drop `.claude/commands/**` from the warning's list while they
   do not load, and add them back if a later item links them.
6. **A repository without `.claude/skills`.** The link would dangle. What `claude` does with a plugin whose `skills`
   link dangles is unmeasured. Recommendation: `ts-0104-gx` writes the plugin only when `<folder>/.claude/skills`
   exists at launch, and the fake test covers the case where it does not (no `--plugin-dir`). A skills folder an agent
   makes mid-run does not load until the next phase, which the warning names.
7. **The session's shape.** Config none (`--setting-sources ""`), as `ts-0103-sa` decided (its 3). The user's own
   skills and plugins under config user are unmeasured. Recommendation: `ts-0105-jv` checks them.
8. **`run_session_within`.** Chosen: the cap and the limit are parameters of one function with both guards, and
   `run_session` keeps `ts-0103-sa`'s constants. Alternative: a second copy of the body. Recommendation: as built, so
   a guard is never missing from a copy.

- Owner decisions (2026-11-17, at the build): 1 to 8 as recommended, on delegation, except that the frontmatter
  question in Not done goes to `ts-0104-gx` first, by the owner: its builder runs one gated haiku session (cap $0.05,
  limit $0.10, config none) with a skill whose frontmatter has a marker hook and `allowed-tools`, and if either takes
  effect the plugin waits for the owner's call. 4 accepted: `COLLIDE_REAL_CLAUDE_BIN=/usr/bin/true` runs no `claude`
  and leaves both guards in place. The folder `claude` wrote under `~/.claude/projects/` was removed by exact name
  with the owner's OK.

### Not done, by design

- A second session, or a re-run: one session gave a clear answer and the fixture had no fault.
- Whether the model can invoke the skill (`Skill` tool) and run it: only the listing and the description were
  measured, which is what the item asked.
- A skill's own frontmatter: its `hooks`, `allowed-tools` and `model` keys. The plugin adds no hook of its own, but a
  skill from the worktree may carry them, and whether `claude` honors a skill's hooks when the skill comes through a
  plugin is unmeasured. Recommendation: measure it before `ts-0104-gx` ships the plugin, with a skill whose
  frontmatter has a hook that writes a marker, and the model asked to use the skill (one more gated session, a
  limit of its own).
- A dangling link; skills in subfolders (`sub/.claude/skills`); `.claude/commands`; the user's own skills and
  plugins under config user; a plugin folder at a path with spaces.
- Anything in `host/src`, `core/src`, `app/src` or the plan.
- A stream was never kept on disk; only the findings above are written down. Nothing from the owner's account or home
  is in this item.
