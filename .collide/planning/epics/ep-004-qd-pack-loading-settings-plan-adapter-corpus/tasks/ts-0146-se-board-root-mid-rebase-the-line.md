---
id: ts-0146-se
kind: task
status: ready
title: "Board root mid-rebase: the line is the branch git is rebasing"
owners: []
links: []
---

## Goal

During a `git rebase` at the board root `HEAD` is detached. `checkout_of` in `host/src/boards.rs` then names the line
`HEAD`, which is not trusted, so the source `plan` is not started and a write is refused at once (exit 1, "the line
`HEAD` is not trusted, so the source `plan` ... is not started") instead of waiting. The Stage 4b criterion "A write to
the board root during a rebase waits, and the CLI says why. It applies once the rebase ends" fails for a rebase. It
passes for a merge. Found by the agent run of `ts-0121-xp`'s checklist (2026-11-19), step 5b.

## Success criteria

- During a rebase at the root, the line is the branch being rebased (git's `rebase-merge/head-name` or
  `rebase-apply/head-name`); the source stays up and its trust is the line's.
- A write to the root then waits with "a rebase is in progress" and applies after `--abort` or `--continue`.
- What is read for the line during the rebase is the branch's own commit, not the rebase's `HEAD`.
- A detached `HEAD` with no rebase is as today.

## Tests

- Write first and see red at runtime: a daemon test that stops a rebase on a conflict at the root, then writes (today:
  refused), aborts, and sees the write applied.

## Notes

- Recommendation: read `head-name` in `checkout_of` when a rebase is in progress, and read the line's snapshot from the
  branch ref. A bisect detaches too (`BISECT_START` holds the branch); take it in the same change if it is the same few
  lines.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers.

### Built

- `host/src/boards.rs`: `Checkout` has a new field `held` and a method `line_branch()`. `checkout_of` fills `held` only
  when `HEAD` is detached, through the new `held_branch`. It asks `root_busy` (`host/src/checkout.rs`, the one
  detection of ts-0114-hr) what is in progress. For a rebase it reads `rebase-merge/head-name` or
  `rebase-apply/head-name`, for a bisect `BISECT_START`. The path of each file comes from `git rev-parse --git-path`, so
  a linked checkout reads its own folder. A name counts only when `refs/heads/<name>` is a commit in the repository;
  anything else (`detached HEAD`, a tag, a gone branch, a missing file, a bisect begun on a detached `HEAD`) leaves the
  line as `HEAD`, which is never trusted. `Checkout.branch` is unchanged (`HEAD` while detached), so `quick_run`
  still refuses a detached checkout and `accept` still refuses a busy root first. New `line_commit` gives the held
  branch's own commit, else the commit `HEAD` names.
- `host/src/board_lines.rs`: `for_sources` and `line_of` use `line_branch()` for the ref name and trust, and
  `line_commit` for the snapshot. The sources, the views, `board.list` and `board.open` all follow.
- Tests, all at runtime red against the old behavior (a stub that kept `held` empty) and green after:
  - `host/tests/board_held_line.rs`: `a_rebase_that_stopped_on_a_conflict_keeps_the_branch_being_rebased_as_the_line`,
    `the_line_commit_in_a_rebase_is_the_branchs_own_and_not_the_rebases_head`,
    `a_rebase_with_the_apply_backend_keeps_the_branch_too`,
    `a_linked_checkout_reads_its_own_rebase_and_the_main_one_stays_on_its_branch`,
    `a_bisect_keeps_the_branch_it_started_on_as_the_line`: all red.
    Green before and after, as guards: `the_line_is_the_branch_again_when_the_rebase_is_aborted`,
    `a_rebase_of_a_detached_head_has_no_branch_so_the_line_stays_head`,
    `a_head_name_that_names_no_branch_leaves_the_line_as_head`,
    `a_detached_head_that_is_in_no_operation_is_the_line_head`,
    `a_bisect_started_on_a_detached_head_leaves_the_line_as_head`.
  - `host/tests/daemon_sources.rs` (a repo pack, a real daemon, a real rebase stopped on a conflict):
    `in_a_rebase_of_an_untrusted_branch_the_source_does_not_start` (red: the old message named the line `HEAD`),
    `in_a_rebase_of_a_trusted_branch_the_source_starts_from_that_branchs_own_commit` (red: refused; `HEAD` holds the
    script of `main`, version two, and the source runs the branch's version one),
    `a_write_at_the_root_in_a_rebase_waits_and_applies_after_the_abort` (red: refused at once). The rig's `trust`
    became `trust_line`, with `trust` calling it for `main`.
  - `host/tests/board_lines.rs`: `a_rebase_at_the_root_leaves_the_line_the_branch_with_its_own_commit` (red: `HEAD`).
- The item's own test (a write waits and applies after `--abort`) could not be red with the built-in plan pack, which
  needs no trust: `a_write_during_a_rebase_is_queued_with_the_reason_and_applies_after_the_abort` in
  `write_guard.rs` already passed. The new one uses a repo pack, as the Orchard run did.

### Decisions for the owner

1. The bisect case is taken in the same change, as the item suggested: it is one more file name in `held_branch`.
   A bisect begun on a detached `HEAD` has a commit in `BISECT_START`, not a branch, so the line stays `HEAD`.
   Alternative: leave bisect out, so a bisect at a trusted repo pack's root would still refuse writes. Recommend as
   built.
2. `Checkout.branch` keeps meaning "what `HEAD` is on" and the held branch is a separate field. Alternative: let
   `checkout_of` return the held branch as `branch`. That would let `quick_run` start a run from a rebase, which it
   refuses today because `HEAD` is detached. Recommend as built.
3. A held name that is not a branch in the repository is not an error: the line is `HEAD`, as before. Alternative:
   fail `checkout_of`. A board listing would then lose the whole board for a damaged file. Recommend as built.
4. Plan text: 0.5, Root busy could say that during a rebase or a bisect the line is the branch git holds, with that
   branch's trust and its own commit. Proposed, not applied.

### Not done, by design

- `ts-0152-jq` (a listing says when the board's line cannot be read) is untouched. `line_of` still returns no line when
  `line_commit` finds none, which is where that item works. This change does not make it trivial; it is unchanged.
- `ts-0147-ve` to `ts-0151-fk` and `ts-0150-dj` are unchanged by this item.
- A merge, a cherry-pick or a revert at the root keep `HEAD` on the branch, so they needed nothing.

### At review

- Decisions 1 to 3 taken as recommended (owner delegation). Decision 4 applied to 0.5, Root busy, with the fallback
  named.
- Trust wall traced: `board.trust` and `board.trust_review` take the line by name, and the name a person sees is now the
  held branch, so they trust the branch at its own commit. A `head-name` that someone wrote by hand gets at most the
  branch's committed snapshot, never `HEAD`'s files.
- `held_branch` matches `root_busy`'s words (`a rebase`, `a bisect`). If those words change, the line falls back to
  `HEAD`, which is never trusted: it fails safe, and the held-line tests go red.
- Mutations, each red then restored: the line commit taken from `HEAD` (two daemon tests, two held-line tests), the
  trust of the branch rebased onto (all three daemon rebase tests), any held name accepted (two guards), the full ref
  kept (four held-line tests), the apply backend not read, the bisect not read.
