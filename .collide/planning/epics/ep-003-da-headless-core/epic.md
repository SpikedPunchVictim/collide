---
id: ep-003-da
alias: stage-3
kind: epic
status: active
title: "Stage 3: Headless core"
owners: []
links: []
---

## Goal

`collide run` starts one Claude phase on a task file in a new worktree, and records it (3a and 3b). The thin loop puts that run behind a prompt box in the app, against scratch tasks (3c). The ACP engine keeps the `AgentSession` trait honest (3d).

## Success criteria

- Stage 3a: Process owner, with python children. IMPLEMENTATION_PLAN.md, Stage 3a, Success Criteria.
- Stage 3b: Claude adapter and the first run. IMPLEMENTATION_PLAN.md, Stage 3b, Success Criteria.
- Stage 3c: The thin loop, in the app (prompt, `collide/quick` run, review, accept). IMPLEMENTATION_PLAN.md, Stage 3c, Success Criteria. It runs after 3b.
- Stage 3d: ACP engine, over a scripted fake ACP child. IMPLEMENTATION_PLAN.md, Stage 3d, Success Criteria. It follows 3c.

## Notes

Stage 3a is split (2026-11-15): `ts-0050-xc` to `ts-0059-xs`, built one at a time in id order, because they share `host` and the generated files. Stage 3b is split (2026-11-15): `ts-0060-mm` to `ts-0071-fd`, built the same way. Fake-Claude work comes first. The real-`claude` steps are in `ts-0070-mk`, and each needs the owner's OK. The owner's own checks (the Orchard run, the restart, the sleep test) are in `ts-0071-fd`. Stages 3c and 3d are split as each starts (0.10, 0.15: only the next stage is split). Each stage's Status line in the plan names the revision its items must come from.
