---
id: ts-0213-as
kind: task
status: ready
title: "Owner run: the opt-in real-agent tests"
owners: []
links:
  - { type: blocked-by, to: ts-0212-km }
---

## Goal

The owner runs the four opt-in checks of `ts-0212-km` with `COLLIDE_REAL_AGENT=1`, and records their results here:
the contract test's pass count, time, cost and input tokens per run, the Claude snippet check, `opencode mcp list`,
and the strict line's token comparison.

A build agent never runs it. It never sets `COLLIDE_REAL_AGENT=1` and never runs `claude` or `opencode`. Once
`ts-0212-km` is done, it writes the command lines and a short checklist under `### Owner checklist`. The owner runs
them and records the results here as owner measurements, with what was awkward.

## Success criteria

- The contract test passes two of three runs, or its failure is recorded with each run's groups. The pass count,
  time, cost and input tokens of each run are recorded. Owner-run.
- The planted marker reached no answer. Owner-run.
- The Claude snippet connects a real `claude -p` that calls `query` once. Owner-run.
- `opencode mcp list` shows the endpoint connected, or the check is recorded as skipped. Owner-run.
- The input tokens of the two example asks under `config: none` and `config: user` are recorded beside S6's 28K to
  107K. Owner-run.
- The total spend and the time are recorded, within the cap. A line records the date.

## Tests

None by a build agent: this run is the owner's. It runs the checks of `ts-0212-km`.

## Notes

- IMPLEMENTATION_PLAN.md: Stage 6b, Success Criteria (Real-agent contract test and the three more opt-in checks);
  The token cost of the strict line is measured here. Part L.
- **Owner's choice, recommendation taken (2026-11-24): spend cap $1.00, time 45 minutes.** Three contract runs, one
  snippet call and four token asks; S6's asks cost under $0.01 each. The ledger's limit is set to the cap.
- `claude` uses the real `HOME`, for its login, by choice; everything Collide writes is in a temp root.
- Area: this item's own text. No code.
- The test rule, for anything automated this run leans on: no default test touches the real home, real launchd, a
  real daemon, the keychain or the network. The opt-in checks reach the network only through `claude` and
  opencode, only with the variable set, by the owner.
- Stage 6b item (revision 26 scope, split 2026-11-24). Items of Stage 6b run one at a time, in id order, because they
  share `app/`, `core`, `host` and the generated files.
- Nothing a build agent runs here starts a model. The owner's run does, within the cap.
- Owner decisions (2026-11-24): the owner took the recommendations at the split.

### Owner checklist

Written by the build agent once `ts-0212-km` is done.
