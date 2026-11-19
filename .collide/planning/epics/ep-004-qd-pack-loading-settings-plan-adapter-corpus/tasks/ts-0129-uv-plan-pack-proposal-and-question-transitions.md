---
id: ts-0129-uv
kind: task
status: done
title: "Plan pack: proposal and question transitions, by: and resolve"
owners: []
links:
  - { type: blocked-by, to: ts-0128-bb }
---

## Goal

A proposal moves only along the moves 0.10 allows. Accepting writes `by: human:<date>`, a question is resolved by an op
that records who and what, and the provider refuses what an agent may not do.

What exists. A proposal has six statuses (`PROPOSAL_STATUSES` in `packs/plan/src/item.ts`), and `setStatus` moves it to
any of them, refusing only `proposed` while the Features part has a problem (`proposedRefusal` in `write.ts`). There
is no transition table, no `by:`, no `defer` trigger and no `resolve`. From `ts-0127-js` a question exists with the
`resolved_by` rule on read, and `includes` links. From `ts-0128-bb` the provider is told the actor.

What this adds.
- A transition table for a proposal, in `packs/plan/src/transitions.ts`. Recommended: `draft` to `proposed` or
  `rejected`; `proposed` to `accepted`, `deferred`, `rejected` or back to `draft`; `deferred` to `proposed` or
  `rejected`; `accepted` to `superseded`; `rejected` to `draft` (reopened); `superseded` is final. A refused move
  names the status and the moves it has.
- Ops `accept`, `defer { trigger }`, `reject { reason? }` and `resolve { by?, text }`, declared for their kinds, beside
  `setStatus`, which stays and applies the same table and the same person-only checks (as `person_only_request` reads
  them).
- `accept` writes `status: accepted` and `by: human:<date>`, the date from `actor.at`. It is refused while a question
  the proposal `includes` is `open`, naming each. `defer` writes the trigger as `trigger:` and refuses an empty one.
- `resolve` on a question writes `status: resolved` and `resolved_by`: `human:<date>`, or the id of the item named in
  `by`. `text` goes in the question's `## Decision` section, which is made at the end of the body when it is missing.
- The provider refuses `accept`, `defer`, `reject` and `resolve` (and `setStatus` to those words) when `actor.kind` is
  `agent`, in its own words. This is the second wall; the daemon's is `ts-0128-bb`.
- A file that is `accepted` with no `by:` reads as before: it lists, and nothing is invalid.

## Success criteria

- Each legal move works and each other pair is refused naming the moves it has.
- `accept` writes `by: human:<date>` with the date of `actor.at`, and changes no other line of the frontmatter.
- `accept` while an included question is open is refused naming it, and works once the question is `resolved` or
  `dropped`.
- `defer` with a trigger writes it; with none it is refused. `deferred` back to `proposed` works.
- `resolve` writes `resolved_by` and the `## Decision` text, and the question then reads without a `task.invalid`.
- With `actor.kind: agent`, every person-only op is refused by the provider and the file is untouched.
- `setStatus accepted` and `accept` give the same file.

## Tests

Write these first.

- Provider tests over fixture proposals and questions: the full table (every pair), `by:` with a fixed `at`, the open
  question refusal, `defer`, `resolve` with and without `by`, an agent actor on each op, a file hand-edited to
  `accepted` with no `by:`.
- The ops' declarations in `initialize`, and their argument schemas.
- A daemon test that drives `accept` from the app's connection and from the CLI (the `ts-0128-bb` shapes) over the real
  built-in provider, in a temp repository.

## Notes

- Carried from `ts-0127-js`: a question's `resolved_by` may name an item id. `reid` does not rewrite it, so a question
  resolved by an item that is then re-ided names an id that no longer exists. `resolve` writes the field, so this item
  makes `reid` rewrite it too.
