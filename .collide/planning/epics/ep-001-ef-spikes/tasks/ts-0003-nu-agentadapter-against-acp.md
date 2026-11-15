---
id: ts-0003-nu
alias: S7
kind: task
status: done
title: "S7: AgentAdapter against ACP (paper check)"
owners: [claude-sonnet]
links: []
---

## Goal

Map the `AgentAdapter` trait onto ACP, run `opencode acp` once with a local Ollama model, and list every member ACP cannot express.

## Success criteria

- `.collide/planning/spikes/sp-0006-kq-agentadapter-acp/record.md` with the mapping table and proposed trait changes.
- The trait has no member ACP cannot express, or each such member is a capability.

## Tests

- The live-run driver in `.collide/planning/spikes/sp-0006-kq-agentadapter-acp/` re-runs and logs every message.
