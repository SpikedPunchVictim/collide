---
id: ts-0018-dd
kind: task
status: ready
title: "Point and event catalogues; state, events and spans"
owners: []
links:
  - { type: blocked-by, to: ts-0009-cu }
---

## Goal

The point and event catalogues in full, and the state, event and span types. Every span has a kind and a parent id, and a wait on the user names the Queue item it waited on. The Queue item's context view is a record of six sections, each filled or `Not available in this view`.

## Success criteria

- CI lists each catalogue point with the stage that makes it fire.
- A span without a kind or a parent id (except a root) does not compile or validate.
- A context record with a missing section is invalid: the section says `Not available`.

## Tests

- Catalogue completeness test.

## Notes

IMPLEMENTATION_PLAN.md, 0.4, 0.8, 0.11 and 0.13.
