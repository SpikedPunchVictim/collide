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
