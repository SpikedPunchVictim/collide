---
id: ts-0045-es
kind: task
status: done
title: "Document how Collide manages tasks and documents"
owners: []
links: []
---

## Goal

One guide, for a person or an agent working in a repo that uses Collide's planning layout, on how Collide keeps
its tasks and its documents. Tasks: the shared root's layout, item frontmatter, kinds and statuses, ids, links and
the ready rule, epics from folders, spikes, how to write an item one Sonnet agent can finish, and the `collide task`
CLI (marked as arriving with the planning slice until it exists). Documents: the plan and its revisions, the Review
log, Deferred and Open questions, spike records, review ledgers, research notes, claim labels and the citation rule.
Collide's own repo is the worked example.

## Success criteria

- Every rule in the guide matches the plan (0.10, 0.15, part L, Stage 2's planning slice) and the files under
  `.collide/planning/`; nothing planned is described as built.
- A reader can add a task, link it, and move it through its statuses from the guide alone.
- `.collide/planning/README.md` points at the guide.

## Tests

- Each command and file path in the guide exists in the plan or the repo (checked by hand until the CLI exists).

## Notes

- Owner, 2026-11-14: "we will also need documentation on how collide manages documentation/tasks".
- Area: `docs/`, `.collide/planning/README.md`.
- Update the CLI section when the planning slice's CLI items land.
