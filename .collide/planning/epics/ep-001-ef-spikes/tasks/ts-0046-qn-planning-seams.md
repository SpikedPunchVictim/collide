---
id: ts-0046-qn
alias: S10
kind: task
status: ready
title: "S10: planning seams"
owners: []
links: []
---

## Goal

Check the planning seams that the group-runs stage builds on (owner, 2026-11-13; $3 cap), with numbers:

- Q1: a fresh agent working from a handoff note built at Share, on a second clone, against the resumed session.
- Q2: whether fan-out instances stay independent (files, `phase.dir`, auto memory).
- Q3: how often two spike records merge mechanically.

## Success criteria

- A record in a new spike folder under `.collide/planning/spikes/` answers Q1–Q3, each claim labelled measured,
  inferred or unmeasured, and lists what was left unmeasured.
- Spend stays under the $3 cap.
- The findings are applied to the plan sections the seams touch (0.1, 0.4, 0.5, 0.7, 0.8, 0.10, 0.11, 0.13), or
  recorded as needing no change.

## Tests

- The reproduction scripts in the spike folder run from a clean checkout and print their measurements.

## Notes

- IMPLEMENTATION_PLAN.md, Stage 1: Spikes (S10 planning seams; Status) and Stage 11: Group runs, integration,
  write-back (the planning spikes run after S10).
- Not ready to start yet, although its status is `ready`: it runs when the group-runs stage (Stage 11) starts, and
  no item for that stage exists to link to. When the epics for Stages 7–11 are made, give this item a `blocked-by`
  link to the one that must be done before Stage 11 starts. Until then `collide tasks --ready` lists it; skip it.
- Closing this item closes the epic `ep-001-ef` (owner, 2026-11-14: an epic's status follows its items).
