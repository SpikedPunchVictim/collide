---
id: ts-0005-st
kind: task
status: ready
title: "Apply S5–S8 to the plan (revision 11)"
owners: []
links:
  - { type: blocked-by, to: ts-0001-cd }
  - { type: blocked-by, to: ts-0002-bj }
  - { type: blocked-by, to: ts-0003-nu }
  - { type: blocked-by, to: ts-0004-qn }
---

## Goal

Read the four decision records against the plan, take the owner's decisions, and write revision 11.

## Success criteria

- Every recommendation in the four records is applied, rejected with a reason, or deferred with a trigger.
- The review log has an entry naming the stages changed.
- Stage 2's items below are updated to the chosen crates, pipeline, trait and component tree.

## Notes

The plan stays frozen until this lands.

Owner, 2026-11-14: S6's sessions with a non-owner participant are skipped (none available); the owner iterates on
the prototype. Replace every plan criterion that names "the non-owner participant" (Stage 1's S6 item and criteria,
and the later stages that reuse S6 scenarios and thresholds in the real app) with checks the owner can run. See the
S6 record, "Session results".
