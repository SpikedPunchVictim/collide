---
id: ts-0005-st
kind: task
status: review
title: "Apply S5–S8 to the plan (revision 11)"
owners: []
links:
  - { type: blocked-by, to: ts-0001-cd }
  - { type: blocked-by, to: ts-0002-bj }
  - { type: blocked-by, to: ts-0003-nu }
  - { type: blocked-by, to: ts-0004-qn }
---

## Goal

Read the four decision records against the plan, take the owner's decisions, and write revision 11.

## Success criteria

- Every recommendation in the four records is applied, rejected with a reason, or deferred with a trigger.
- The review log has an entry naming the stages changed.
- Stage 2's items below are updated to the chosen crates, pipeline, trait and component tree.

## Notes

The plan stays frozen until this lands.

Owner, 2026-11-14: S6's sessions with a non-owner participant are skipped (none available); the owner iterates on
the prototype. Replace every plan criterion that names "the non-owner participant" (Stage 1's S6 item and criteria,
and the later stages that reuse S6 scenarios and thresholds in the real app) with checks the owner can run. See the
S6 record, "Session results".

## Ledger (revision 11)

One row per recommendation or proposed change in the four records (`sp-0005-fv-schema-expressions`, `sp-0006-kq-agentadapter-acp`, `sp-0009-bb-interface-prototype`, `sp-0010-bn-metrics-sources`), with the owner's decisions of 2026-11-14. Plan sections and stages are cited by name. S5 and S6 give no separate recommendation list beyond the rows shown.

Totals: 77 rows. Applied 71, rejected 3, deferred 2, noted with no plan change 1.

