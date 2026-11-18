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

### Built

Wire (additive, so the CLI can print what the daemon knows):
- `core/src/owner/task.rs` `TaskGetResult` gained `links` (the `HookLink` list, one hop out in both directions) and
  `ops` (the ops the task's source declares for its kind, left out when empty). `core/src/corpus.rs` `Corpus::get` fills
  `links` from `links_of`. `host/src/daemon.rs` `task_get` fills `ops` through the new `ops_of`; `empty_get` has both.
  `pnpm gen` was run: `schemas/TaskGetResult.*.json` and `packages/pack-sdk/src/generated/TaskGetResult.*.ts`.
- `app/test/support/loop-world.ts`: the `task.get` double returns `links: []` (the generated type now requires it).
- `host/src/task_args.rs` `declared_ops` moved here from `task_cli.rs` (the daemon and the CLI share it).
- `host/src/launch.rs`: `ensure_running_from_cli_kind` returns the `StartKind`; `ensure_running_from_cli` calls it.
  The CLI needs the kind to say once that it started the daemon.
- `host/src/daemon.rs` `task_apply`: the refusal of `commit` now points to `collide task <key> <op> --commit`.

CLI:
- `host/src/task_cli.rs`: `TasksOptions.view` and `.checkout` (`--view`, `--checkout`, each also as `--opt=value`;
  a missing value or a second one is a usage error naming the option, through `set_once`); `Command::New` and
  `parse_new` (`task new <prompt> [--json]`, one non-blank argument; `--checkout`, `--commit`, `--wait` and `--no-wait`
  are refused by name); `checkout_named` (the `git worktree list` rule, shared); `exit::VIEW_FAILED = 10` and
  `exit::QUEUED = 11`; `USAGE` and `HELP` list every new option and the two codes; `run` hands a command that
  `task_daemon::wants_daemon` accepts to the daemon path.
- `host/src/task_daemon.rs` (new): `wants_daemon`, `run`, `run_with`. `task new` is `task.apply` of the scratch source's
  `create`; it prints the key (`--json`: the provider's result). `tasks --view/--checkout` is `task.list` (settled:
  it polls while `refreshing`), one line per member (key, status, kind, title), `--ready` and `--kind` on top; a view
  with an error prints it on stderr and no member, exit 10; tasks the view could not check are counted on stderr,
  exit 0. `task <id> --json` is `task.get` with the `ops` key left out; a short id is resolved over `task.list`
  (`short_id::resolve`), an ambiguous one or a key at two paths prints the keys and each location, exit 3.
  `task <source>:<id> <op>` is
  `task.apply` with the version; `queued` prints the reason and the queue item's id, then `wait_for_queued` retries
  with `queue.answer` every 500 ms for the wait (`--wait`, 10 minutes by default), then `settle` looks for the task's
  version to change, and reports applied (0), not applied (5, from the `-undone` item, or when the version did not
  change) or still queued (11); `--no-wait` returns 11 at once. When no daemon answers, the
  command starts it, says "the daemon was not running; started it" once on stderr, or exits 1 with "the daemon is not
  running and did not start: <why>" (the direct commands never need it).
- `host/src/lib.rs` has the module.

Tests (all new tests were written before the code; the list says which went red at runtime):
- `host/tests/task_daemon_cli.rs` (in-process daemon, plan bundle, fixture repo): parser tests
  (`tasks_takes_a_view_and_a_checkout_in_both_forms`, `an_option_with_no_value_or_given_twice_is_refused_and_named`,
  `task_new_*`, `only_the_forms_that_need_the_corpus_go_through_the_daemon`); `task new` in two boards; `--view` by
  name against `task.list`; a broken view; `--checkout` with a worktree and with paths that are not one; `--ready` and
  `--kind` on a view; `task <id> --json` with links in and out, a collision (a worktree with the file moved), a short
  id with two matches, and a latest run put in through the store; a write in a rebase (`--no-wait`, `--wait 1`), with
  a live phase that ends (the version changed), one that a person dismisses, and with the file edited while it
  waited. Three tests moved here from `task_cli.rs` (the
  epic as the `part-of` link, the features and their problems, a proposal with no features part). Red at runtime
  with `wants_daemon` stubbed to false and `run_with` stubbed to the direct path: 31 of 38 failed (all but the parser
  tests and the no-features test, whose assertion also holds without the code).
- `host/tests/task_daemon_process.rs` (the real binary, `env_clear`, temp home and daemon root, daemon stopped by
  `common::DaemonStop`): the daemon started once and the key printed; a second command says nothing; `--json`; no
  `collide-daemon` beside `collide` (exit 1, the direct `tasks` still works); the option errors; help and usage against
  `host/tests/fixtures/task-cli-help.txt`. Red under the same stub: the four tests that start or need the daemon.
  The help and parser tests were not seen red under that stub; their text changed in the same step, and
  `help_keeps_the_text_of_the_built_commands` and `help_prints_the_usage_the_exit_codes_and_where_the_provider_is_found`
  failed on the old text before it was updated.
- `host/tests/common/mod.rs`: `DaemonStop`. `host/tests/command_tree.rs` and `host/tests/task_cli.rs`: the `tasks`
  usage line updated; `task_json_carries_the_epic` and `task_json_lists_the_features_and_the_problems` removed (moved,
  see above); the proposal test keeps its text half.

Exit codes added: 10 (the view has an error of its own), 11 (the write is queued in the daemon, not applied yet).

### Decisions for the owner

- **Which forms use the daemon.** `task new`, `tasks --view`, `tasks --checkout`, `task <id> --json` (any id), and
  `task <source>:<id> <op>` (a key with a colon, such as `scratch:3` or `plan:ts-0002-bb`). A bare id with an op
  (`task 3 setStatus done`), `task <id>` text, `tasks`, `task create`, `task reid` and `sections` stay direct. The
  alternative is that a bare id with an op also goes through the daemon; that makes `task 3 setStatus` need a daemon.
  Recommend the split above until 4c.
- **A bare id means a plan item, except that an exact id wins.** `task 3 --json` resolves over `task.list` in the CLI,
  because `task.get` reads a bare id as a scratch id. Alternative: `task.get` resolves short ids itself (a daemon
  change, with 4c's `task <id>` text). Recommend that in 4c.
- **The daemon's `task.get` carries `links` and `ops`.** Not in the item's Area (`core`, `daemon.rs`, generated files
  changed). Needed: the item's `--json` has the links and the CLI needs the op list to map words. Alternative: the CLI
  would call `task.list` and the source itself. Recommend keeping it.
- **`--commit` on a write through the daemon** is done by the CLI after an `ok`, as on the direct path; the daemon
  still refuses `commit`. A queued write ignores `--commit` (stderr says so). Alternative: the daemon commits. That is
  the `record` hook's job (Stage 8).
- **How the wait learns the outcome (changed at review).** `queue.answer` Retry every 500 ms until the item is gone
  (a Retry on an item that is gone counts as gone). Then `settle` polls every 250 ms for up to 10 s: a `-undone`
  Conflict item for the task, newer than the queued item, is not applied (exit 5); a `task.get` version that differs
  from the one the write was sent at is applied (exit 0); the deadline with neither is exit 5 with "the queued write
  is no longer queued and <key> did not change: it was dismissed in the Queue, or it changed nothing". Applied is
  never said without seeing the version change, so a Dismiss, or another drain that took the item, cannot give a
  false 0. `--json` says `not_applied` for a dismissal. Alternative: a daemon call that reports a queued write's
  outcome (4c).
- **Exit 10 only for a view's own error.** Per-task errors of a view (invalid items, over-budget) print a count on
  stderr and exit 0. Alternative: exit 10 for those too. Recommend as built: the list is still usable.
- **`task new` prints the key alone** (one argument for the prompt; more words are a usage error that says "put it in
  quotes"). Alternative: join the words. Recommend as built: the plain form cannot lose a word to a flag.
- **`--kind` is not checked on the daemon path.** An unknown kind lists nothing. Same as the direct path.
- **The scratch tasks are not in `--checkout`** (they have no checkout; this is core's rule). They show with `--view`.
- **The listing text** is `key  status  kind  title`, with `-` for no kind. Alternative: the direct path's columns
  (id, epic, ready flag). Recommend as built: a view mixes sources, which have no epic.
- **Plan text proposal** (Stage 4b CLI bullet): "`collide task new`, `tasks --view`, `tasks --checkout` and `task <id>
  --json` go through the daemon, started on use (it says so once; if it cannot start the command exits 1 and the direct
  commands go on). Exit 10: a view with an error of its own. Exit 11: a write queued behind a busy checkout."

- Owner decisions (2026-11-18, at the build): all taken as recommended, on the owner's delegation. One was changed at
  review: a queued write is reported applied only once the task's version has changed, so a dismissal or a write that
  changed nothing exits 5 with the reason, never 0. The plan text proposed here is in the Stage 4b CLI bullet. `task
  <id> --json` now has the daemon's shape (no `epic` or `ready` keys; the epic is the `part-of` link), as the owner's
  2026-11-17 choice to serve it from the daemon implies.

### Not done, by design

- `tasks`, `task <id>` text, `task create`, `task reid` and `sections` through the daemon: Stage 4c.
- `task.sections` and the short-id resolution in `task.get`: Stage 4c.
- The owner check of the CLI forms: `ts-0121-xp`.
- A commit of a write the daemon applied from the queue: Stage 8 (`record`).
