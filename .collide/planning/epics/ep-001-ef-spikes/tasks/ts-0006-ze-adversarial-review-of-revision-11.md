---
id: ts-0006-ze
alias: R5
kind: task
status: ready
title: "Adversarial review of revision 11 (R5)"
owners: []
links:
  - { type: blocked-by, to: ts-0005-st }
---

## Goal

Before the skeleton starts, independent reviewers (Fable and Opus, as in R1–R4) attack the finalized plan: contracts that cannot hold, stages in the wrong order, missing failure modes, cost and scope. Part L's planning-first slice and item format are in scope.

## Success criteria

- Each finding is labelled core or pack, with its plan location.
- The owner's decision on each finding is applied, rejected with a reason, or deferred with a trigger.
- A review-log entry names the stages changed.

## Notes

Owner, 2026-11-13. Reviewers are read-only and get the plan and records, not our conclusions.
