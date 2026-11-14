---
id: ts-0022-mc
kind: task
status: ready
title: "Test doubles as outlines"
owners: []
links:
  - { type: blocked-by, to: ts-0019-vm }
  - { type: blocked-by, to: ts-0036-xt }
---

## Goal

Test doubles in `core` (a `testing` module behind a feature): a fake adapter that emits normalized events from a script; a second `AgentSession` that is a scripted fake ACP child, sans-IO, with a handshake, request ids and a pending permission request (the ACP engine itself is Stage 3d, which reuses this script); a fake provider (an in-memory `TaskProvider`); a fake hook runner that returns scripted results. One contract test drives both `AgentSession` doubles.

## Success criteria

- The doubles compile against the outlined traits.
- Both `AgentSession` doubles drive the same contract test: start, feed, send a turn, answer a permission request, cancel, finish. The same input gives the same output.
- The fake ACP child can hold a permission request pending across feeds, so the replay item can check it.

## Tests

Write first: the shared `AgentSession` contract test, run against both doubles; the fake provider's `list`, `get`, `apply`; the fake hook runner's scripted results.

## Notes

- IMPLEMENTATION_PLAN.md, 0.15 (Dependency injection; both fixture packs and the fake adapter), 0.3 (Two core traits), Stage 2: Skeleton (Test doubles as outlines).
- Area: `core` (the `testing` module).
- Done when the criteria hold.
