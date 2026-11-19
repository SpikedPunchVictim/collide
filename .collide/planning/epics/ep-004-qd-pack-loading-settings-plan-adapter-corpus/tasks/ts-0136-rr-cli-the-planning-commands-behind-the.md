---
id: ts-0136-rr
kind: task
status: done
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

- Carried from `ts-0130-mv`: `renameSection` has no `id` and takes an optional `versions` map, but `write_op` in
  `host/src/task_daemon.rs` sends an id and a version for every op except `create`, so the provider refuses it through
  the daemon. This item gives it a CLI form and carries it through the daemon as `create` is carried; `addFeature` and
  `moveFeature` need CLI words too.
- Carried from `ts-0129-uv`: a bare-id write that runs without the daemon carries no `actor`. The plan provider reads
  none as a person, so until this item it refuses `accept` and `resolve` (no date) and does not wall a run's `reject` or
  `defer`. All writes going through the daemon here closes both; a test that a bare-id `accept` from the CLI waits as a
  Queue confirmation shows it.
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
- Owner decisions (2026-11-18, at the split): every task command through the daemon, no `--no-daemon` and no direct
  path. The builder's decisions 1 to 7, 9 and 11 as built, on the owner's delegation. At review: an unknown `--kind` had
  stopped failing when the direct path went (its test went with it); it is refused in `task.list` again and the CLI
  exits 2, with a daemon and a CLI test. Decision 10 (a source that never started leaves `collide tasks` empty and exit
  0) is `ts-0141-sn`, option A, before `ts-0138-pn`; decision 8 (the ts-0120 settle race) is `ts-0142-ua`. The builder's
  plan text (decision 9) is applied to 0.10 and Stage 4c. Mutations caught at review: `sections` counting a rejected
  proposal, `reid` listing its own item as rewritten, `epic=` resolved among every item, the `-undone` item never found,
  and the unknown-blocker stderr line.

### Built

- **Wire** (`core/src/owner/task.rs`, `core/src/owner.rs`, `core/src/schema.rs`): `TaskCreateParams`,
  `TaskReidParams` and `TaskReidResult { from, result?, queued?, rewritten }`; methods `task.create` (answers
  `TaskApplyResult`) and `task.reid`; schemas registered. `pnpm gen` wrote the TypeScript and JSON schemas under
  `packages/pack-sdk/src/generated/` and `schemas/`; `app/src/owner/methods.ts` lists the two methods;
  `core/tests/owner_protocol.rs` has a sample for each.
- **Daemon** (`host/src/daemon.rs`): dispatch and handlers `task_create`, `task_reid`, `task_sections`; the guarded
  write (`judge_write` and the busy-checkout queue) is one function, `judged_write`, shared by `task_apply`,
  `task_create` and `task_reid`. `task_sections` reads the corpus rows, keeps the proposals that are not dropped and
  calls `section_uses`, so it follows the layout and every checkout the board has.
- **CLI** (`host/src/task_cli.rs`): `USAGE` and `HELP` rewritten for the daemon-only commands; `CreateOptions`,
  `ReidOptions`, `RenameSectionOptions`; `parse_create`, `parse_reid`, `parse_seed`, `parse_rename_section`; the id
  word `renameSection` is read before op parsing; `ambiguous_message` lists `epic / key  kind  title`. The direct-path
  functions and `wants_daemon` are gone. There is no `--no-daemon`.
- **Daemon client** (`host/src/task_daemon.rs`): every command goes through `run_with`. `key_of` resolves a bare id in
  the daemon (a key is its own; a bare id is a full id or a short form among the tasks the board lists);
  `resolve_epic` resolves `epic=` among the epics first; `settled_write`, `finish`, `confirmation_waits` (exit 12) and
  `wait_for_queued` are shared by `task <id> <op>`, `create`, `reid` and `renameSection`; `sections` prints
  `task.sections`. `tasks --ready` is the built-in view, with its cycle and unknown-blocker lines on stderr.
