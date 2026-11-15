---
id: ts-0043-fr
kind: task
status: active
title: "React component tree"
owners: []
links:
  - { type: blocked-by, to: ts-0042-vn }
  - { type: blocked-by, to: ts-0037-vg }
---

## Goal

In `app/`, the React component tree for Field, Tracks, Queue, run detail (Output, Time breakdown, Review), Compare and Experiments, following S6's surfaces, with typed props and placeholder rendering, wired to the reducer of `ts-0042-vn`. The Queue item's context view renders the six sections of the context record, showing a missing one as `Not available in this view`. Bodies that need later stages are stubs with tags.

## Success criteria

- The app builds and renders each surface with placeholders.
- The Queue context view shows all six sections, and a section the record lacks shows `Not available in this view`.
- Every component's props are typed from the generated types, and `tsc --noEmit` passes.
- Every stub carries a tag that belongs to a stage.

## Tests

Write first: a component smoke test per surface (it renders with placeholder props); the context-view test with each section missing.

## Notes

- IMPLEMENTATION_PLAN.md, 0.11 (the surfaces; the context view is a data contract), 0.15 (Pass 1: the React component tree), Stage 2: Skeleton (the React tree for Field, Tracks, Queue, run detail, Compare and Experiments). S6's record: `.collide/planning/spikes/sp-0009-bb-interface-prototype/record.md`.
- Area: `app/` (components).
- Done when the criteria hold.
