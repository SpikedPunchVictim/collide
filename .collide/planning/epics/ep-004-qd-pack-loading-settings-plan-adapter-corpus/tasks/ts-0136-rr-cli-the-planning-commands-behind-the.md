---
id: ts-0136-rr
kind: task
status: ready
title: "CLI: the planning commands behind the daemon, task.sections and short ids"
owners: []
links:
  - { type: blocked-by, to: ts-0135-cx }
---

## Goal

Every `collide` task command goes through the daemon and the provider supervisor, and the planning commands show what
this stage added: the layout, the private root, the new kinds, the person-only answer and the link labels.

What exists. `host/src/task_cli.rs` runs `collide tasks`, `collide task <id>` and its writes, `collide task create`,
`reid` and `collide sections` directly: it reads the plan pack's files itself with the built-in provider's own library
and the empty config. `host/src/task_daemon.rs` `wants_daemon` sends only `task new`, `tasks --view` and `--checkout`,
`task <id> --json` and `task <source>:<id> <op>` through the daemon (`ts-0120-wm`). The short-id rule is
`core/src/short_id.rs` `resolve`; the ready rule is `core/src/ready.rs`. The daemon has `task.list`, `task.get`,
`task.apply`, `task.new` and `task.views`; no method lists sections, creates a plan item or re-ids one. The direct path
cannot read the `layout` setting or the private root, because both need the board's config.

What this adds.
- Daemon methods `task.create` (the provider's `create`, with the epic and kind), `task.reid` (the provider's `reid`,
  with the result's rewritten ids) and `task.sections` (the sections of the proposals' Features parts, the
  near-duplicate groups of `core/src/section.rs`, over the corpus). Each is a `TaskSectionsParams`-shaped request with a
  board, and is added to the generated types.
- `wants_daemon` is true for every task command. A bare or short id resolves in the daemon with `short_id::resolve`
  over the board's corpus (ambiguity lists the matches, with their sources). `tasks --ready` is the built-in view of
  `ts-0135-cx`, and `collide tasks` with no flag is the corpus's default listing.
- `collide task <id>` text and `--json` come from `task.get`: they show the item's `fields` that the provider sets
  (`needs`, `resolved_by`, claims, coverage and the part problems as counts), its links with the `inverse` label for an
  incoming link ("evidence for"), and `feature` on a link that names one.
- A person-only op from the CLI prints that a confirmation is waiting in the app and exits 12 (`ts-0128-bb`). A refusal
  from the provider prints its message and exits as `refused` does today.
- The direct path is removed from `host/src/task_cli.rs`; what remains there is the argument parser, the help text and
  the output formatting, shared with the daemon path. The `--help` text is updated for the new forms.
- A daemon that is not running is started as `ts-0120-wm` starts it for `task new`, with the same message and exit
  codes when it cannot be reached.

## Success criteria

- A board with a `layout` setting and a private root: `collide tasks` lists the items from the layout's folders and the
  private root; `collide task create` writes into the layout's folder; the same commands before this item (the direct
  path) did not know either.
- `collide task 12`, `12-ua`, `ts-12` and the full id give one answer; a short form that fits two gives the list and the
  exit code the direct path gave.
- `collide tasks --ready` equals the built-in view's members, and what the direct path listed on the old fixtures.
- `collide task reid <id>` prints the old and new id and each item whose links changed; no commit is made.
- `collide sections --json` equals what the direct path printed over the same proposals.
- `collide task <spike id>` shows the claims count and the part problems as a count; a proposal shows its coverage.
- `collide task <id> accept` on a proposal prints the confirmation text and exits 12; `--json` has the same fields.
- With no daemon and no way to start one, each command fails with the one message and the same exit code as `task new`,
  and none reads the files itself.
- `collide tasks --view spikes --json` over a copy of Collide's own `.collide/planning/` lists the ten spike records
  with their statuses and no item from inside a spike folder (the stage's first criterion, run over a fixture copy).

## Tests

Write these first.

- Daemon request and reply tests for `task.create`, `task.reid` and `task.sections`, with the real built-in provider
  over fixture trees in a temp root, in the style of `host/tests/daemon_corpus.rs`.
- The CLI tests of `ts-0020`, `ts-0049` and `ts-0120-wm` re-pointed at the daemon with a temp `COLLIDE_DAEMON_ROOT`;
  each one that passed before passes now with the same text and exit code, except the lines that changed on purpose.
- A test for each short form against a corpus with an ambiguity, and for the text of a task with each new field.
- A test that no command reads the planning files in the CLI process: the direct-path functions are gone from
  `task_cli.rs`, and a test over a repository whose planning folder has a layout the CLI could not guess lists it
  right only through the daemon.
- A copy of Collide's spike records in a temp repository for the first stage criterion.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4c: Planning ("The slice's CLI moves behind the provider supervisor"); Success Criteria (the spike view;
    `collide task reid`).
  - 0.10: Planning sources (the slice's CLI), Identity and `reid`. Stage 2 slice text. Part L.
  - Carried from `ts-0120-wm`: "`create`, `reid` and `sections` stay direct till 4c".
- **Owner's choice, with a recommendation: no direct path after this item.** The plan says the CLI moves behind the
  supervisor. Recommend removing the direct read and write, so there is one code path and the layout, the private root,
  the trust gate and the write guards apply to every command. The cost is that `collide tasks` in a fresh clone starts
  a daemon, and needs a trusted line for a pack that is not built in. Alternative: keep a read-only direct path for the
  built-in `collide/plan` with an explicit flag (`--no-daemon`), for a CI job or a machine with no daemon. It is a
  second implementation to keep equal to the first. This sets what `collide tasks` needs to work.
- **Decision for the item, with a recommendation: exit codes.** Keep the codes `ts-0120-wm` set (10 view error, 11
  queued) and add 12 for a confirmation waiting. Alternative: reuse 11 (a write that is waiting). One code is simpler,
  but "waiting for a rebase" and "waiting for you in the app" need different words and different next steps.
- **Decision for the item: `task.reid` does not commit.** The plan's `reid` rewrites files and leaves the commit to the
  person, as the slice did.
- **Decision for the item: `sections` over the corpus, not the files.** It reads the proposals' `features` from the
  corpus's tasks, so it follows the layout and covers every checkout the board has; the direct one read one tree.
- Area: `core/src/owner/task.rs`, `host/src/daemon.rs`, `host/src/task_cli.rs`, `host/src/task_daemon.rs`,
  `host/src/section_view.rs`, `core/src/short_id.rs`, `core/src/ready.rs`, the generated schemas and TypeScript,
  `host/tests/`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test that runs the `collide` binary
  clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as
  `host/tests/common/mod.rs` `TempRoot` does. Settings files are fixtures, never the owner's own `settings.json`. A temp
  repository sets `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_SYSTEM` to empty files. Never run `collide task <id> --json` or
  `task new` by hand: they start the real daemon.
- Stage 4c item (revision 23 scope, split 2026-11-18). Items of Stage 4c run one at a time, in id order, because they
  share `core`, `host`, `packs/plan` and the generated files.
- Nothing here runs a real `claude`. No dependency is added.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers at the split.