| Source | Disposition | Where | Reason or trigger |
| --- | --- | --- | --- |
| S5 rec 1 (schemars, JSON Schema, json-schema-to-typescript; `Wait {}`; `uint*` formats; split input/output) | applied | 0.6 Types bullet; 0.15 chosen crates; Stage 2 contracts, criteria, tests; task ts-0008-mx | S5 named the 0.7 Types bullet; it is in 0.6. Owner chose this pipeline, not specta or ts-rs. |
| S5 rec 2 (`serde-saphyr` hardened options pinned by a test; no anchors; size cap) | applied | 0.7 YAML bullets; 0.9 untrusted YAML; 0.15; Stage 2 criteria and tests; task ts-0012-ua |  |
| S5 rec 3 (loader hints, `minLength: 1`, schema-check `config` maps) | applied | 0.7; Stage 2; task ts-0012-ua |  |
| S5 rec 4 (`yaml` core mode, size cap, no `gray-matter`, no 1.1 mode) | applied | 0.10 frontmatter bullet; 0.15; Stage 4 plan adapter; task ts-0014-yc; `docs/research/planning.md` 4.2 | The 64 KiB cap is S5's suggestion, labelled as such. |
| S5 rec 5, id rule `^[a-z]{2,}-[0-9a-z]{4,}$` with the bare short id as display alias only | rejected | 0.10 ids bullet (unchanged rule) | Superseded by the owner's id rule of 2026-11-13, `^[a-z]{2}-[0-9]{3,}-[a-z]{2}$`. S5's `e-` rationale is kept in 0.10 as support for the two-letter suffix. |
| S5 rec 5, quote hashes | applied | 0.10 ids bullet; `docs/research/planning.md` 4.2 (example `preregistered` quoted); task ts-0014-yc |  |
| S5 rec 6 (`cel` 0.15 plus step counter, per-task and per-pass limits, 1 KB cap, list caps, `catch_unwind`, no corpus variable) | applied | 0.9 CEL bullet; 0.10 fields; 0.15; Stage 1 S5 bullet; Stage 4 view tests | Owner: vendor the patch and offer it upstream. The limits of 10,000 steps per task and 2,000,000 per pass are S5's suggestions, labelled inferred. |
| S5 rec 7 (static cost estimate at view save and load) | applied | 0.9; 0.10 fields; Stage 4 views and tests | Refusal of a view over the per-pass budget is labelled inferred. |
| S5 change 1 (0.7 first bullet: `strict_booleans`, `no_schema`, duplicate keys, no anchors or merge keys, size cap) | applied | 0.7; 0.9; Stage 2 criteria; task ts-0012-ua |  |
| S5 change 2 (0.7 `serde_yaml` bullet names `serde-saphyr` 2.x) | applied | 0.7 (`serde_yaml` archived and not used); 0.15 |  |
| S5 change 3 (Types bullet and Stage 2: `Wait {}`, input/output split, Node build step) | applied | 0.6 Types bullet; Stage 2; task ts-0008-mx | The bullet is in 0.6, not 0.7. |
| S5 change 4 (0.9 CEL bullet: no limit in `cel`, step counter, budgets, 1 KB cap, list caps, thread deadline is not a limit) | applied | 0.9 CEL bullet | The note that a 4 KB expression can overflow a 2 MB stack is kept as the reason for the source cap. |
| S5 change 5 (0.10: views bind per-task fields only; static estimate at save and load) | applied | 0.10 fields bullet; 0.9 |  |
| S5 change 6 (Stage 1 S5 bullet marked done) | applied | Stage 1 S5 bullet, with its record by folder name |  |
| S5 change 7 (planning.md 4.2: id rule `^[a-z]{2,}-[0-9a-z]{4,}$`, the `e-` hazard, bare form is an alias) | rejected | `docs/research/planning.md` 2.4 and 4.2 aligned to the owner's rule instead | Superseded by the owner's id rule of 2026-11-13. The `e-` hazard is kept in 0.10 as rationale. |
| S5 change 8 (0.15: add `schemars`, `serde-saphyr`, `cel` patched) | applied | 0.15 chosen crates and packages (also `json-schema-to-typescript`, `yaml`) | There was no dependency list in 0.15; the bullet was created. |
| S6 change 1 (wait spans are data, linked to the Queue item) | applied | 0.13 model bullets; Stage 2 contracts; task ts-0018-dd |  |
| S6 change 2 (three time totals, span parent id) | applied | 0.11 tool-call breakdown; 0.13; Stage 2; task ts-0018-dd |  |
| S6 change 3 (context view is a data contract; "Not available") | applied | 0.11 Queue pane; Stage 2 contracts; task ts-0018-dd |  |
| S6 change 4 (no colour for run identity) | applied | 0.11 Tracks and run-identity bullet | Q1 default `id`. |
| S6 change 5 (Queue answer keys per kind in one table; keep an undo) | applied | 0.11 keyboard triage table | Q2 default `literal`. The exact reach of undo is labelled inferred. |
| S6 change 6 (an experiment result states its scope) | applied | 0.11 experiments result |  |
| S6 change 7 (fake-data usability gate with a person who did not design it) | rejected | Stage 1 S6 item and criteria; Stages 6, 9, 11, 15 criteria | Withdrawn by the owner on 2026-11-14: no such participant exists. Replaced by S6's pilots ported to UI tests in the real app plus an owner walkthrough recorded as owner judgement. |
| S6 Q1 (run identity label) | applied | 0.11; Stage 1 | Default `id` (owner, 2026-11-14). |
| S6 Q2 (`a` and `d` per Queue kind) | applied | 0.11 keys table | Default `literal` (owner, 2026-11-14). |
| S6 Q3 (where `o` goes from a Queue item) | applied | 0.11 | Default `moment` (owner, 2026-11-14). |
| S6 Q4 (where the line switcher lives) | applied | 0.11 Lines bullet | Default `top` (owner, 2026-11-14). |
| S6 Q5 (what "review ×2" means) | applied | 0.11 Tracks | Two reviewers in parallel, A and B, not two rounds (owner, 2026-11-14). |
| S6 thresholds (time and error limits) | applied | Stage 1 S6 item; Stages 6, 9, 11, 15 criteria | Kept as recorded and advisory, because the sessions that would test them were skipped. |
| S7 P1 (split into `AgentAdapter` and a sans-IO `AgentSession`) | applied | 0.3 two core traits; Stage 2 `core` outlines; Stage 3; tasks ts-0019-vm, ts-0022-mc |  |
| S7 P2 (session id as an event: `SessionStart`, `SessionBound`) | applied | 0.3 session start; launch-line comment |  |
| S7 P3 (permission answers and requests) | applied | 0.3 permissions; 0.11 Queue pane; Stage 3 criteria | Never an "always" option; unknown input is shown as "input not available until approved". |
| S7 P4 (usage events with provenance) | applied | 0.3 usage; 0.13 agent-stream rows; Stage 3 criteria |  |
| S7 P5 (capability table with Yes, No, Unknown) | applied | 0.3 capabilities table; Stage 3 |  |
| S7 P6 (turn end and outcome) | applied | 0.3 `TurnEnd` and `FinalReport`; Stage 3 |  |
| S7 P7 (model naming, opaque `model.id`) | applied | 0.3 `model.id` beside `model.name` |  |
| S7 P8 (an ACP adapter is an engine plus a profile in `host`) | applied | 0.3 later adapters; Stage 3 ACP engine; Deferred other agent tools |  |
| S7 P9 (client capabilities to advertise) | applied | 0.3 later adapters | No `fs`, `terminal` or `elicitation` in v0. |
| S7 change 1 (0.3 first `AgentAdapter` bullet; Stage 2 outlines) | applied | 0.3 two core traits; Stage 2 `core` outlines | Stage 2 names P1–P9. |
| S7 change 2 (capability table, Yes/No/Unknown, refuse a role that needs No or Unknown) | applied | 0.3; Stage 3 |  |
| S7 change 3 (launch line comment; `model.id`) | applied | 0.3 |  |
| S7 change 4 (ACP stop: `session/cancel`) | applied | 0.3 stopping a phase |  |
| S7 change 5 (`empty_turn` for protocols with no failure flag) | applied | 0.3; Stage 3 criteria |  |
| S7 change 6 (0.2 shim: settle rule for ACP; log stdin) | applied | 0.2 shim; 0.3 settle rule; task ts-0016-ka |  |
| S7 change 7 (`limited` only with `limit_signal`) | applied | 0.3 usage and rate limits |  |
| S7 change 8 (budget native or host-enforced) | applied | 0.3 |  |
| S7 change 9 (partial usage shown as "at least"; cost `None`, not 0) | applied | 0.13 new agent-stream row; Stage 3 criteria |  |
| S7 change 10 (permission with unknown input; no input-based auto-allow) | applied | 0.3; 0.11 Queue item; Stage 3 criteria |  |
| S7 change 11 (Stage 1 marks S7 done; criterion met) | applied | Stage 1 S7 bullet and criteria |  |
| S7 change 12 (second `AgentSession`: scripted fake ACP child) | applied | Stage 2 test doubles; Stage 3 ACP engine and criteria; task ts-0022-mc |  |
| S7 change 13 (Deferred: ACP engine plus profile, opencode first, probing, Unknown) | applied | Deferred, other agent tools and model sources |  |
| S8 rec 1 (sample at 1 s; 250 ms optional; never below 250 ms by default) | applied | 0.13 process sampling row; Stage 13; Risks |  |
| S8 rec 2 (sampling is the series, `wait4` totals are the truth; say short processes are missed) | applied | 0.13 process sampling row | Honest wording of S8 owner extra (c). |
| S8 rec 3 (disk bytes from `proc_pid_rusage`, never `wait4`) | applied | 0.13 process sampling row; Stage 1 S8 item |  |
| S8 rec 4 (one streaming `nettop`, stdin open, read through a pty) | applied | 0.13 `nettop -P` row; Stage 13 |  |
| S8 rec 5, poll GPU every 5 s; label "GPU busy seconds"; per-process counters only when a local model is declared | applied | 0.13 GPU row; Stage 13 |  |
| S8 rec 5, gate the device GPU poll on a declared local model | applied | 0.13 GPU row and `full` bullet; Stage 13 items and criteria; Open questions item 8 | Owner chose it on 2026-11-14 over revision 10's poll in every `full` run. Stage 13's Metal test now uses a fixture pack that declares a local model. |
| S8 rec 6 (Docker API one-shot at 1 s or slower; `metrics.processes` for identities; always include lane containers via the API) | applied | 0.13 Containers row; Stage 13 | Owner extra (a). |
| S8 rec 7 (sleep: two-clock formula, 2 s threshold) | applied | 0.13 sleep formula; Stage 13 | The mid-run test is deferred (below). |
| S8 rec 8 (about 34 bytes per sample row) | applied | 0.13 volume and retention |  |
| S8 rec 9 (tail by polling every 50 ms; 4 KB, 100 lines/s burst 1,000, 16 MiB) | applied | 0.5 report caps; task ts-0010-yb |  |
| S8 rec 10 (telemetry spans for the breakdown when present, the stream otherwise) | applied | 0.11 tool-call breakdown; 0.13 telemetry bullet; Stage 13 |  |
| S8 change 1 (0.13 process sampling row) | applied | 0.13 Sources table |  |
| S8 change 2 (0.13 `nettop -P` row) | applied | 0.13 Sources table |  |
| S8 change 3 (0.13 GPU row: cost, counter ends with the process) | applied | 0.13 Sources table |  |
| S8 change 4 (0.13 "Sleep is not work": 2 s on 1 s ticks, validated, mid-run test an owner step) | applied | 0.13 sleep bullet; Deferred | Mid-run test deferred (below). |
| S8 change 5 (0.13 volume and retention) | applied | 0.13 |  |
| S8 change 6 (0.13 telemetry: traces beta flag for headless phases needing exact timing) | applied | 0.13 telemetry bullet; Stage 13; Deferred telemetry bullet rewritten | Default-on trigger is the traces flag leaving beta. |
| S8 change 7 (run view tool-call breakdown, three choices) | applied | 0.11 tool-call breakdown | Owner extra (b). |
| S8 change 8 (0.5 report file caps) | applied | 0.5 |  |
| S8 change 9 (Stage 1 S8 status) | applied | Stage 1 S8 item and status line |  |
| S8 owner extra (a) (lane containers always counted, listed via the Docker API, not the CLI) | applied | 0.13 Containers row; Stage 13 | Written as always included under `full`, with no setting to leave them out (interpretation). |
| S8 owner extra (b) (the three breakdown choices) | applied | 0.11 tool-call breakdown | 1 s gap threshold with an "unattributed" row; Agent self time with nested rows; share is the sum of rows plus an exclusive "of wall" column. |
| S8 owner extra (c) (honest sampling wording) | applied | 0.13 process sampling row | 1 s sampling missed 33–73% of processes but caught 93–100% of CPU. |
| S8 left unmeasured 2, sleep mid-run test (owner step) | deferred | Stage 13; Deferred | Trigger: before Stage 13 (Full metrics) starts. |
| S8 left unmeasured 1, a run measured from its start (container startup cost) | deferred | Stage 13; Deferred | Trigger: before Stage 13 (Full metrics) starts. |
| S8 left unmeasured 3 (kqueue exit capture for missed processes; a human permission wait in telemetry; GPU under a local model with several clients; an Intel Mac) | noted, no plan change | 0.13 process sampling row (CPU converted with `mach_timebase_info`, never a constant) | Not recommendations. No stage depends on them; the Intel timebase is already handled. |
