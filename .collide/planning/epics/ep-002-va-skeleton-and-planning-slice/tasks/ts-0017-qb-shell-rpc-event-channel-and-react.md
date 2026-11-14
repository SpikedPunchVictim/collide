---
id: ts-0017-qb
kind: task
status: ready
title: "Shell rpc, event channel and React component tree"
owners: []
links:
  - { type: blocked-by, to: ts-0016-ka }
---

## Goal

The shell's `rpc` command and event channel; the React component tree following S6's surfaces: Field, Tracks, Queue, run detail (Output, Time breakdown, Review), Compare and Experiments, with typed props and placeholder rendering, and one reducer for UI state in which every user action is a reducer action.

## Success criteria

- The app builds and renders each surface with placeholders.
- Every user action is a reducer action, and a reducer test drives one without a browser.

## Tests

- Component smoke tests; reducer tests.