- IMPLEMENTATION_PLAN.md:
  - Stage 4c: Feature ops bullet ("The `proposal` kind gets its transitions, the person-only rule and `by:` (0.10),
    with the actor"). Stage 2's slice text: "The transitions, the person-only rule and `by:` wait for Stage 4c".
  - 0.10: Planning sources (the proposal's statuses; "Only a person accepts, defers, rejects or resolves ...
    `collide/plan` refuses those ops from a run ... and refuses `accepted` while a question the proposal includes is
    open"; "A question holds its own decision once it is resolved").
- **Decision for the item, with a recommendation: the transition table.** 0.10 gives the statuses and a few rules, not
  the edges. Recommend the table above. Alternative: any move among the six, as now, with only the rules above. Simpler,
  and a typo could take an `accepted` proposal back to `draft`. The edges are in one table, so a change is one line.
- **Decision for the item: the phase-end check is not here.** 0.10 says an agent phase that writes a `status:` or `by:`
  line itself fails at the phase end, by hashing the planning files that hold those lines before and after. That is
  `protectedPaths` at `phase-end`, which Stage 8 builds. Carried there; see the split proposal. Until then an agent that
  edits the file directly is not caught, which 0.10 already says is a mistake guard and not a wall.
- **Decision for the item: `reject` takes an optional reason**, written as a `reason:` line. Alternative: none.
- **Decision for the item: `resolve`'s `by`.** An item id is accepted only when it exists, as a link target does.
- Area: `packs/plan/src/transitions.ts` (new), `write.ts`, `item.ts`, `frontmatter-edit.ts`, `provider.ts`,
  `packs/plan/test/`, `host/tests/`.
- The test rule: no test touches the real home, real launchd or a real daemon. Each test that runs the `collide` binary
  clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as
  `host/tests/common/mod.rs` `TempRoot` does. Settings files are fixtures, never the owner's own `settings.json`. A temp
  repository sets `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_SYSTEM` to empty files.
- Stage 4c item (revision 23 scope, split 2026-11-18). Items of Stage 4c run one at a time, in id order, because they
  share `core`, `host`, `packs/plan` and the generated files.
- Nothing here runs a real `claude`. No dependency is added.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers at the split.

### Built

- Table: `PROPOSAL_MOVES` and `moveRefusal` in `packs/plan/src/transitions.ts`, the edges as the Goal gives them. A
  refused move says `pr-0001-aa is accepted and cannot move to draft; from accepted it can move to superseded`, or
  `superseded is final: it has no moves`.
- Second wall: `personOnlyWrite` and `agentRefusal` in the same file. `personOnlyWrite` reads the same writes as
  `person_only_request` in `host/src/auth.rs`: the four ops (proposal and question only), a proposal's `setStatus` to
  `accepted`, `deferred` or `rejected`, and `deferred` back to `proposed` (reopen). `writeLocked` in `write.ts` asks it
  after the item is found and before the version is compared, with `params.actor`. An agent gets
  `only a person may accept pr-0001-aa; this request came from the agent of run R-7, and a person does it in the app`
  and nothing is written.
- Ops: `packs/plan/src/person-ops.ts` (new). `proposalMove` is the one move that `setStatus`, `accept`, `defer` and
  `reject` share: same status changes nothing, then the table, then the Features check for `proposed`, then (for
  `accepted`) the open-question refusal and `by:`. `PERSON_OPS` holds `accept`, `defer`, `reject` and `resolve`;
  `withDecision` makes or fills the `## Decision` section. `write.ts` registers them beside `setStatus`
  (`setProposalStatus`); `Prepared` and `OpSpec` moved to `op-util.ts`, and `Prepared` gained an optional `body` edit
  that `writeLocked` applies with the file's own line ending and the body cap.
- Lines: `setFieldLine`, `replaceFieldValue` and `chain` in `frontmatter-edit.ts`. A new field line goes right after
  `status:`; one that is there is replaced in place. The result is re-parsed and must equal the expected frontmatter,
  as for every other op.
- Declarations: `provider.ts` declares `accept`, `defer { trigger }`, `reject { reason? }` on the proposal kind, and
  `resolve { by?, text }` on the question kind, with their argument schemas.
- Carried note: `reid.ts` rewrites a question's `resolved_by` when it holds the re-ided item's id (`resolvedBy`), in
  its quotes and with its comment kept; a `human:<date>` value is left alone.
- Tests. `packs/plan/test/transitions.test.ts` (100): declarations (3), the table and every pair of the six statuses
  (30 pairs, plus same-status and Features), accept (`by:` with a fixed `at`, same file as `setStatus accepted`, no or
  bad `actor.at`, open-question refusal naming each, hand-edited `accepted` with no `by:`), defer, reject, resolve (with
  and without `by`, `## Decision` made, filled, CRLF), reid of `resolved_by` (3), the agent wall on 12 proposal rows and
  the question ops, and a person with no actor. `host/tests/daemon_plan_person_ops.rs` (5): the real built-in provider
  through the daemon, `accept` from the app (date of the daemon's clock), `setStatus accepted` equal to `accept`,
  `defer`, `reject` and `resolve` from the app, `accept` from the CLI waiting for a confirmation and the app's accept
  writing `by:`, and all four ops from the CLI waiting with no file changed. `host/tests/common/plan_rig.rs` gained
  `rig_with`. Four existing tests changed meaning and were edited: `proposals.test.ts` (any move among the six),
  `provider.test.ts` and `questions.test.ts` (the declared ops), and `task_cli.rs`
  `a_proposals_set_status_takes_the_six_words_and_lists_them_when_it_is_wrong` (draft to `deferred` is no move now).
- Seen red at runtime, by running the new tests against the code before the change (`write.ts`, `reid.ts` and
  `provider.ts` as at `HEAD`): 82 of the 100 in `transitions.test.ts` failed, every other one is a control that is legal
  before and after (and the table-data test, which has no old behavior). Of the five daemon tests, four failed (the
  fifth, all four ops waiting, is the daemon's own wall and passes before). By mutation of the new code: the reopen
  line removed from `personOnlyWrite` (the deferred to proposed agent row), `reid` not looking at `resolved_by` (2),
  the open-question status test broken (2).

### Decisions for the owner

- **`setStatus` to `deferred` is refused**, naming `defer`. A deferred proposal holds its trigger (0.10), and
  `setStatus` has no trigger argument. The table still has the edge, so the message for `draft` to `deferred` is the
  table's. Alternative: allow it and write no trigger. Recommend the refusal.
- **No date, no `by:`.** `accept` and `resolve` without `by` take the day from `actor.at` and refuse when the actor has
  none or it is not `YYYY-MM-DD`. A call with no actor is read as a person (as the daemon's types say), so
  `reject` and `defer` work without one. Alternative: fall back to the provider's own local date. Recommend refusing:
  the daemon's clock is the one date, and a command that runs without the daemon has no business accepting.
- **Written lines.** `by`, `trigger`, `reason` and `resolved_by` go right after `status:`. A line that is already
  there is replaced where it stands. A stale `trigger:` or `reason:` stays when the proposal moves on (a rejected
  proposal reopened to draft keeps its `reason:`). Alternative: clear them when the status leaves. Recommend keeping
  them: they are history, and the table lets a person see why.
- **Same status is no op.** `defer` on a deferred proposal, or `accept` on an accepted one, changes nothing, as
  `setStatus` to its own status does; a new trigger is not written. Alternative: let `defer` update the trigger.
- **`resolve` is from `open` only.** A resolved question is refused (reopen by hand, as `setStatus` already says), and
  a dropped one is refused (set it to open first). `by` must match the id pattern, name an item that exists, and not
  be the question itself.
- **The Decision text is added, not replaced.** When `## Decision` already has text, the new text follows it after a
  blank line. Alternative: replace. Recommend adding: nothing a person wrote is lost. The section ends at the next `#`
  or `##` heading. The body's line ending is the one of the file's opening `---` line.
- **Limits.** A trigger and a reason are 1 to 200 characters, with no control character and no space at either end (as
  a claimant). The decision text is 1 to 10000 characters, a tab and line breaks allowed. Alternative: other numbers.
- **Declared on the kinds only.** `accept`, `defer` and `reject` are on `proposal`, `resolve` is on `question`; the
  provider's own `ops` list is unchanged, and none has `movesTo`. Alternative: `movesTo` of `done`, `backlog`,
  `dropped` and `done`, so `task.before_status` hooks fire for them. Recommend leaving that to the item that wires
  hooks to the planning kinds.
- **The wall comes before the version check**, so an agent with a stale version is refused, not given a conflict.
- **`KindDecl.transitions` is not filled.** The host reads none. Alternative: declare the edges there too, so the app
  can grey out a move. Recommend it when the app has a proposal page.
- **The daemon test copies `MiddayClocks` and `CallerTree` from `person_only.rs`.** Alternative: move both to
  `common/`. Recommend that when a third file needs them.
- **Plan text to add under 0.10, Planning sources:** the transition table (`draft` to `proposed` or `rejected`;
  `proposed` to `accepted`, `deferred`, `rejected` or `draft`; `deferred` to `proposed` or `rejected`; `accepted` to
  `superseded`; `rejected` to `draft`; `superseded` final); `reject` may write `reason:`, `defer` writes `trigger:`
  and needs one, and `setStatus` to `deferred` is refused for lack of one; `resolve` writes `resolved_by:` and the
  `## Decision` text; `reid` rewrites `resolved_by`.

- Owner decisions (2026-11-18, at the build): the item's four decisions as it recommends, on the owner's delegation, and
  the builder's as recommended above. The plan text was applied to 0.10, Planning sources. A bare-id write still runs
  without the daemon until `ts-0136-rr`, so it carries no actor: the provider reads it as a person, refuses `accept` for
  want of a date, and does not wall a run's `reject` or `defer`; carried to `ts-0136-rr` Notes.

### Not done, by design

- The phase-end check for `status:` and `by:` lines an agent wrote by hand: Stage 8 (`protectedPaths` at `phase-end`),
  as the item says.
- The provider's own wall cannot be reached through the daemon, which refuses a run's agent first (`ts-0128-bb`), so it
  is tested in the provider tests only, with an `agent` actor.
- The command shapes `collide task <id> accept` and the `defer` and `resolve` words on the command line, and the app's
  buttons for the ops: `ts-0136` (the CLI) and the app items of Stage 6. Their argument schemas are declared here, so
  the host's positional rule already reads `defer <trigger>` and `resolve <text>`.
- A check on read that an `accepted` proposal has `by:`, or that `trigger:` and `reason:` hold text: a file hand-edited
  as it was still lists, as the item says.
