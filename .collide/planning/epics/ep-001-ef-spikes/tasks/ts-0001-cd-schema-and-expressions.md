---
id: ts-0001-cd
alias: S5
kind: task
status: review
title: "S5: schema and expressions"
owners: [claude-sonnet]
links: []
---

## Goal

Choose the Rust → JSON Schema → TypeScript pipeline, the YAML crate (and the TypeScript YAML parser for frontmatter), and the CEL crate, and show a nested-macro view over 1567 tasks is stopped within a cost limit.

## Success criteria

- `.collide/planning/spikes/sp-0005-fv-schema-expressions/record.md` with measured numbers and recommendations.
- `.collide/planning/spikes/sp-0005-fv-schema-expressions/` re-runs every measurement with one command.

## Tests

- The reproduction script runs from a clean checkout and prints its measurements.

## Notes

Also covers YAML hazards in planning-item frontmatter (ids that parse as numbers).
