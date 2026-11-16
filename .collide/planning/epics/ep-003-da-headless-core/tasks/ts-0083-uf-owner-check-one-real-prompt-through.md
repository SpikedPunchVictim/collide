---
id: ts-0083-uf
kind: task
status: active
title: "Owner check: one real prompt through the app"
owners: []
links:
  - { type: blocked-by, to: ts-0082-fp }
  - { type: blocked-by, to: ts-0071-fd }
---

## Goal

The Stage 3c check that only the owner runs: one real prompt on a worktree of Collide, through the app. The owner
reviews the diff and accepts it.

A build agent never runs it. It never starts the app against the owner's daemon, never runs `claude`, and never
touches the owner's worktree. It writes into this item, under `### Owner checklist`, the command lines and a short
checklist. The owner runs them and records the result here as owner judgement, with what was awkward.

This item also records when the dogfooding starts: from then on, the owner's Collide work that fits a prompt goes
through the thin loop (Stage 3c criterion).

## Success criteria

- The owner runs one real prompt on a worktree of Collide through the app, reviews the diff and accepts it (Stage 3c
  criterion). Owner-run, and recorded as owner judgement in this item, with what was awkward.
- The checklist holds:
  - the command lines to build and start the app, with `<collide worktree>` as a placeholder;
  - how to make the worktree of Collide and open it as a board;
  - the model, chosen by the owner through `models.yaml` for the builder's alias (haiku suggested for the first
    prompt), and a small prompt, since the page has no budget field in 3c;
  - what to look at;
  - a line for the owner's result, and one for what was awkward.
- A line records the date the owner starts sending prompt-sized Collide work through the thin loop (Stage 3c
  criterion: dogfooding).

## Tests

None by a build agent: this check is the owner's. The checklist names the automated tests it relies on:

- `ts-0077-ws`: the app's connection, and the daemon started on demand;
- `ts-0078-ye`: the run up to review, and the permission request;
- `ts-0079-pu`: accept into a checkout, including a linked worktree;
- `ts-0080-tq`: re-prompt and discard;
- `ts-0082-fp`: the loop in a browser.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 3c: the owner's real prompt and the dogfooding criteria.
  - 0.2: Starting the daemon (the app starts it on demand).
  - 0.3: Two version chips.
  - Part L.
- What the checklist covers. The build agent fills in the exact commands once the app connects.
  - **Set up.** Make a worktree of Collide on a branch of its own. Check the builder's alias in `models.yaml`. Build
    and start the app.
  - **Open.** Open the worktree as a board. Look at the two version chips.
  - **Prompt.** Type a small, real Collide change. Answer any permission request.
  - **Look for:**
    - a fresh worktree under Collide's data folder, and nothing changed in `<collide worktree>` while the run works;
    - "ready to review" with a diff that matches the change;
    - optional: close the app mid-run and open it again. The same run and item show.
  - **Accept.** The worktree's branch has the change (`git log`). The run's worktree and branch are gone. The
    scratch task is `done`.
  - **Record.** The result, the time it took, and what was awkward.
- **Decision for the owner, with a recommendation: the order.** This item waits on `ts-0071-fd`, the owner's 3b
  checks, so a real run is checked from the CLI before the app. Recommend that the Stage 3c build items start now:
  none needs anything from `ts-0071-fd`.
- Area: this item's own text. No code.
- The test rule, for anything automated this check leans on: no test touches the real home, real launchd or a real
  daemon. Each test clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root,
  as `host/tests/common/mod.rs` `TempRoot` does. Settings files are fixtures, never the owner's own
  `settings.json`. The owner's own run here uses the owner's real setup by choice.
- Stage 3c item (revision 22 scope, split 2026-11-15). Items of Stage 3c run one at a time, in id order, because
  they share `host`, the app and the generated files.
- Owner decisions (2026-11-15, at the split): the order (Stage 3c builds while `ts-0071-fd` is open; only this check
  waits on it). The recommendation, taken under the owner's delegation; the owner may change it before the item is
  built.
