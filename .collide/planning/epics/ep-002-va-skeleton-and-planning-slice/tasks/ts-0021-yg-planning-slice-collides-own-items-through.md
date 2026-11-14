---
id: ts-0021-yg
kind: task
status: ready
title: "Planning slice: Collide's own items through the CLI"
owners: []
links:
  - { type: blocked-by, to: ts-0015-dp }
  - { type: blocked-by, to: ts-0020-tt }
  - { type: blocked-by, to: ts-0030-qw }
  - { type: blocked-by, to: ts-0031-xu }
---

## Goal

Switch Collide's own build tracking from hand edits to the CLI: from here on an item's status, links and claim change only through `collide task <id> <op>`, committed with `--commit`, and new items come from `collide task create`. Make the `collide` binary runnable from a checkout with one documented command.

## Success criteria

- The owner and the agents read and update the items of `.collide/planning/` only through `collide tasks` and `collide task`.
- One item is taken from `ready` through `active` to `review` with the CLI, and each step is a one-line diff and a one-file commit.
- `collide tasks --ready` lists the next buildable item, and it equals the hand reading.
- The one command that runs `collide` from a checkout is written in the epic's `epic.md` Notes.

## Tests

Write first: none of its own. The check is the worked example above, run on a throwaway branch and then repeated on the real items.

## Notes

- IMPLEMENTATION_PLAN.md, 0.10 (Collide's own build tasks), Stage 2: Skeleton, Planning slice (from here on, Collide's own build tasks are read and updated through the CLI).
- Area: the item files, `.collide/planning/epics/`.
- Standing rule after this item: Stages 3 to 6 are split into items with `collide task create` as each starts, and only the next stage is split (0.10).
- Done when the criteria hold. This is the point where Collide starts tracking its own build (part L).
