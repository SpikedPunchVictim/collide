---
id: ts-0150-dj
kind: task
status: done
title: "Orchard check: the minor differences"
owners: []
links: []
---

## Goal

The small differences the agent run of `ts-0121-xp` found (2026-11-19). Each is fixed, or dropped with a reason in
Built.

## Success criteria

- The Orchard plan provider declares no kinds, so `--kind widget` exits 0 with an empty list, and the text of `task
  <key>` has a blank `kind` line where the listing shows `-`. It declares `task`, and the text prints `-`.
- The fake's illegal-move message has other words and one line; the real CLI's names the transition and the legal moves
  on two lines. Both exit 2 with nothing on standard output. The fake follows the real shape, in made-up words.
- The first listing right after a re-trust warns "source `plan` may be out of date: the line ... is not trusted". It
  does not.
- `collide run` printed no `claude published:` line in a temp `HOME` (`published_chip` in `host/src/run_cli.rs`:
  `agent.published` was `None`). Find why, and fix it or say so in the run's text.
- After a daemon restart the first `collide tasks --checkout` of 1,659 tasks took 5.5 s, as long as the cold one (5.7
  s); later ones 0.5 s. Find what the first listing waits for while the store's cache (`ts-0116-wh`) is there.
- A worktree Collide makes has no build of a repo's own tools when they are not committed: the Orchard plan CLI's
  packages and `dist/`. A write queued into a run's worktree then fails (`./plan exited with 1`, a node error). Until
  setup hooks (Stage 7), the failure's Queue item says the repo's plan CLI could not run there; Stage 7 gets a note.

## Tests

- One per fix, red at runtime first. The timing one is a measurement in Built, not a test.

## Notes

- No item for: the real `depends_on` block lists (the fixture has both shapes), the `ID-MAP.md` fourth column's name
  (the adapter reads the first two cells), and the extra folders under `arch/` (ignored).
- Owner decisions: none recorded yet. The recommendations stand until the owner answers.

### Built

Each part was checked against `HEAD` first. Part 1 and part 2 had partly changed since the item was written; the rest
were as described.

1. **Kinds.** `HEAD` already differs from the item: `ts-0137-pg` made the Orchard provider declare `adr`, `spike` and
   `question`, and `ts-0136-rr` refuses an unknown `--kind` (exit 2, `task.list` in `daemon.rs`). So `--kind widget`
   no longer exits 0 with an empty list; it exits 2 and names the declared kinds. The part left to build was the text.
   - `show_text` in `host/src/task_view.rs` prints `-` for an item with no kind, as `list_text` in
     `host/src/task_daemon.rs` does. The provider declares no `task` kind (see Decisions).
   - Tests in `host/src/task_view.rs`: `an_item_with_no_kind_shows_a_dash_as_the_listing_does` (red at runtime: the
     line was `kind` and a blank) and `an_item_with_a_kind_shows_it` (green before and after; it pins the rule).
2. **The fake's illegal move.** `fixtures/orchard-arch/plan`, `commandSet`: the refusal is now two lines in made-up
   words, `<file>: <from> -> <to> is not a legal step` then `legal steps from <from>: <list>` (`none, it is final`
   for a final status). Exit 2 and nothing on standard output were already true and stay. No real-CLI word was
   copied: only the shape the `ts-0121-xp` record describes.
   - Tests in `fixtures/arch-plan-tests/test/fake-plan.test.ts` and `write.test.ts`: the pair table, `refuses a move
     the real write would refuse, in the same way`, and the two `write.test.ts` setStatus refusals now expect the new
     words; `a refusal names the step and then the legal ones, on two lines, with nothing on standard output` is new.
     25 failed at runtime against the old fake, all pass now.
3. **The re-trust warning.** Cause, read in `host/src/corpus_service.rs`: a source that fails to start is not tried
   again until a wait that doubles with each failure (`failed_to_start`, `look_at`). `board.trust` called
   `BoardCorpus::wake`, which only ended the sleep between looks; the look then skipped the source because its wait
   had not run out, so the source stayed stale with the "not trusted" reason until it did.
   - `BoardCorpus::wake` also sets every failing source's `next` to now.
   - Test `a_retrust_lists_the_source_at_once_and_the_first_listing_after_it_is_not_stale` in
     `host/tests/daemon_corpus.rs`. It uses a 30 s recheck so the earned wait is 60 s, far past the 30 s deadline:
     the daemon is restarted over an untrusted line (the source is stale with the cached task), the line is trusted,
     and the first listing that is not refreshing must have no stale source. Red at runtime: timed out after 30 s.
