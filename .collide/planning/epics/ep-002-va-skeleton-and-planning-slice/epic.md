---
id: ep-002-va
alias: stage-2
kind: epic
status: done
title: "Stage 2: Skeleton and planning slice"
owners: []
links: []
---

## Goal

Every cross-process contract defined in full; Milestones A and B as outlines that compile and do nothing; the planning slice working for real, so Collide tracks its own build. The items were brought to plan revision 13 on 2026-11-14.

## Success criteria

- IMPLEMENTATION_PLAN.md, Stage 2: Skeleton, Success Criteria (revision 13), including the contracts R5 added: the shim journal and framed messages, `pending` with `retryAfter` and the `wait` keys, the actor from peer credentials and the socket directory rules, `merged` and `pending` with the `integrating` state, intent and outcome events, the `ask.*` and `ui_tools.*` contracts, the `agents` and `ask` settings keys and `layout.sort`.
- Every item of this epic is `done`.

## Notes

Build the planning slice first (part L). Its items, in order: `ts-0007-ga`, `ts-0008-mx`, `ts-0009-cu`, then `ts-0010-yb` and `ts-0027-ar`, `ts-0011-at`, `ts-0028-hq`, `ts-0014-yc`, then `ts-0029-ea` and `ts-0015-dp`, `ts-0020-tt`, then `ts-0030-qw` and `ts-0031-xu`, and last `ts-0021-yg`. The other items follow whenever their links allow.

The Stage 2 plan text does not outline the ACP engine or its profile. They are Stage 3d.

Running `collide` from a checkout (`ts-0021-yg`): after `pnpm install` and `pnpm build:packs` (once, and again after a change under `packs/`), run it from anywhere in the checkout with

    cargo run -q -p collide-host --bin collide -- <args>

for example `-- tasks --ready`, `-- task ts-13`, or `-- task ts-13 setStatus active --commit`. From here on the items of `.collide/planning/` are read and changed only through `collide tasks` and `collide task` (0.10, Collide's own build tasks): status, links and claims with an op and `--commit` (one line, one file, one commit), new items with `collide task create`. Bodies (Goal, criteria, Notes) are still edited by hand and committed with the work they describe.
