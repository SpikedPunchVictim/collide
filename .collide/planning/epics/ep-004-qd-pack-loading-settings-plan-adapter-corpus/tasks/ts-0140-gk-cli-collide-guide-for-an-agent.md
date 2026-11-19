---
id: ts-0140-gk
kind: task
status: ready
title: "CLI: collide guide, for an agent in your own terminal"
owners: []
links:
  - { type: blocked-by, to: ts-0139-wq }
---

## Goal

Any Claude Code session the user runs can drive Collide through the CLI, so the user's own terminal works before the
Desk exists (`IMPLEMENTATION_PLAN.md` Stage 4c, Your own terminal; revision 24).

What exists. `COMMANDS` in `host/src/commands.rs` holds `run`, `tasks` and `task`; `task new`, `task <id>` and `task
<id> link` are forms of `task`. Exit codes 10 (a view error), 11 (a write queued) and 12 (a person-only op waiting in
the app) come from `ts-0120-wm` and `ts-0128-bb`. Every write goes through the daemon after `ts-0136-rr`, so the app
shows it as it lands. A run's agent has no `collide` on its `PATH` today: the launch takes `PATH` from the request
(`host/src/launch.rs`), and a headless run's Bash call is a permission prompt to the host (`--permission-prompts host`,
`host/src/adapters/claude/launch.rs`). The quick run's first turn is the task text alone; its standing instructions are
`BUILDER_DEFINITION` in `host/src/builtin_roles.rs`.

What this adds.
- `collide guide` prints a short guide for an agent: what Collide is; the commands it may use, `run`, `tasks` and `task`
  from `COMMANDS` with their summaries, and the forms `task create` (an item of the plan), `task new` (a scratch
  task), `task <id>` and `task <id> link`; the three exit
  codes; and that the person-only ops (accept, defer, reject, resolve, reopen) wait for the person in the app. It needs
  no daemon and writes nothing.
- A docs page, `docs/own-terminal.md`: put `collide` on `PATH`, and add one line to the repository's or the user's
  CLAUDE.md: "This repo is planned in Collide; run `collide guide`." Collide writes nothing into the user's Claude
  config.
- `collide` for a run's agent (0.2): every run's launch puts a directory holding the `collide` binary the daemon runs
  from first on `PATH`, and Claude's launch line allows `Bash(collide:*)` (`--allowed-tools`, beside the existing
  `--disallowed-tools`). An ACP agent's permission requests are answered as today.
- A paragraph in `BUILDER_DEFINITION`: file follow-ups with `collide task create` (0.10, Follow-ups), not only in the
  last reply. The planning workflow's prompt gets the same paragraph in Stage 9d.

## Success criteria

- `collide guide` with no daemon running and an empty `HOME` prints the three commands, their forms and the three exit
  codes, exits 0, and starts no daemon.
- A test fails when a command the guide lists leaves `COMMANDS` or is renamed, or a form leaves `task`'s parser.
- The guide holds no path, task text or setting from the board.
- A fake-adapter run's launch has the `collide` directory first on `PATH` and the `Bash(collide:*)` rule in its argv.
- `BUILDER_DEFINITION` names `collide task create` for follow-ups, pinned by a test.

## Tests

Write these first.

- `collide guide` through the binary under the test rule; the daemon root stays empty.
- The drift test against `COMMANDS` and `task`'s parser.
- The launch's `PATH` and argv for a fake-adapter run; the existing launch-line tests updated only where the new rule
  appears.
- The paragraph in `BUILDER_DEFINITION`.

## Notes

