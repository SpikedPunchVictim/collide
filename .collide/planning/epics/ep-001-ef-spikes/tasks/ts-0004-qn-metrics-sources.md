---
id: ts-0004-qn
alias: S8
kind: task
status: ready
title: "S8: metrics sources"
owners: []
links: []
---

## Goal

Measure the metric sources on an Orchard iteration run and a lane test: sampler overhead, missed short-lived processes,
disk and network per process, GPU, Docker, sleep detection, store growth, telemetry tool timing, report-file tailing.

## Success criteria

- A sampling interval whose overhead is below 1% of one core.
- Each resource's granularity and what it cannot attribute.
- Whether telemetry tool timings beat stream-json timing, and by how much (labels the breakdown's precision).

## Tests

- The reproduction script runs from a clean checkout and prints its measurements.

## Notes

The record and its scripts go in a new spike folder under `.collide/planning/spikes/` (plan 0.10). Settles the breakdown's open timing choices: model-time gap threshold, nested subagent calls, share denominator.
