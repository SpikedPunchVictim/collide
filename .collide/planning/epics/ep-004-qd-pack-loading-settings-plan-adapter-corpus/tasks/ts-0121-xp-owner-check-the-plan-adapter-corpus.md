---
id: ts-0121-xp
kind: task
status: ready
title: "Owner check: the plan adapter, corpus and views on the Orchard repo"
owners: []
links:
  - { type: blocked-by, to: ts-0120-wm }
---

## Goal

The Stage 4b checks that only the owner runs, on the Orchard repo: the plan adapter, the corpus and the views against
the real `arch/` tree and the real plan CLI, which the agent-built items only meet as fixtures.

A build agent never runs it. It never touches the Orchard repo or any other repo, never starts a daemon against the
owner's data, and never runs `claude`. It writes the command lines and a short checklist under `### Owner checklist`,
from the code and from the Built sections of `ts-0108-dq` to `ts-0120-wm`. The owner runs them and records the result
here as owner judgement, with what was awkward.

The owner's run is also the one place the fixtures are compared with the real thing: the `arch/` shapes of
`fixtures/orchard-arch/` against the real files, and the fake plan CLI against the real one (its `--format json`,
`--dry-run` and exit codes). A difference is recorded here and becomes a fix item.

## Success criteria

- A task shows `doing` in one worktree and `ready` at the board root, with the divergence reported by `collide task <id>
  --json` and `task.get`. Owner-run.
- The MVP view's `closure` for T-01-042 has the same task set and the same steps as the prototype board's generator's
  for T-01-042 at the same commit. Owner-run. A difference is the closure rules of `ts-0118-wp` to correct, and the
  record says which tasks differ.
- `"plan:E-07" in collections` returns the same tasks as the prototype board's E-07 epic. Owner-run.
- `setStatus` with an illegal transition comes back `refused`, with plan's message. Owner-run.
- A legal `setStatus` reports exactly the task file and its regenerated epic table as touched, and `setStatus` with
  category `active` writes `doing`. Owner-run.
- A write into a checkout with a running phase is queued, not applied. Owner-run, with a phase that does not call
  `claude` (see Notes).
- A write to the board root during a rebase waits, and the CLI says why. It applies once the rebase ends. Owner-run, in
  a copy of the repo made for the check.
- An edited provider script or view on a branch is not used, and a changed hash asks again. Owner-run.
- A scratch task created with `collide task new` is listed for that board only. Owner-run.
- The fixtures match the real `arch/` shapes and the real plan CLI, or each difference is written down. Owner-run.
- The checklist holds the command lines, with `<collide checkout>`, `<orchard copy>` and `<scratch repo>` as
  placeholders, what to look at, how to clean up, and a line for the owner's result and one for what was awkward.
- A line records the date Stage 4b is judged done by the owner.

## Tests

None by a build agent: this check is the owner's. The checklist names the automated tests it relies on:

- `ts-0108-dq`, `ts-0109-qj`: the supervisor against a fake provider, and the SDK's hooks;
- `ts-0110-ra`, `ts-0111-vn`, `ts-0112-px`: the adapter against fixture `arch/` trees and the fake plan CLI, in one
  checkout and two (rename, collision, watch);
- `ts-0113-ck`, `ts-0114-hr`: the daemon's sources and trust gate, and the write guards (root busy, the phase queue);
- `ts-0115-ug`, `ts-0116-wh`: the corpus, its cache and its events;
- `ts-0117-gu`, `ts-0118-wp`, `ts-0119-tj`: CEL, the views, and the views in the daemon;
- `ts-0120-wm`: the CLI.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 4b: Success Criteria ("On the Orchard repo", the nine criteria above); Tests.
  - 0.4: Root busy. 0.9: Trust. 0.10: Providers, Ref-scoped sources, Views. Part L.
- What the checklist covers. The build agent fills in the exact commands once the earlier items are built.
  - **Set up.** Build the CLI and the daemon from `<collide checkout>`. Make `<orchard copy>` with `git clone` of the
    repo or `git worktree add` into a folder made for the check (never the repo the owner works in), and a second
    worktree of it. Use a fresh `HOME` and `COLLIDE_DAEMON_ROOT` for everything, so the owner's settings and daemon stay
    as they are. Say in the checklist which steps, if any, use the real `HOME`.
  - **Read side.** `collide init`, commit, `collide trust`; `collide tasks`; `collide task <id> --json` for a task whose
    state differs between the two worktrees (change one with the plan CLI, in the worktree).
  - **Views.** `collide tasks --view <name>` for the MVP view and a view on `"plan:E-07" in collections`; compare the
    set and the steps with the generator's output at the same commit, and the E-07 set with the epic's.
  - **Writes.** An illegal `setStatus` through `collide task <id> setStatus <status>`, a legal one, and one by category
    `active`; the files touched (`git status` in the copy) against the report.
  - **Queue and wait.** A write while a phase is live in the checkout (a workflow of the fixture packs that runs a
    `sleep`), then one in a copy stopped mid-rebase, read the CLI's message, finish the rebase, see the write apply.
  - **Trust.** Edit the adapter script and a view in a commit on a branch that is not the line, see neither used; change
    one on the line and see the second prompt.
  - **Scratch.** `collide task new` in `<orchard copy>` and in `<scratch repo>`; each lists only its own.
  - **Fixtures.** The real `arch/` task file, epic file with its generated table, id map head, and the plan CLI's
    `--format json`, `--dry-run` and an illegal-status run, next to `fixtures/orchard-arch/`; write the differences.
  - **Record.** The result, the time it took, and what was awkward.
- **Decision for the owner, with a recommendation: the phase for the queue check.** The queue criterion needs a live
  phase in a checkout. Recommend a fixture workflow whose phase is a shell `sleep`, which needs no `claude` and no
  spend. Alternative: a real `claude` phase on haiku, which costs, and the owner sets the cap in the checklist (a figure
  of `$0.10`) if they choose it.
- **Decision for the owner: what to share first.** If the owner supplies a sanitized sample of the `arch/` files and the
  plan CLI's output before `ts-0110-ra` is built (see that item's Notes), the fixtures start from the real shape and
  this step's fixture comparison is short.
- Area: this item's own text. No code.
- The test rule, for anything automated this check leans on: no test touches the real home, real launchd or a real
  daemon. Each test clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as
  `host/tests/common/mod.rs` `TempRoot` does. Settings files are fixtures, never the owner's own `settings.json`. The
  owner's own run here uses the owner's real repo copy by choice, where the checklist says so.
- Stage 4b item (revision 23 scope, split 2026-11-17). Items of Stage 4b run one at a time, in id order, because they
  share `core`, `host` and the generated files.
- Nothing here runs a real `claude`, unless the owner picks the alternative above.
- Owner decisions (2026-11-17, at the split): the recommendations above, on delegation (a fixture phase that runs
  `sleep`). The owner made a local copy of the Orchard repo; which steps of this check could run on it is decided when
  the checklist is written.
