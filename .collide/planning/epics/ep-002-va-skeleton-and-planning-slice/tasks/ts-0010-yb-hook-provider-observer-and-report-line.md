---
id: ts-0010-yb
kind: task
status: ready
title: "Hook, provider, observer and report-line protocols"
owners: []
links:
  - { type: blocked-by, to: ts-0009-cu }
---

## Goal

The three script protocols and the report line format, in full, with JSON Schema and generated types.

## Success criteria

- Serde round-trips pass for every protocol type.
- The report line has the caps of 0.5: 4 KB a line, 100 lines a second with a burst of 1,000, 16 MiB a file. An excess line is dropped and "report truncated" is recorded.

## Tests

- Round-trip tests; schema tests; the report caps.

## Notes

IMPLEMENTATION_PLAN.md, 0.5 and 0.10.
