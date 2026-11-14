---
id: ts-0006-ze
alias: R5
kind: task
status: review
title: "Adversarial review of revisions 10–12 (R5)"
owners: []
links:
  - { type: blocked-by, to: ts-0005-st }
---

## Goal

Before the skeleton starts, independent reviewers (Fable and Opus, as in R1–R4) attack the finalized plan: contracts that cannot hold, stages in the wrong order, missing failure modes, cost and scope. Part L's planning-first slice and item format are in scope.

## Success criteria

- Each finding is labelled core or pack, with its plan location.
- The owner's decision on each finding is applied, rejected with a reason, or deferred with a trigger.
- A review-log entry names the stages changed.

## Notes

Owner, 2026-11-13. Reviewers are read-only and get the plan and records, not our conclusions.

## Ledger (revision 13)

One row per finding and challenge in the two R5 reports (Fable F1–F25 and challenges 1–5, Opus F1–F20 and challenges
1–5), with the owner's decisions of 2026-11-14. Plan sections and stages are cited by name. Where two reviewers made
the same finding, each has a row and the change is written once. B and M numbers are those of the merged list.

Totals: 55 rows. Applied 51 (4 of them in part, with the other part rejected or left to measurement), rejected 1,
deferred 1, considered and not chosen 2.

| Source | Disposition | Where | Reason or trigger |
| --- | --- | --- | --- |
| Fable F1 (session replay cannot take its recorded inputs) | applied | 0.2 shim and journal; 0.3 `AgentSession` (`feed`, `replay`); Stage 2 contracts, criteria, tests; Stage 3a | One ordered journal of `{seq, wall_ns, dir, bytes}`, written before the child sees the bytes, and framed daemon-to-shim messages (M1). |
| Fable F2 (`wait` hook has no "not yet") | applied | 0.5 result enum; 0.7 `wait` phase; Stage 2; Stage 8 | `pending` with `retryAfter`, `interval:` and `timeout:`; a pending poll replaces its run-state entry (M2). |
| Fable F3 (socket token and "only a person") | applied | 0.1; 0.2 socket and "Who is asking"; 0.10 person-only ops; 0.11 Plan…; Stage 2; Stage 3a | Actor from peer credentials, process tree and marker; `0700` directory and uid check; CLI token file; the same-user rule restated as a guardrail (M3). |
| Fable F4 (Stage 2 items are revision 11; two contradict the plan) | applied | 0.10 spike kind and build-task rule; Stage 2 slice, criteria, Status; Stage 4c; four spike records `concluded` | The ep-002 item files are left as they are and the Stage 2 Status says they are to be brought to revision 13 (M4). The slice reads `epics/` only (criterion narrowed, records not made read-only items). |
| Fable F5 (`query` passes Claude's MCP output cap) | applied | 0.11 `query` row; Stage 6b criteria and tests | No body by default, `fields`, cursor, 20K-token page, a fixture of about 1,500 tasks (B2). |
| Fable F6 (the ask line does not confine reads) | applied | 0.3 ask line, `--add-dir`, `--permission-prompts none`, "starts in this folder"; Stage 6b | Measured in Stage 6b; the page does not say "can read only" until then (B3). |
| Fable F7 (external client cannot connect while the window is closed) | applied | 0.2 endpoint; 0.9; Stage 2; Stage 6b | Shell answers `initialize`, `ping`, `tools/list` from a manifest, 202 and 405, forwards only `tools/call`, counts the cap, fails with an "open Collide" message (M7). |
| Fable F8 (Stages 3, 6 and 9 are each several stages) | applied | Stages 3, 4, 6 and 9 split into 3a–3d, 4a–4c, 6a–6c, 9a–9d; 0.10 build-task rule; 0.15 | Merged with Opus F10; no renumbering. Stage 4's agent-secrets launch test moved to 6b (S1). |
| Fable F9 (a full disk is handled for Collide's files, not the child's) | applied | 0.2 durability; Stage 3a | Free-space check, `paused: disk`, cause `disk`, stream-line and log caps (B8). |
| Fable F10 (no rule says which clock each timer uses) | applied | 0.2 clock table; 0.13; Stages 3a and 3b | Uptime for work timeouts, continuous time for the cache countdown, wall time for display and limit reset; the owner's sleep test moves to Stage 3b (B9). |
| Fable F11 (a "pinned version" of Claude has no mechanism) | applied | 0.3 version range, smoke check; Stage 3b; Risks | Tested range, version in the fingerprint, unknown events counted as Other (B6). A managed copy is deferred (Opus C5). |
| Fable F12 (the slice does not say which checkout it writes) | applied | 0.10 slice bullet; Stage 2 slice and criteria | Main checkout's shared root by default, `--checkout`, `--commit`, a claim refused when an owner is shown (M9). |
| Fable F13 (string and file caps collide with real bodies) | applied | 0.9 list caps; 0.10 frontmatter cap; Stage 2 tests | Caps on CEL-bound fields only; the frontmatter block capped, not the file (B18). |
| Fable F14 (id uniqueness is a probability) | applied in part | 0.10 ids; Stage 2 tests; Stage 4c | Seeded draw test, forced-collision redraw, `collide task reid` (B20). The three-letter suffix is rejected: the owner's rule of 2026-11-13 (two letters) stands. |
| Fable F15 (`git:` packs fetched before trust) | applied | 0.7 `git:` fetch rules; Stage 4a | Ask before a new host, `GIT_TERMINAL_PROMPT=0`, protocol allowlist, `--`, timeout, refuse a URL starting `-` (B14). |
| Fable F16 (`/dev/full` cannot run on the first platform) | applied | 0.2 durability; Stage 3a | An `ENOSPC`-injecting writer replaces `/dev/full` (B8). |
| Fable F17 (repo file's agent keys address ids it cannot know) | applied | 0.7 agent entries; Stage 4a | The repo sets `ask.budget_usd` and `ask.timeout_s` defaults, not per id; an unenforceable budget is ignored with a notice (B19). |
| Fable F18 (reported spans primary, reported metrics not) | applied | 0.5; 0.14; Stages 7 and 15 | Only a hook's own report lines, through a descriptor its children do not inherit, may be primary (B15). |
| Fable F19 (ACP model mapping names only the legacy method) | applied | 0.3 models; Stage 3d | `configOptions` and `session/set_config_option` first, `session/set_model` as the fallback (B16). |
| Fable F20 (planning path sits inside the protected directory) | applied | 0.9 protected files; 0.10 layout refusal; Stage 4c | Protected files are named; `.collide/` as a whole is not (B17). |
| Fable F21 (ACP read rule needs canonical paths) | applied | 0.3 ACP profile; Stage 6b | `realpath` of both the request's paths and the readable folders (B5). |
| Fable F22 (an ask outlives its app) | applied | 0.2 ask child; Stage 6b | The daemon cancels an ask when the starting connection drops (B13). |
| Fable F23 (owner walkthroughs cannot fail) | applied | Stages 6a, 6b, 9a, 9b | Each walkthrough lists what to record: the owner's time per scenario beside S6's advisory limit, errors, and what was awkward (B21). |
| Fable F24 (small contradictions) | applied | 0.7 layers; 0.8; Deferred; revision 10 summary; Review log | Detectors: seven. Layers: four. `owners`, not `assignee`. The R5 task file renamed to match its title (B22). |
| Fable F25 (daemon start and environment edge cases) | applied | 0.2 starting the daemon and base environment; 0.13 fingerprint; Stage 3a | `setsid` fallback with a warning over SSH, login environment recaptured at app start, `apiKeySource` in the fingerprint (B23). |
| Fable challenge 1 (a thin loop before the adapters) | applied | Stage 3c; Milestone A; Stage 9a; header | Owner, 2026-11-14. Names what it pulls forward from Stages 4b, 6, 7, 8 and 9a, and its cost (launch and accept written twice, replaced in 9a). Nothing cut. |
| Fable challenge 2 (a stdio bridge instead of the HTTP listener) | considered and not chosen | 0.2 endpoint (one line); Risks; Review log | Owner, 2026-11-14: the endpoint stays in the app shell with the M7 fixes. The shell is the only process that can answer a screen-changing call. |
| Fable challenge 3 (the markdown provider in Rust) | rejected | Stage 2 planning slice (one line) | Owner, 2026-11-14: Stage 2 must prove the TypeScript provider path, the extension model's main risk. |
| Fable challenge 4 (`--restricted` or `--bare` as the ask line's base) | applied | 0.3 `--restricted` and `--bare` bullet; Stage 6b | Handled by B3. `--restricted` is measured under `-p` in Stage 6b and adopted if it confines reads. `--bare` is not adopted: it changes how the login is read. |
| Fable challenge 5 (GitHub Projects v2 after the board) | applied | Stage 5 (Issues only); Stage 6c; 0.15 | Owner, 2026-11-14. Stage 2 still freezes the provider contract in full, Projects keys included. |
| Opus F1 ("integrated" undefined for PRs) | applied | 0.4 accept row and `integrating`; 0.5; Stages 2, 8, 11 and 12 | `integrate` answers `merged {commit}` or `pending {url, head}`; the run is `integrating`; group readiness is the merge commit being an ancestor of the line ref (M5). |
| Opus F2 (a crash between non-idempotent steps) | applied | 0.2 intent and outcome bullet; Stages 2, 7 and 8 | Intent and outcome events; restart checks the world; unknown outcome goes to the Queue; hooks run under a shim; SIGKILL-after-intent tests (M6). |
| Opus F3 (the webview renders untrusted text with full `rpc`) | applied | 0.9 webview bullet; 0.11 drawer; Stage 6a | CSP, sanitized markdown, no `javascript:` or `file:` links, capabilities limited to `rpc` and one event channel; hostile-issue test (B1). |
| Opus F4 (opencode's default is unrestricted) | applied | 0.3 opencode; 0.7 Test this agent; 0.11 `query`; Stage 6b | Probe (write a file, run `true`); red means refused as an ask agent until acknowledged; untrusted bodies withheld from `query` (B4). |
| Opus F5 (CLI token, caller identity, person-only rule) | applied | 0.2; 0.10; Stage 2; Stage 3a | Same change as Fable F3 (M3). |
| Opus F6 (nothing notices Claude's version changing) | applied | 0.3; Stage 3b; Risks | Same change as Fable F11 (B6). |
| Opus F7 (Claude silently ignores an invalid settings file under `-p`) | applied | 0.3 settings canary; Stage 3b | Schema check in the contract test and a runtime canary, else the phase fails with "Claude ignored Collide's settings" (B7). |
| Opus F8 (`ui_tools` contract lacks the caller and breaks MCP transport rules) | applied | 0.2 endpoint; 0.9; Stage 2; Stage 6b | `ui_tools.request {requestId, caller, deadline, message}`; 202 for notifications; 405 for GET and DELETE (M7). |
| Opus F9 (Stage 2 items leave out revision 12's contracts) | applied | Stage 2 Status and contracts | The item files are not edited here. The Status says they are to be brought to revision 13 (M4). |
| Opus F10 (Stages 4, 6 and 9 too big) | applied | Stages 3, 4, 6 and 9 split | Same change as Fable F8 (S1). |
| Opus F11 (the slice cannot read the spike records; statuses undefined) | applied | 0.10 `spike` kind; Stage 2 slice; Stage 4c; four spike records | `spike` kind with `planned`, `running`, `results`, `concluded`, `dropped`; four records set `concluded`; the slice criterion narrowed to `epics/`, records not made read-only items (M4, B22). |
| Opus F12 (provider writes neither atomic nor locked) | applied | 0.10 flock bullet; Stage 2 slice, criteria, tests | Per-root `flock`, temp, `fsync` and rename; a concurrent `setStatus` test (M8). |
| Opus F13 (the id-collision criterion fails at random) | applied in part | 0.10 ids; Stage 2 tests; Stage 4c | Same change as Fable F14 (B20). The three-letter suffix is rejected. |
| Opus F14 (the ask line reads only one folder) | applied | 0.3 ask line; Stage 6b | `--add-dir` per extra folder; the criterion is "equals the adapter's pinned line" (B3). |
| Opus F15 (the OTLP receiver is an undeclared listener) | applied | 0.2 OTLP paragraph; 0.13; Stage 9b | Daemon-hosted, `127.0.0.1`, `Host` check, per-run token, body cap; the user's endpoint detected before the strip (B10). |
| Opus F16 (snippets and interactive sessions break "run agents have no access") | applied in part | 0.7 interactive-launch MCP rule; Risks; Stage 9b | Interactive launches pass the snapshot's `--mcp-config` with `--strict-mcp-config`; a per-client call limit (B11). Named per-client tokens are rejected: one rotatable token is kept. |
| Opus F17 (no rule says which clock each timer uses) | applied | 0.2 clock table; Stage 3a | Same change as Fable F10 (B9). |
| Opus F18 (`opencode acp` can open its own listener) | applied | 0.3 listener check; Stage 6b | `lsof` by pid; the profile refuses `--mdns`, a non-loopback `--hostname` and `--cors`; the page names any port (B12). |
| Opus F19 (the repo file sets agent keys for ids a developer lacks) | applied | 0.7 agent entries; Stage 4a | Same change as Fable F17 (B19). |
| Opus F20 (the slice's built-in pack has no loading path) | applied | 0.4; 0.6; Stage 2 slice | Built-in packs are compiled into the app bundle with their own SDK copy, with no materialization or trust prompt; `node` from the captured environment, with a clear error (M10). |
| Opus challenge 1 (UI-tools endpoint and OTLP receiver in the daemon) | considered and not chosen | 0.2 endpoint (one line); Risks; Review log | Owner, 2026-11-14: the endpoint stays in the app shell. The OTLP receiver is the daemon's (B10). |
| Opus challenge 2 (`--restricted` plus `--add-dir` for the ask line) | applied | 0.3; Stage 6b | Handled by B3, as Fable challenge 4. |
| Opus challenge 3 (caller identity from peer credentials) | applied | 0.2; 0.10; Stage 2; Stage 3a | Handled by M3. |
| Opus challenge 4 (ACP ask agents under a config Collide writes, with a probe) | applied in part | 0.3 opencode; Stage 6b | The probe is adopted (B4). Whether a Collide-supplied permission configuration changes the result is measured in Stage 6b, and the ask line does not depend on it until it is. |
| Opus challenge 5 (a Collide-managed copy of Claude) | deferred | Deferred | Trigger: a Claude update breaks a run despite the version-range check and smoke check (B6). |