- **Text** (`host/src/task_view.rs`): `show_text` prints the heading `<epic> / <id>`, the fields the source sets,
  claims, coverage, part problems, links (an incoming link by its inverse label, a link to a feature names it, a
  missing one reads "(proposal or feature not found)"), features by section with their implementers, and the body.
- **Tests, by file.**
  - `host/tests/daemon_plan_write.rs` (11, through the built plan pack): `task_create_*` (applied; refused with no
    epic and nothing written; unknown board; queued then applied), `task_reid_*` (`from`, `result`, `rewritten`; bare
    id; unknown id; queued then applied), `task_sections_*` (counts, dropped proposals left out, none, layout root).
  - `host/tests/task_daemon_cli.rs` (65, in-process daemon): queued create and reid waited for through a live phase,
    `--no-wait` exit 11, reid text and no commit, `renameSection` (text, `--json`, refused), `addFeature` and
    `moveFeature` by a short id, `sections` text equal to `task.sections` and `--json` equal to the answer, rejected
    proposals left out, short ids and the ambiguous one (exit 3), the `spikes` view over a copy of Collide's own ten
    spike records, and the person-only and parse tests.
  - `host/tests/task_cli.rs` (process per call, `env_clear`, temp HOME and `COLLIDE_DAEMON_ROOT`, each call stops its
    own daemon): the ts-0020, ts-0049 and ts-0120 tests re-pointed at the daemon, the new listing, reid and show text,
    and the person-only exit 12 tests, including a bare id (`accept`) with no source named.
  - `host/tests/task_cli_plan.rs` (3): the layout root lists and creates in its folder; a private item is made, listed
    and shown while `git status` stays clean.
  - `host/tests/task_daemon_process.rs`: with no `collide-daemon` beside the binary every command exits 1, prints
    nothing, says "the daemon is not running and did not start", and leaves the repository untouched.
  - `host/tests/fixtures/task-cli-help.txt` follows `USAGE` and `HELP`; the tests that ran a stand-in provider
    through the CLI are deleted, because no command has a direct path now (the fixture stays: `provider_launch.rs`
    uses it).
- **Unknown kind refused again** (review follow-up). `task.list` with a `kind` that no source of the board declares is
  `INVALID_PARAMS` with "unknown kind `widget`; the board's sources declare: task, epic, proposal, spike, question"
  and `data { unknown_kind, declared }` (`task_list` in `host/src/daemon.rs`, `Corpus::declared_kinds` in
  `core/src/corpus.rs`). The CLI turns an error with `unknown_kind` in its data into a usage error (`rpc_failed` in
  `host/src/task_daemon.rs`), so it prints the message and exits 2 as before; any other `INVALID_PARAMS` is still 1.
  Tests: `an_unknown_kind_fails_listing_the_kinds_the_boards_sources_declare` (plan pack, process) and
  `task_list_with_a_kind_no_source_declares_is_refused_and_names_the_kinds_the_board_declares` plus
  `task_list_with_a_kind_a_source_declares_lists_only_that_kind` (wire, `daemon_plan_write.rs`).
- **Tests added for removed ones:** `reads_in_a_rebase_never_wait_and_print_what_they_always_print` and
  `a_write_in_a_rebase_waits_and_is_applied_when_the_rebase_is_aborted` in `task_daemon_cli.rs`.
