---
id: ts-0104-gx
kind: task
status: done
title: "Role options at launch: require, disallowed tools, config and repo_instructions"
owners: []
links:
  - { type: blocked-by, to: ts-0103-sa }
  - { type: blocked-by, to: ts-0106-nr }
---

## Goal

Make the role keys of `ts-0093-am` do something at launch, and build the repository's own instructions on the line that
`ts-0103-sa` measured. Four parts, each small, all in the Claude adapter's launch path and the file run that uses it.

1. **`require`.** `host/src/file_run.rs` `check_agent` calls `refuse_unmet_capabilities("claude", &capabilities(), &[])`
   with an empty list, because a role had no key (`ts-0061-bg`, `ts-0065-kx`). It passes the role's `require` now. A
   role that requires a capability the adapter says `No` or `Unknown` for is refused before the run starts, naming the
   role, the capability and what the adapter said (the refusal's own text). The built-in `builder` requires nothing, so
   no run changes.
2. **Asking for a disallowed tool.** `launch::disallowed_tools_without(&[])` takes the tools a role asked for.
   `launch_spec` passes the role's `allow_tools`. A name that is not in `DISALLOWED_TOOLS` is refused with
   `LaunchError::Invalid`, naming the tool and the role (a typo must not look like it worked). The other tools stay
   disallowed.
3. **`config: none`.** The carried follow-up of `ts-0070-mk` (decision 2): `collide run --config none|user` overrides
   the built-in role's `config` for that run, so a Collide task can run with no user settings without a wrapper script.
   The file-run request gains an optional `config` (`run.start` params; a wire change, `pnpm gen`), and `launch_spec`
   already reads `role.config`.
4. **`repo_instructions`.** The role key (`worktree`, the default, or `none`) picks the launch line `ts-0103-sa` chose:
   `worktree` loads the worktree's `CLAUDE.md` files and skills while settings, hooks and MCP config stay from the
   snapshot; `none` is today's line, byte for byte. If `ts-0103-sa` found no line that loads instructions without
   project settings, this part builds what that item recommended instead, and says so here.
   - **The warning.** A phase that starts after its run changed an instruction file shows a warning that names the
     files, on the run (in the run summary Tracks reads) and in the run record. The files are `CLAUDE.md` at any depth,
     `.claude/skills/**` and `.claude/commands/**`; "changed" is a difference from the commit the workspace was made
     from, committed or not, new or deleted. The first phase of a new run in a fresh workspace has none; a re-prompt
     run, which resumes in the same workspace, does. `RunSummary` gains `warnings` (a list of kind, phase and files),
     the store keeps them with the run (a migration), and `collide run` prints the line when it ends. The app's display
     on Tracks is 6a's; the acknowledgement before `integrate` (3c) is unchanged.

## Success criteria

- A role whose `require` names a capability the Claude adapter says `No` for is refused before any process starts, and
  the message names the role, the capability and `no`. `Unknown` is refused too. A role that requires only what the
  adapter says `Yes` for runs.
- With `allow_tools: [CronCreate]` the launch line's `--disallowed-tools` holds the other seven and not `CronCreate`.
  With no `allow_tools` it holds all eight, as today. `allow_tools: [NotATool]` fails naming it.
- `collide run --config none` gives `--setting-sources ""`, `--config user` gives `user`, no flag gives what the role
  has. A bad value is refused by the CLI.
- With `repo_instructions: none` the line equals today's for every role in the existing launch tests, byte for byte.
  With `worktree` (the default) the line is the one `ts-0103-sa` recorded, and the four snapshot flags are present.
- Under the fake `claude`, the recorded argv, environment and working directory for a default role equal the chosen
  candidate of `ts-0103-sa`.
- A second phase, or a re-prompt run, that starts after `CLAUDE.md` or a skill changed in the worktree carries a warning
  that names each changed file once, with its phase. A new `CLAUDE.md` in a subfolder, a deleted skill and an
  uncommitted edit each count. A run where no instruction file changed carries none. A change to `README.md` or to code
  does not count. The warning is in `run.get`/`run.list`, in the store after a daemon restart, and in what `collide run`
  prints.
- A role with `repo_instructions: none` shows no warning.
- The gated real-`claude` contract tests of `ts-0070-mk` still pass with the default role (run by the owner,
  `ts-0105-jv`); the automated suite runs against the fake only.
- `pnpm gen` is run for the wire change, and `pnpm check` passes. No new crate.

## Tests

Write these first:

- `require`: a fixture role per case (`Yes`, `No`, `Unknown`, empty) through `check_agent`, and the message;
- `allow_tools`: the argv with none, one and an unknown name; the order of the other seven is unchanged;
- `--config`: the CLI parse (valid, invalid), the file-run request, the argv; the wire round trip;
- `repo_instructions`: every existing launch test with `none`; the chosen candidate's argv and working directory with
  `worktree`; the snapshot flags always present; the fake `claude` records what it was given;
- the warning, against a temp git repository and workspace: no change, an edit, an uncommitted edit, a new nested
  `CLAUDE.md`, a deleted skill, a code change, a `README.md` change, a re-prompt run, a role with `none`; each phase
  index; the store round trip and a daemon restart; the CLI's line;
- the migration: an old database opens, and a run with no warnings reads as an empty list.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4a: "The repository's own instructions" (build it, with the role key `repo_instructions` and the warning when
    a run changed an instruction file).
  - 0.3: Capabilities (`require`); Disallowed tools; Context options (`config`, `repo_instructions`); The repository's
    own instructions (the warning; the acknowledgement before `integrate`). Revision 21.
  - 0.4: where an agent starts. 0.13: the fingerprint records the launch line's flags.
- Carried from `ts-0061-bg` (review): the capability refusal's role key and the disallowed-tool ask; `launch_spec`
  passed none until now.
- Carried from `ts-0065-kx`: `check_agent` checked the built-in role against an empty requirement.
- Carried from `ts-0070-mk` (decision 2): a role or `collide run` option for `config: none`.
- **Decision for the owner, with a recommendation: the default.** `repo_instructions` defaults to `worktree` (0.3,
  revision 21), so every `collide run` phase, the built-in `builder` included, now reads the repository's `CLAUDE.md`
  files. That changes what a run costs and does, and a team's instructions may say things a Collide run should not
  follow. The owner chose the default in revision 21, so recommend building it as the plan says, with `none` one key
  away. Alternative: build the key with default `none` first and switch the default after a dogfood week.
- **Decision for the item, with a recommendation: where "changed" is measured.** Against the commit the workspace was
  made from. A quick run holds it already (`QuickRun::base_commit`, `host/src/file_run/quick_run.rs`, used by accept:
  `ts-0079-pu`); a run that has none records it when the workspace is made, and the store keeps it. Alternative: compare
  with the line's ref now, which also reports another person's change that arrived by a fetch. Recommend the base: it
  reports only what this run did.
- **Decision for the item:** the instruction paths are listed in one constant (`CLAUDE.md`, `.claude/skills/**`,
  `.claude/commands/**`). `@`-imported files are not followed. Add `AGENTS.md` only if `ts-0103-sa` shows the line loads
  it.
- **Decision for the item:** `RunSummary.warnings` is a list of structs (`kind`, `phase`, `files`), not text, so the app
  can render them. The kind is `instruction_files_changed` now.
- The real-`claude` check of the chosen line is `ts-0105-jv` (owner-run). This item's tests use the fake only.
- Area: `host/src/file_run.rs`, `host/src/adapters/claude/launch.rs`, `host/src/run_cli.rs` (`--config`, the printed
  warning), `host/src/store.rs` and its migration, `core/src/owner/run.rs` (`warnings`, `config`), `schemas/`,
  `host/tests/adapters/claude/`, `host/tests`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 4a item (revision 23 scope, split 2026-11-16). Items of Stage 4a run one at a time, in id order, because
  they share `core`, `host` and the generated files.
- Owner decisions (2026-11-16, at the split): the recommendations above are taken under the owner's delegation; they
  stay overridable until the item is built. `repo_instructions` defaults to `worktree`, as revision 21 of the plan
  says (owner).
- Owner decisions (2026-11-16, after `ts-0103-sa`): the `worktree` line is candidate 2, `--add-dir <the folder the
  phase starts in>` with `CLAUDE_CODE_ADDITIONAL_DIRECTORIES_CLAUDE_MD=1`; it loads the root `CLAUDE.md` and its
  imports, and no nested `CLAUDE.md`. Skills follow `ts-0106-nr`: its plugin is added to the line if it works,
  otherwise skills do not load and the Roles page says so. The contract test pins the variable.
- Owner decisions (2026-11-17, after `ts-0106-nr`): the `worktree` line also carries the skills plugin of
  `ts-0106-nr` (`--plugin-dir <snapshot>/plugins/repo-skills`, a manifest and a `skills` link to `<the folder the
  phase starts in>/.claude/skills`), written only when that folder exists; with no such folder the line has no
  `--plugin-dir`. `.claude/commands/**` leaves the warning's list, since commands do not load. Before the plugin
  ships, one gated session measures a skill's own frontmatter (see `### Owner approval`); if its `hooks` or
  `allowed-tools` take effect through the plugin, build the line without the plugin, record the finding, and the
  owner decides (strip them from a copy, or accept them as plain `claude` would).

### Owner approval

Given by the owner on 2026-11-17: one gated session with `haiku`, `--max-budget-usd 0.05` on the line and $0.10 in
all (a ledger with that limit; one re-run if a fixture fault needs it), on the terms of `ts-0103-sa`: config none
(`--setting-sources ""`), a fresh temp repository, the `claude` child may use the real `HOME` for its login only, and
nothing from the owner's account or home is written down. The fixture skill's frontmatter has a `hooks` entry that
writes a marker in the temp root and an `allowed-tools` entry, and the prompt asks the model to use the skill. The
session goes through `run_session_within` of `ts-0106-nr`; no guard is stubbed.

### Built

- **`require`.** `host/src/file_run.rs` `check_role` (new, public) checks one role against a capability table and
  returns the refusal texts; `check_agent` calls it with the built-in role and `capabilities()`. A refusal names the
  role, the capability and what the adapter said. Tests: `host/tests/adapters/claude/capabilities.rs` (`Yes`, `No`,
  `Unknown`, empty; the `No` case clones the table, since Claude's own has no `No`).
- **`allow_tools`.** `host/src/adapters/claude/launch.rs` `check_allowed_tools` refuses a name that is not in
  `DISALLOWED_TOOLS` with `LaunchError::Invalid` (the tool and the role named); `disallowed_tools_without` gets the
  role's `allow_tools`. Tests in `host/tests/adapters/claude/launch.rs`: none, one, an unknown name, one unknown among
  known names (the whole request is refused), and the order of the other seven.
- **`--config`.** `core/src/owner/run.rs` `RunStartFileParams.config` (optional `RoleConfig`); `host/src/run_cli.rs`
  parses `--config none|user`, refuses other values naming the value, and refuses it with `--resume`. The file run
  keeps it in `RunContext` (a re-prompt run copies the prior run's). `pnpm gen` was run. Tests:
  `run_cli` (parse, bad value, `--resume`), `core/tests/owner_protocol.rs` (round trip, old payload),
  `host/tests/adapters/claude/run_options.rs` (the argv through the daemon and fake `claude`).
- **`repo_instructions`.** `host/src/adapters/claude/instructions.rs` (new): `is_instruction_path`,
  `write_skills_plugin`, and the constants of the line, with `PLUGIN_ON_THE_LINE` as the seam for the plugin.
  `launch_spec` adds, for `worktree` (the default), `--add-dir <cwd>` after `--disallowed-tools`, `--plugin-dir
  <snapshot>/plugins/repo-skills` when that folder exists, and `CLAUDE_CODE_ADDITIONAL_DIRECTORIES_CLAUDE_MD=1`. `none`
  is the old line, word for word. The plugin sits in the run's own snapshot folder (`runs_dir/<run>/snapshot`; a run
  has one phase, so it is that phase's own, and a resumed attempt rewrites it). It is a manifest and a `skills` link to
  `<start folder>/.claude/skills`, written only when that folder exists. `file_run.rs` `write_snapshot` writes it
  unless the role says `none`. Tests: `host/tests/adapters/claude/instructions.rs`; `repo_instructions.rs` builds its
  candidate lines on `none` now, and the default line is asserted equal to the chosen candidate (argv, environment,
  working directory), also through the fake `claude`.
- **The warning.** `host/src/instruction_warning.rs`: `changed_instruction_files` (git diff against the base, plus
  untracked files that are not ignored; each name once, in path order), `instruction_warning` and `not_yet_warned`.
  `file_run.rs` records the base commit in `RunContext` after provisioning (`QuickRun::base_commit` for prompt runs)
  and warns when an attempt starts. `core/src/owner/run.rs`: `RunWarning`, `RunWarningKind::InstructionFilesChanged`,
  `RunSummary.warnings`, `RunRecordResult.warnings`. `host/src/store.rs`: migration 8 `run_warnings`, with
  `add_run_warning` and `run_warnings`; `daemon.rs` fills `run.get`, `run.list` and the run record. `run_cli.rs`
  prints `warning: phase <phase> started after instruction files changed in the worktree: <files>`, with control
  characters of a file name removed. The app's fixtures (`app/src/components/placeholders.ts`,
  `app/test/support/loop-world.ts`) gain `warnings: []` so the app still type-checks; the display is 6a's. Tests:
  `instruction_warnings.rs` (git cases, re-prompt, nested file, deleted skill, uncommitted edit, `none`, list, record
  and restart), `store.rs` (migration from version 7, round trip, reopen), `run_cli.rs` (the printed lines).
- **The gated session.** `repo_instructions_live.rs`: the test
  `live_a_skills_own_frontmatter_through_the_snapshot_plugin` (constants
  `FRONTMATTER_*`: cap $0.05, ledger limit $0.10), `frontmatter_skill_text`, `frontmatter_line`, and the `Findings`
  fields for it. Its offline tests check the fixture text, the line (haiku, `0.05`, config none, the plugin),
  the ledger and what `findings` records (only the fixture's names; other tool and skill names are counted at most).
- **Seen red at runtime** (against the old behaviour or a stub): the four `run_cli` tests (unknown option `--config`,
  no warning text); three of the four `run_options` tests (with the request's `config` ignored and the default line
  without `--add-dir`); three of the four offline frontmatter tests and the findings test (by changing the cap, the
  fixture text and the permission count); the tests of the path list, the plugin writer, the warning function,
  `check_role` and `allow_tools` against stubs, before their code was written. Red was not recorded for the store
  migration tests. Not red, by nature:
  `config_user_and_no_config_both_give_user` (it states today's behaviour). Unseen: the gated test (with its gate in
  place it skips; it ran for real once, below).
- **Checks, last run:** `cargo build --workspace`, `cargo test --workspace` (no failures), `cargo clippy --workspace
  --all-targets -- -D warnings`, `cargo fmt --all --check`, `pnpm test`, `pnpm typecheck`, `pnpm --filter @collide/app
  build`, `pnpm build:packs` and `pnpm check` all pass.

### Decisions for the owner

1. **The plugin stays on the line, but the gated session did not answer its question.** See Measured. The rule of the
   brief (plugin off only if the hook ran or `allowed-tools` took effect) keeps it on. Alternative: take it off now,
   which is the safe side if a skill's `hooks` run outside the snapshot's control. Recommend one more gated session,
   $0.05, with a fixture that answers the host permission request for `Skill` and refuses `Write` (the harness of
   `real.rs` `run` has an `answer`), and keep the plugin on the line until it shows a hook or `allowed-tools` at work.
   If either takes effect, the choice is: strip `hooks` and `allowed-tools` from a per-phase copy of the skill (more
   code, one more thing that can drift from the repository), or accept them as plain `claude` would (the repository's
   own skills do what they say). Recommend accepting them, since a person's own `claude` runs them the same way.
2. **Ignored files do not count as changed.** The warning uses git's standard ignore rules for untracked files, so a
   file in `.gitignore` is not named. Alternative: name them too. Recommend the standard rules: ignored files are the
   person's own and do not arrive from the repository.
3. **A missing base does not stop a phase.** An old run context without a base commit logs and skips the warning. A git
   failure while reading changes does stop the phase, with the message. Alternative: warn nothing on any failure.
   Recommend this split: an unreadable workspace is a real fault.
4. **A re-prompt run measures against the base of the first run's workspace,** so it names every instruction file the
   work changed so far, once per run, not only the last attempt's. A file already named in this run is not named again
   (`not_yet_warned`).
5. **Plan text to propose** (Stage 4a, "The repository's own instructions"): the warning's list is `CLAUDE.md` at any
   depth and `.claude/skills/**` (commands are out); `AGENTS.md` and `@` imports are not followed; the line carries
   `--add-dir`, the variable and the skills plugin, and the plugin's skills are listed as `repo-skills:<name>`.

### Measured

One gated session on the terms of `### Owner approval`, and one re-run, which the terms allow for a fixture fault.
`claude` 2.1.326, `haiku`, `--max-budget-usd 0.05`, config none, `child_env()`, through `run_session_within`, a fresh
ledger with a limit of $0.10. Spend: $0.003419 and $0.001914, $0.005333 in all. Both ran with the opt-in and the
ledger in place.

- The fixture skill was listed as `repo-skills:frontmatter-canary-skill`: the plugin lists a skill with its prefix.
- The model called `Skill` three times in each session. Every call was an error result that held the word
  `permission`. No `can_use_tool` request came, `Write` was never called, no `PreToolUse` hook started, and neither
  the hook's marker nor the output file was written.
- The first session's prompt named the skill without the prefix. The re-run used the prefix, and the result was the
  same, so the prefix was not the cause. The likely cause is that the `Skill` call itself needs a permission that no
  host answers: `run_session_within` closes stdin after the first turn.
- Result: neither the hook nor `allowed-tools` was seen to take effect, and neither was seen not to. The skill never
  ran, so the question is open. The ledger holds $0.094667 of its limit, and the one re-run is used.
- Folders `claude` wrote under the real home are in the report to the caller, not here.

- Owner decisions (2026-11-17, at the build): 1 is overridden by the owner: the plugin is off the line
  (`PLUGIN_ON_THE_LINE = false`) until a session shows whether a skill's frontmatter takes effect through it; that
  session is `ts-0107-rj` (one more haiku session, $0.05, inside this item's $0.10), whose harness answers the
  `Skill` permission request. The strip-or-accept choice waits for its result. 2 to 4 as recommended, on delegation;
  5 applied to the plan with the plugin off. At review: the tests that pinned the plugin on the line now follow the
  seam, and `frontmatter_line` names the plugin by hand while the seam is off, since the plugin is what it measures.
  The two folders `claude` wrote under `~/.claude/projects/` were removed by exact name with the owner's OK.

### Not done, by design

- The app's display of warnings on Tracks (6a); the acknowledgement before `integrate` is unchanged.
- No real `claude` run of the default line beyond the gated session; the contract tests of `ts-0070-mk` with the
  default role are `ts-0105-jv`'s.
- `@` imports and `AGENTS.md` are not followed or listed. `.claude/commands/**` is not in the list.
- No per-phase copy of a skill with `hooks` and `allowed-tools` removed (Decision 1).
- `IMPLEMENTATION_PLAN.md` is unchanged; the text is proposed in Decision 5.

