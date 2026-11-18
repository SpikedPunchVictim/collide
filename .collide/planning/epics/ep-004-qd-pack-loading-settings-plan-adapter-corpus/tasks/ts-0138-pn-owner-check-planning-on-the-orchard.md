---
id: ts-0138-pn
kind: task
status: ready
title: "Owner check: planning on the Orchard repo and Collide's own records"
owners: []
links:
  - { type: blocked-by, to: ts-0137-pg }
---

## Goal

The Stage 4c checks that only the owner runs, on the Orchard repo and on Collide's own planning folders: the Decisions
needed view against the plan CLI's human queue, the private root and the layout against a real repository, and the
planning provider against the real `arch/` shapes of ADRs, spikes and questions.

A build agent never runs it. It never touches the Orchard repo or any other repo, never starts a daemon against the
owner's data, and never runs `claude`. It writes the command lines and a short checklist under `### Owner checklist`,
from the code and from the Built sections of `ts-0124-xm` to `ts-0137-pg`. The owner runs them and records the result
here as owner judgement, with what was awkward.

The owner's run is also the one place the fixtures of `ts-0137-pg` are compared with the real `arch/` files and the real
`plan status --format json`. A difference is recorded here and becomes a fix item.

## Success criteria

- Over the Orchard repo, the Decisions needed view lists the same question ids as the human queue in
  `plan status --format json`. Owner-run, in a clone made for the check.
- An ADR's `evidence` link shows from the spike's side as "evidence for", for an ADR and spike of the real repo.
  Owner-run.
- Collide's spike records in `.collide/planning/spikes/` list in `collide tasks --view spikes --json` with their
  statuses and no item from inside a spike folder other than its record. Owner-run, on a clone of the Collide repo.
- A private-root item never appears in `git status` of the Collide clone, and is listed by `collide tasks`. Owner-run.
- A layout path outside the repository or under `.git/` is refused with the reason. Owner-run.
- A proposal is accepted from the app's connection, and the same from the CLI waits as a confirmation. Owner-run, with
  the app if it can show it (the Queue page of Stage 3c), else by the answer from the CLI's `queue` command.
- `collide task reid` on an item whose id was made twice (two branches merged, in a clone) rewrites the links that name
  it. Owner-run.
- The fixtures of `ts-0137-pg` match the real `arch/` shapes and the real plan CLI, or each difference is written down.
  Owner-run.
- The checklist holds the command lines, with `<collide checkout>`, `<orchard copy>` and `<collide clone>` as
  placeholders, what to look at, how to clean up, and a line for the owner's result and one for what was awkward.
- A line records the date Stage 4c is judged done by the owner.

## Tests

None by a build agent: this check is the owner's. The checklist names the automated tests it relies on:

- `ts-0124-xm`, `ts-0125-uh`: the layout setting and the private root;
- `ts-0126-nf`, `ts-0127-js`: the spike and question kinds, the planning links and ids;
- `ts-0128-bb`, `ts-0129-uv`: the actor and the person-only rule, the proposal's transitions;
- `ts-0130-mv` to `ts-0134-ch`: feature ops, the quality parts, coverage, claim blocks;
- `ts-0135-cx`, `ts-0136-rr`: the pack's views and roles, the CLI behind the daemon;
- `ts-0137-pg`: the Orchard plan provider for ADRs, spikes and questions.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4c: Success Criteria (the criteria above that name the Orchard repo, Collide's records, the private root and
    the layout); Tests.
  - 0.10: Planning sources. 0.9: Trust. Part L.
- What the checklist covers. The build agent fills in the exact commands once the earlier items are built.
  - **Set up.** Build the CLI and the daemon from `<collide checkout>`. Make `<orchard copy>` as a clone of the owner's
    local copy in a folder made for the check (never the repo the owner works in) and `<collide clone>` of this
    repository. Use a fresh `HOME` and `COLLIDE_DAEMON_ROOT` for everything. Say which steps, if any, use the real
    `HOME`.
  - **Spikes.** `collide tasks --view spikes --json` in `<collide clone>`.
  - **Private root and layout.** Make an item in the private root, see `git status`; set a layout path of each bad
    kind and read the refusal.
  - **Orchard.** `collide init`, commit, `collide trust`; `collide tasks --view decisions-needed --json` against
    `plan status --format json`; the evidence link from a spike.
  - **Accept.** A proposal from the app, then from the CLI.
  - **Reid.** The two-branch collision in a clone.
  - **Fixtures.** The real ADR, spike and question files and the plan CLI's JSON, next to `fixtures/orchard-arch/`.
  - **Record.** The result, the time it took, and what was awkward.
- **Decision for the owner, with a recommendation: the app for the accept step.** The confirmation is shown by the Queue
  lane of Stage 3c's loop page; Stage 6 builds the full Queue page. Recommend the loop page, since it exists. If it
  does not show the kind well, that is a result to record.
- **Decision for the owner: what to share first.** The sanitized sample of `ts-0137-pg`'s Owner's choice makes this
  check short.
- Area: this item's own text. No code.
- The test rule, for anything automated this check leans on: no test touches the real home, real launchd or a real
  daemon. Each test clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as
  `host/tests/common/mod.rs` `TempRoot` does. Settings files are fixtures, never the owner's own `settings.json`. The
  owner's own run here uses the owner's real repo copy by choice, where the checklist says so.
- Stage 4c item (revision 23 scope, split 2026-11-18). Items of Stage 4c run one at a time, in id order, because they
  share `core`, `host`, `packs/plan` and the generated files.
- Nothing here runs a real `claude`.
- Owner decisions: none recorded yet. The recommendations stand until the owner answers at the split.