4. **`published_chip`.** Cause: a daemon under `COLLIDE_DAEMON_ROOT` (a test's, a developer's, the check's) is given no
   source of published versions on purpose (`published_from_env`: it must reach no network), and `status` in
   `host/src/claude_versions.rs` gave `published: None` for that, the same as for `claude.check_published: false`. So
   it is not a bug in the CLI, and nothing was fetched.
   - `ClaudeVersions::status` now gives a `published` with no versions and the error `NO_SOURCE` ("this daemon does
     not look it up") when there is no source and the setting is not off. `collide run` prints `claude published: not
     known (this daemon does not look it up)`. A daemon with a source, and a setting that is off, are as before. No
     wire type changed, so no `pnpm gen`.
   - Tests (the old ones asserted "no source, no chip" and were changed to the new rule):
     - `with_no_source_nothing_is_fetched_and_the_chip_says_it_is_not_looked_up` (`published.rs`)
     - `a_daemon_given_no_source_makes_no_fetch_and_the_published_chip_says_so` (`agent_setup.rs`)
     - `with_no_published_source_the_published_line_says_the_daemon_does_not_look_it_up` (`run_command.rs`, also named
       in `run_env.rs`)
     - `a_run_through_a_daemon_the_command_starts_prints_the_version_and_the_record_and_exits_0` (`run_process.rs`, a
       real daemon process)
     - `the_status_names_the_tested_range` (`version.rs`): changed to the new rule; not seen red.
     - New, green before and after (it pins the setting): `with_no_source_and_check_published_false_the_chip_is_hidden`.
     The first four were seen red at runtime against the old `status`.
5. **The slow first listing after a restart.** Read first. Nothing waits on the cache. The daemon serves the cache at
   once with `refreshing: true` (`serve_cache`), but `Session::settled_list` in `host/src/task_daemon.rs` asks again
   until `refreshing` is false, and the corpus thread relists every source for every checkout before it clears it
   (`startup`). So `collide tasks` after a restart takes as long as the listing, which is as long as the cold one;
   the cache only helps a caller that reads the first answer (the app). Measured through
   `host/tests/daemon_corpus.rs` (1,600 task files of fixture data, temp roots, an added measurement test that is
   not kept): cold first listing 3.79 s; after a restart, three times: the first `task.list` answered in 69, 73 and
   65 ms with all 1,600 tasks and `refreshing` true, the settled listing came after 1.24, 1.36 and 1.37 s, and a
   later list took 63 ms. The restart is faster than the cold start here because the cold figure includes the
   provider's first start; on the real tree the listing itself dominates (5.7 s cold, 5.5 s restarted). No code
   changed: the choice is the CLI's, not a bug (see Decisions).
6. **The worktree failure of the plan CLI.** `execute` in `fixtures/packs/orchard/providers/plan/write.ts`: when the CLI
   fails and its standard error holds node's own "Cannot find module" or "Cannot find package" line
   (`missingModule`), the error reply is `./plan could not run in this checkout: its packages or its build are
   missing here (<that line>)`. The Queue item is `A <op> of <key> was not applied: ` and then this text
   (`give_up` in `host/src/write_guard.rs`), so it needed no change.
   - Tests in `fixtures/arch-plan-tests/test/write.test.ts`: `exit 1 over node not finding a module of its own says
     the plan CLI could not run here` and `... a package ...`. Red at runtime: the reply was `./plan exited with 1:
     node:internal/modules/...`. The existing `Internal error` test is the control that the old text stays for other
     failures.

Runtime-red notes: the daemon and CLI tests above were run against the code before each change (the new assertions
failed for the reasons given); for the `run_command` and `run_process` tests the change was stubbed back
(`not_looked_up = false`) to see them fail, then restored.

### Decisions for the owner

1. **Kinds: the text prints `-`, the provider keeps no `task` kind.** Alternative: the Orchard provider declares a
   `task` kind, so the listing and the text both print `task`. Recommendation: what I built. An Orchard task has no
   kind of its own, and the text now agrees with the listing in Collide's code, for any source whose items have none.
   A cost of it: on a board whose only source is Orchard's, `--kind task` is refused (exit 2) because the declared
   kinds are `adr`, `spike` and `question`, though its tasks list with `-`. The alternative would make `--kind task`
   work there. If the owner wants that, it is a one-line kind declaration plus the status table it needs; it is not
   built here. The first success criterion of this item (`--kind widget` exits 0) was written before `ts-0136-rr`;
   the refusal is the rule now.
2. **The fake's refusal words.** Made up: `<file>: <from> -> <to> is not a legal step` and `legal steps from <from>:
   <list>`. Alternative: none that keeps the rule that no real word enters the repo. Recommendation: keep.
3. **The re-trust fix is in `wake`, not in the failure counter.** Alternative: reset the counter for a source whenever
   the line's hash changes. Recommendation: `wake` (it is the one call a trust makes, and a source that is still
   broken after the trust fails again and waits again).
4. **The published line is the daemon's, not the CLI's guess.** Alternative: the CLI reads `COLLIDE_DAEMON_ROOT` and
   prints the line when `published` is empty. That needs no change in the daemon's status, but it infers what the
   daemon knows. Recommendation: what I built. The cost is that the app's published chip would show "unknown" with a
   "last check failed" tooltip for a daemon with no source; only a test's or a developer's daemon has none.
5. **The slow first listing: no fix.** The cause is clear but the fix is a choice about what `collide tasks` prints.
   Proposed follow-up item, "CLI: the first listing after a restart prints the cache at once": `collide tasks` and
   `collide task <id>` print the cached list when the first answer has `refreshing: true` and the cache is not empty,
   with one line on standard error ("listing again; this is the last run's list"), and `--wait` keeps today's
   behaviour (wait for the listing to settle). Pros: 0.07 s against 5.5 s on 1,659 tasks, and the app already behaves
   so. Cons: a script reading the first output gets a list that may be stale, so the default for scripts must be
   decided by the owner; `--json` would need a `refreshing` key it already has. Alternative: leave it as it is (a
   wrong list is worse than a slow one for a script) and have the stderr line say "listing, 5 s" while it waits.
6. **The worktree failure is worded in the provider, not in `write_guard.rs`.** Alternative: the host names it for
   any provider whose error mentions a missing module. Recommendation: the provider, since only it knows the repo's
   plan CLI; the host stays generic. Limit: it matches node's own "Cannot find module" and "Cannot find package"
   lines. If the real CLI prints other words when its `dist/` is missing, the old text stays for that case; the
   check's record does not give them.

Proposed plan text for Stage 7 (setup hooks), as a note under its bullets: "A worktree Collide makes holds only what
is committed. A repo's own tools that are not (installed packages, a build folder) are missing there, so a write
queued into a run's worktree through such a tool fails with 'could not run in this checkout'. A setup hook (install
and build, run once per worktree before the first write) is what fixes it; until then the Queue item says so."

### Not done, by design

- The slow first listing after a restart: a follow-up item is proposed above (Decision 5); not filed.
- `--kind task` on an Orchard-only board: see Decision 1.
- `ts-0151-fk`, `ts-0152-jq`, `ts-0153-kp`: none became trivial or changed. `ts-0153-kp` (the fake held to recorded
  real shapes) now has a two-line refusal shape in the fake to hold to, without real words.

### At review

- Decisions 1 to 6 taken as recommended (owner delegation). Decision 1's cost stays open: on a board whose only source
  is Orchard's, `--kind task` is refused though its tasks list with `-`; on a mixed board `--kind task` lists only
  Collide's tasks. Not filed; raised with the owner.
- Decision 4's cost is accepted: the app's published chip on a daemon with no source reads "The last check failed: this
  daemon does not look it up". Only a test's or a developer's daemon has no source.
- Changed at review: `ClaudeVersions::status` read the `check_published` setting's file while the state lock was still
  held (the guard of `locked(..)` lives to the end of its statement). The source is now read into a local first.
- Decision 5 filed as `ts-0155-wb`, with the recommendation to keep waiting and explain the wait on standard error.
- Stage 7 note applied to the plan, as a bullet after the lifecycle steps.
- Mutations, each red then restored: `wake` not ending the wait (the re-trust test), the kind line blank (the
  `task_view` test), the no-source chip off (three adapter tests), `missingModule` not used (two write tests), the
  fake's refusal on one line (the fake-plan test).