- **Removed tests and what covers each now.**
  - `task_cli_wait.rs`, the parse tests stay.
    `a_write_in_a_rebase_says_why_once_waits_and_applies_after_the_rebase_ends` is
    `a_write_in_a_rebase_waits_and_is_applied_when_the_rebase_is_aborted` (also
    `a_write_waits_for_a_live_phase_and_reports_it_applied_when_the_phase_ends`);
    `no_wait_refuses_at_once_with_the_same_reason_and_writes_nothing` is
    `a_write_in_a_rebase_says_why_names_the_queue_item_and_exits_11_with_no_wait`;
    `a_wait_of_one_second_ends_with_a_refusal_that_names_the_cause` is
    `a_wait_that_runs_out_leaves_the_write_queued_and_exits_11` (the exit is now 11, the write stays queued);
    `create_and_reid_wait_too_and_refuse_with_no_wait` is the three `a_create_..`/`a_reid_in_a_busy_checkout_..`
    tests; `a_read_never_waits_for_a_rebase` is `reads_in_a_rebase_never_wait_and_print_what_they_always_print`;
    `a_write_in_a_checkout_that_is_not_busy_goes_at_once` is
    `a_write_to_a_free_checkout_is_applied_at_once_and_says_nothing_of_a_queue`.
  - `task_cli.rs`, re-pointed (same behaviour over the daemon, new name):
    `tasks_prints_each_items_epic_beside_its_id_..` is `tasks_lists_each_item_by_key_with_its_status_kind_and_title`;
    `task_shows_the_epic_beside_the_id_and_each_link_target_with_its_epic` is
    `task_shows_the_epic_beside_the_id_and_each_link_target_with_its_state`;
    `json_carries_the_epic_ready_flag_and_no_invalid_items` is
    `json_is_the_daemons_list_with_the_epic_and_the_state_of_each_item`;
    `a_cycle_and_an_unknown_blocker_are_reported_on_stderr` is `..._by_the_ready_view`;
    `help_prints_the_usage_the_exit_codes_and_where_the_provider_is_found` is
    `help_prints_the_usage_the_exit_codes_and_the_forms_that_go_through_the_daemon`;
    `an_unknown_kind_fails_listing_the_declared_kinds` is the new unknown-kind test above.
  - **Conflict, exit 4 (two tests): covered only below the CLI.** `a_conflict_prints_the_current_state_says_nothing_
    was_written_and_exits_4` and `a_conflict_with_json_prints_the_result` are now the `finish` unit tests in
    `host/src/task_daemon.rs` (`a_conflict_says_nothing_was_written_names_the_state_now_and_exits_4`,
    `a_conflict_under_json_prints_nothing_of_its_own_and_exits_4`), plus the daemon's own conflict answers in
    `daemon_sources.rs` and `scratch_tasks.rs`. No test drives a real CLI call to a conflict: through the daemon a CLI
    write reads the version and applies in one call, and a queued write whose file changed is refused (exit 5, tested),
    so the CLI meets a conflict only in a race between its read and the apply that I could not make deterministic.
    The JSON print of a conflict is not covered end to end.
  - **Direct path only, removed with it:** `a_provider_that_exits_is_reported_and_its_stderr_passes_through`,
    `an_answer_that_is_not_a_list_result_is_reported`, `an_error_reply_is_surfaced_with_its_message`,
    `notifications_from_the_provider_are_skipped` (the CLI no longer speaks to a provider).
  - **`a_missing_bundle_says_what_was_looked_for_and_where`: no longer true, no test. See Decisions 10.**
- **Seen red at runtime.**
  - `a_reid_in_a_busy_checkout_is_queued_and_waited_for_and_renames_the_file_when_the_phase_ends` failed against the
    first build: the waiter watched the old key, which is gone after a reid, and said "no longer queued and did not
    change" (exit 5). Fixed by waiting as a create does (see Decisions 4).
  - By mutation of `host/src/daemon.rs`, restored afterwards: the sections filter made to match no proposal failed
    `task_sections_counts_...`, `task_sections_follows_the_layout_the_repo_sets`, `sections_prints_what_...` and
    `sections_leave_out_...`; `task_create` ignoring `epic` failed `task_create_makes_the_item_...`,
    `task_create_in_a_busy_checkout_...` and `a_create_in_a_busy_checkout_...`. The same sections mutation also
    failed `rename_section_goes_through_the_daemon_...`, which ends by listing `sections`.
  - The re-pointed `task_cli.rs` tests failed against their old expectations while the CLI was being moved (the
    listing line, reid text, show text); I did not count them or mutation-check each one.
  - Not mutation-checked: the wire reid tests, the short-id test, `task_cli_plan.rs`, the lonely-binary test. They
    passed on their first run against the finished code.

