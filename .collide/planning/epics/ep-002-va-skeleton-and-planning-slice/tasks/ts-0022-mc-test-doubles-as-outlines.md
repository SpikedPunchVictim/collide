---
id: ts-0022-mc
kind: task
status: ready
title: "Test doubles as outlines"
owners: []
links:
  - { type: blocked-by, to: ts-0019-vm }
---

## Goal

A fake adapter that emits normalized events, a second `AgentSession` that is a scripted fake ACP child, a fake provider, a fake hook runner.

## Success criteria

- The doubles compile against the outlined traits.
- Both `AgentSession` doubles drive the same contract test.

## Notes

IMPLEMENTATION_PLAN.md, 0.15.
