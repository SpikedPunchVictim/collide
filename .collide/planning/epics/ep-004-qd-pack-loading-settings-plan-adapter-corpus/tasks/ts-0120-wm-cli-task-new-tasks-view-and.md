---
id: ts-0120-wm
kind: task
status: ready
title: "CLI: task new, tasks --view and --checkout, task --json"
owners: []
links:
  - { type: blocked-by, to: ts-0119-tj }
---

## Goal

The CLI of the stage over the daemon's corpus: `collide task new <prompt>` for a scratch task, `collide tasks [--view
<name>] [--checkout <path>]`, and `collide task <id> --json` with links one hop out, the version and the latest run.

What exists. `host/src/task_cli.rs` parses and runs `tasks [--ready] [--kind] [--json]`, `task <id> [--json]`, `task
<id> <op> ...`, `task create`, `task reid` and `sections`, directly in the checkout against the built-in `collide/plan`
pack, with `--checkout` and `--commit` on writes (`ts-0020-tt`, `ts-0031-xu`). `collide run` and `collide config` go
through the daemon: `host/src/launch.rs` has `ensure_running_from_cli`, and `host/src/commands.rs` holds the command
table. After `ts-0113-ck` to `ts-0119-tj` the daemon serves `task.list` (with a view), `task.get`, `task.apply`,
`task.views` and the scratch source. `collide task new` does not exist.

What this adds.
- `collide task new <prompt> [--json]`: a scratch task for the board of the current repository, through the daemon
  (`task.apply` of the scratch source's `create`). It prints the key.
- `collide tasks --view <name> [--checkout <path>]`: the view's members through `task.list`, with the view's error count
  and a line for the view's own error. `--checkout` limits the list to the copies in one checkout. `--ready` and
  `--kind` apply on top.
- `collide task <id> --json` from the daemon: the item's fields, status and category, links one hop out in both
  directions (the `HookLink` shape of `ts-0115-ug`), the `version` that `task.apply` takes as `expected`, the latest
  run, the copies, and `ambiguous` for a collision.
- `collide task <id> <op>` on a source other than the built-in one: through the daemon's `task.apply`, so a write waits
  or queues as `ts-0114-hr` says, and the CLI says why.
- The help text and the exit codes of the new paths, in the style of the existing ones.

## Success criteria

- `collide task new "try the cache"` in a fixture repo prints a key; the task is in `collide tasks` for that board and
  in no other board's list (the stage's scratch criterion, through the CLI).
- `collide tasks --view <name>` prints exactly the members of `task.list` for that view, in the same order, and a view
  that has an error prints the error and no members, with a non-zero exit code of its own.
- `--checkout <path>` accepts only an entry of `git worktree list` (as `ts-0031-xu` does) and limits the list to the
  copies there.
- `collide task <id> --json` has the keys above, and the text form is unchanged. A task with a collision says it is
  ambiguous and lists each location, and exits 3.
- A write to a checkout that is busy (a rebase) or has a live phase prints why it waits or is queued and the queue
  item's id, and `--no-wait` returns at once with a code of its own (as `ts-0114-hr` defines).
- With no daemon running, the commands that need one start it as `collide run` does, and say so once. With the daemon
  impossible to start, they say so and exit non-zero, and the direct commands still work.
- The commands that are not changed (`tasks`, `task <id>`, `task create`, `task reid`, `sections`) behave as before, and
  the dogfood board's own use is untouched.
- Every new option is in `collide help` and its usage text, and the unknown-option errors name the option.

## Tests

Write these first. The CLI tests run the built `collide` binary, with the environment cleared (`env_clear()`) and `HOME`
and `COLLIDE_DAEMON_ROOT` set to a temp root, as `host/tests/common/mod.rs` `TempRoot` does, against a temp repository
with `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_SYSTEM` set to empty files.

- `task new` then `tasks` in two boards (two temp repositories).
- `tasks --view` by name; a broken view; `--checkout` with a worktree and with a path that is not one.
- `task <id> --json` for a task with links in and out, a collision, a latest run (a run row put in by the test through
  the store).
- A write during a rebase (a repository stopped in the middle of one) and with a live phase: the message and the code.
- The daemon started by the CLI is the temp one, and it is stopped by the test; no other daemon is touched.
- The old commands against the existing test fixtures, unchanged.
- The usage text and help against a stored copy, so a change is seen.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4b: Scratch source ("`collide task new <prompt>` creates one"); CLI ("`collide tasks [--view <name>]
    [--checkout <path>]`, `collide task <id> [--json]` (one item: fields, status, links one hop out, version and latest
    run; part K)"); Success Criteria: "A scratch task created with `collide task new` is listed for that board only."
  - Part K: the CLI. 0.10: Scratch source. 0.5: `tasks[].links`.
- **Owner's choice, with a recommendation: which path the commands take.** The corpus lives in the daemon, so `--view`,
  `task new` and the `--json` with links need it. Recommend the new forms go through the daemon (started on use) and the
  existing text commands stay on the direct path until Stage 4c, which moves the planning commands behind the daemon as
  the plan says. A person who only uses `collide tasks` as today gets no new process, and the dogfood board keeps
  working while this stage is built. Alternative: move every command behind the daemon now. That gives one answer
  everywhere, and the dogfood board then depends on a daemon for `collide tasks`, which is a risk in the middle of the
  stage. This sets whether the CLI needs a running daemon.
- **Decision for the item, with a recommendation: `--ready`.** The ready rule is the existing one (`core/src/ready.rs`),
  applied by the daemon for a view and by the CLI for the direct path. Alternative: a different rule for a view. No.
- **Decision for the item, with a recommendation: the board of a scratch task.** The board of the repository that holds
  the current directory (main checkout resolved as `host/src/checkout.rs` does), never a workspace's own id. Carried
  from `ts-0075-hj`: board-id reuse after a repository is removed and the main-checkout root belong to 6a.
- **Decision for the item: exit codes.** The existing table in `host/src/task_cli.rs` is extended with at most two new
  codes (a view with an error, and a write that was queued); the numbers are in the item's Built section.
- Out of scope: `collide init`, `collide config` and `collide pack` (built in 4a); moving the text commands behind the
  daemon and `task.sections` (4c); `collide record` (Stage 7).
- Area: `host/src/task_cli.rs`, `host/src/commands.rs`, `host/src/main.rs` (only the dispatch), `host/src/launch.rs` (a
  call only), `host/tests/`, the CLI's help text.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Settings files are fixtures, never the owner's own `settings.json`.
- Stage 4b item (revision 23 scope, split 2026-11-17). Items of Stage 4b run one at a time, in id order, because they
  share `core`, `host` and the generated files.
- Nothing here runs a real `claude`.
- Owner decisions (2026-11-17, at the split): the new forms (`task new`, `tasks --view`, `tasks --checkout`, `task
  --json`) go through the daemon; `tasks`, `task <id>` and `task create` stay direct until 4c. The rest: the
  recommendations above.