- IMPLEMENTATION_PLAN.md: Stage 4c (Your own terminal, its success line); 0.2 (the CLI from a terminal; the socket token
  bullet, for a run's agent); 0.10 (Follow-ups); the revision 24 entry.
- **Decision for the item, with a recommendation: how a headless run may call `collide`.** Recommend the allow rule on
  the launch line, so Claude Code's own matching, which splits compound commands, passes `collide …` and any other
  command still prompts the host. Alternative: answer each `collide` prompt in the host by reading the command, which
  means Collide parses shell.
- Whether real Claude honours the rule beside `--permission-prompt-tool stdio` is unmeasured; `ts-0138-pn` checks it
  with a real run.
- **Decision for the item, with a recommendation: plain text only.** The guide is read by an agent as text. Alternative:
  `--json`, which no reader needs yet.
- **Decided at the `ts-0139-wq` review: both forms in the guide, each with its meaning.** `task create` makes an
  item of the plan, and is how an agent files a follow-up; `task new` makes a scratch task in Collide's store, which
  is no follow-up. Revision 24 took them for one command; the plan text is corrected.
- Area: `host/src/commands.rs` and a `guide` command, the run's launch environment (`host/src/launch.rs`) and Claude's
  launch line (`host/src/adapters/claude/launch.rs`), `host/src/builtin_roles.rs`, `docs/`, `host/tests/`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test that runs the `collide` binary
  clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as
  `host/tests/common/mod.rs` `TempRoot` does. Settings files are fixtures, never the owner's own `settings.json`. A temp
  repository sets `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_SYSTEM` to empty files.
- Stage 4c item, added by revision 24 (2026-11-18). Built after `ts-0139-wq`; `ts-0138-pn` waits for it.
- Nothing here runs a real `claude`. No dependency is added.
- From `ts-0139-wq`: a run's agent files a follow-up with `collide task create` (the planning source), which the create
  rule makes private with `raised-by` and `run:`. `collide task new` makes a scratch task, which the rule does not
  touch. A quick run's task is a scratch task today, so its follow-ups get `run:` and no `raised-by`.
- Owner decisions: none recorded yet. The recommendations stand on the owner's delegation.

### Built

- `host/src/guide.rs` (new), `collide guide`. `text()` builds the guide from the CLI's own tables: `LISTED_COMMANDS`
  (`run`, `tasks`, `task`) are looked up in `commands::COMMANDS` and printed with their summaries; `TASK_FORMS` names
  `task create`, `task new`, `task <id>` and `task <id> link`, each with its meaning and an example that
  `task_cli::parse_task` must read as that form (`Form::is_read_as`); exit codes 10, 11 and 12 print from
  `task_cli::exit`; the five person-only ops print from `auth::PersonOnlyOp::name`. It is plain text of at most 120
  columns, has no `/` in it, and takes no input, so it holds nothing of the board.
- `host/src/commands.rs`: `guide` is the twelfth entry of `COMMANDS` (built), so `--help` lists it.
  `host/src/bin/collide.rs`: `guide` is handled before anything else is read, prints the text and exits 0; an extra
  argument is exit 2. It starts no daemon and writes nothing.
- `host/src/adapters/claude/launch.rs`: `ALLOWED_TOOLS` (`Bash(collide:*)`), pushed as `--allowed-tools` after the
  `--disallowed-tools` block on every line, also for a role that allows all eight disallowed tools.
- `host/src/file_run/collide_path.rs` (new): `link_dir` makes `<run folder>/bin/collide`, a link to the `collide` the
  daemon runs from (replacing one an earlier attempt left); `path_with_first` puts that folder first on `PATH`.
  `host/src/file_run.rs`: `FileRunConfig::collide_exe` (set from `shim_exe` in `host/src/daemon.rs`) and
  `FileRuns::put_collide_first`, called in `start_attempt` after the base environment is shaped, so a first attempt
  and a resume both get it. `COLLIDE_RUNTIME` is not touched.
- `host/src/builtin_roles.rs`: `BUILDER_DEFINITION` has a paragraph that names `collide task create` for follow-ups,
  says the item is private, says `collide task new` is no follow-up, and points at `collide guide`.
- `docs/own-terminal.md` (new): `collide` on `PATH`, the one CLAUDE.md line, what an agent may do, what stays the
  person's, and the runs Collide starts.
- Tests. `host/src/guide.rs` unit tests (8); `host/tests/guide_command.rs` (5, through the binary under `env_clear`,
  temp `HOME` and daemon root; the root is empty afterwards); `host/tests/collide_path.rs` (6);
  `host/tests/adapters/claude/launch.rs`:
  `the_line_allows_collide_commands_and_nothing_else_beside_the_disallowed_tools` and
  `the_collide_rule_is_on_the_line_of_a_role_that_allows_every_disallowed_tool`;
  `host/tests/adapters/claude/file_run.rs`:
  `a_run_has_the_collide_folder_first_on_its_path_and_the_allow_rule_on_its_line` (fake adapter, `dump_launch`);
  `host/src/builtin_roles.rs` (4 new). Updated only where the rule appears:
  `instructions.rs` `none_is_the_line_of_ts_0104_gx_and_the_collide_rule_word_for_word` (renamed, two words added),
  `command_tree.rs` (`TREE`, and `guide` in `NOT_OWNER_METHODS`: no owner method serves it), `commands.rs` unit tests.
- Seen red at runtime, against a stub that compiled (`text()` empty, `link_dir` and `path_with_first` returning the old
  behavior, the rule not pushed, no paragraph, `guide` not in `COMMANDS`): in `guide.rs` the summaries, forms, exit
  codes and person-only ops tests; in `builtin_roles.rs` the two paragraph tests; `commands::the_tree_lists_...`;
  `command_tree` `help_lists_every_command_of_the_tree_with_a_line_of_its_own` and
  `an_unknown_command_is_still_a_usage_error_that_lists_the_whole_tree`; in `guide_command.rs` the prints, the
  starts-no-daemon, the takes-no-argument and the help tests; all six in `collide_path.rs`; the four claude-adapter
  tests above. `guide_holds_no_path_...` was green on the stub (empty equals empty); I then added its
  `!stdout.is_empty()` guard and did not rerun it against the stub. Mutation: renaming `new` in the `task` parser
  turned `every_form_of_task_the_guide_names_...` red; restored.

### Decisions for the owner

1. **A folder of its own per run, holding one link.** `<run>/bin/collide` links to the `collide` beside the daemon.
   Recommend this: the agent gets `collide` and nothing else from the daemon's folder. Alternative: put the folder of
   the daemon's binary first on `PATH`; no file is written, but an installed folder such as `/usr/local/bin` would then
   shadow the user's own tools for every run.
2. **The allow rule is on every Claude line**, whatever the role, after `--disallowed-tools`. Alternative: a role key
   that turns it on; more to configure, and a role that forgot it could not file follow-ups.
3. **The guide gives forms for `task` only.** `run` and `tasks` are a summary each, from `COMMANDS`. Alternative: forms
   for `tasks` too, each read by `parse_tasks` in a test. Cheap to add when an agent needs them.
4. **The guide prints exit codes 10, 11 and 12, and a pointer to `--help` for the rest.** Alternative: print every
   code. Recommend the three: they are what the plan names, and they are the codes an agent must not mistake for a
   failure.
5. **`guide` is in `COMMANDS`**, so `--help` lists it; no owner method serves it. Alternative: hidden, as `shim` is.
6. **`collide guide <word>` is exit 2**, not ignored.
7. **The guide says an item an agent creates in a run is private.** It rests on `ts-0139-wq`. Alternative: leave it
   out until the app shows the follow-ups.
8. **No `collide` link when the daemon cannot name its binary** (`collide_exe` is `None`); that daemon cannot start a
   phase either (`NoShimExe`), so the case is only a test one.

Plan text to propose:
- 0.2, the socket token bullet: "every run has `collide` first on its `PATH`, in a folder of the run's own that holds a
  link to the binary the daemon runs from".
- Stage 4c, Your own terminal: "exit codes 10, 11 and 12 (the rest are in `collide --help`)", and "`collide guide` is a
  command of the tree".

### Not done, by design

- The planning workflow's prompt gets the same follow-up paragraph in Stage 9d.
- Whether real Claude honours `Bash(collide:*)` beside `--permission-prompt-tool stdio`: `ts-0138-pn`.
- An ACP agent in a file run gets the same `PATH` folder when ACP file runs come (Stage 6b); an ACP agent's permission
  requests are answered as before.
- `ts-0141-sn`, `ts-0142-ua`, `ts-0144-eh` to `ts-0150-dj` and `ts-0151-fk` are not touched, and none gets trivial.

### At review

- The allow rule opens every `collide` command to a run's agent; the daemon is the wall. Checked: it refuses a run's
  agent `run.start` and `run.start_file`, `queue.answer`, `board.init`, `board.trust`, `config.set` and the
  `config.pack_*` writes, `run.stop` of another run, the person-only ops and `move`. `collide pack` goes through the
  daemon. `collide guide` needs none of it.
- Added to the guide: inside a run, `collide run` is refused. The docs page says what a run's agent may not do.
- Plan 0.2 now names the per-run `bin/` link and the run-start refusal; Stage 4c says the guide points at `--help` for
  the other exit codes (the builder's two proposals).
- A quick run's task is a scratch task, so its agent's `create` gets no epic from the rule and must name one, or the
  plan pack refuses it. Not built here; the refusal names the missing epic.
- Mutations, each red then restored: no `collide` link on `PATH` (the fake-adapter run test), the allow rule dropped
  (four launch tests), the paragraph's `task create` renamed (the role test), the link folder last on `PATH` (the
  `collide_path` test and the run test), `collide guide <word>` accepted (the CLI test).