### Decisions for the owner

1. **Exit 12 for a confirmation waiting** (10 view error and 11 queued stay). Alternative: reuse 11. Recommend as
   built: a person's answer is a different wait from a busy checkout.
2. **`task.reid` does not commit; the CLI's `--commit` commits the files it reports.** Alternative: the daemon commits.
   Recommend as built: the daemon never commits for a caller.
3. **`sections` reads the corpus** (layout, private root and every checkout), not one tree's files. Alternative: the
   old direct read. Recommend as built.
4. **A queued reid waits as a create does** (no version to watch, because the item has a new key afterwards): it is
   applied when the write leaves the Queue with no `-undone` item. Alternative: also watch for the old key to
   disappear. Recommend as built; a second watch is a later item if a dismissed reid must be told apart.
5. **A bare-id write for an item that exists only in another worktree exits 5, not 3** (the id resolves in the board,
   the write is then refused for that checkout). Alternative: exit 3. Recommend as built: the id exists.
6. **`resolve_epic` reads a short form among the epics first**, so `epic=2` is `ep-002-..` and never `ts-0002-..`; a
   non-epic id is a usage error (2) and an unknown one is exit 3.
7. **No `--no-daemon`, no direct path** (owner decision of 2026-11-18), so the stand-in provider tests are removed and
   the exit 4 and exit 5 formatting is covered by `finish` unit tests in `host/src/task_daemon.rs`.
8. **Known flake, not from this item:** `a_queued_write_whose_file_changed_while_it_waited_...` (ts-0120) failed once
   in a full parallel file run and passed when run alone. The person's edit changes the task's version, so `settle`
   can see "version changed" before the daemon records the `-undone` item (the item leaves the Queue at take, before
   it is applied). Alternative: check the `-undone` item again after a version change. Recommend a small item.
9. **Plan text to propose.** 0.10 CLI: every task command goes through the daemon and exits 1 when it cannot start;
   `task create`, `task reid`, `task renameSection`, `sections` and `tasks --ready`; exit 12; short ids resolve in the
   daemon; `task.create`, `task.reid` and `task.sections` in the owner method list.

10. **A missing or unreadable packs bundle is silent through the daemon (found while mapping the removed tests; not
    fixed).** Two facts, both checked with a test I then removed: (a) `COLLIDE_BUILTIN_PACKS` is not among the
    variables a CLI start forwards to the daemon (`FORWARDED_ENV` is the daemon root, HOME and SHELL), so setting it
    on the CLI does nothing once a daemon exists or is started; the daemon finds the bundle from its own environment
    and executable. (b) When the daemon's loader fails (override at a folder with no `plan/pack.yaml`), `collide
    tasks` printed nothing on stdout and stderr and exited 0: a source that never started is only written to the
    daemon log (`failed_to_start` in `corpus_service.rs`), and the listing is empty. `task <id>` and `board.open`
    do name the problem. Options: (A) the list result names a source that could not start, and the CLI prints it on
    stderr (wire change, `stale_sources` today covers only sources that declared once); (B) the CLI exits 1 when the
    list is empty and the board's line problems are not; (C) leave it, the log has the text. Recommend A in its own
    item; B hides the case of an honestly empty board. Also decide whether to forward `COLLIDE_BUILTIN_PACKS`.
11. **Scratch declares no kinds.** `declared_kinds` is the union over the sources, so scratch adds none; the refusal
    applies only when that union is not empty, as the direct path did, so a board with a source that declared
    nothing accepts any `--kind` and lists nothing.

### Not done, by design

- No `--no-daemon` flag and no direct read or write path.
- `task.reid` and `task.create` do not commit; the CLI's `--commit` is the only commit.
- Nothing in the app calls the three new methods yet (Stage 6 and later).
- Provider-side `renameSection` over a private root is unchanged: it writes the shared and the private roots as the
  provider already did.
