---
id: ts-0214-rz
kind: task
status: ready
title: "Owner check: the in-app agent walkthrough"
owners: []
links:
  - { type: blocked-by, to: ts-0213-as }
---

## Goal

The owner's walkthrough of Stage 6b, in the bundled app, recorded as owner judgement: the asks, Undo, snapshots, the
Agents page, the keychain, Rotate and an external client, and a window closed during an ask. It also covers what
only the bundled app shows: the endpoint token in the login keychain and the real window's close.

A build agent never runs it. It never runs `tauri build`, never starts the app or a daemon against the owner's data,
and never runs `claude` or `opencode`. It writes the command lines and a short checklist under `### Owner
checklist`, from the code and from the Built sections of `ts-0199-xz` to `ts-0213-as`. The owner runs them and
records the result here as owner judgement, with what was awkward.

## Success criteria

- Ask the Field request on S6's fixture board and read the chip against the code for two groups; Undo. Owner-run.
- Save snapshot, restart the app, and open it. Owner-run.
- Ask "what should I work on next?" and read the lens. Owner-run.
- On the Agents page: add an ACP agent and Test it; read each field's source; put a key in the keychain and see
  that it is in no file; set a budget on the Claude entry and see the words for the ACP one. Owner-run.
- Rotate the token and connect an external client with a fresh snippet. Owner-run.
- Close the window during an ask and read the error. Owner-run.
- The endpoint token is in the login keychain under Collide's service, and in no file. Owner-run.
- The differences `ts-0205-qh` recorded are each closed or carried. Owner-run.
- The walkthrough records the time, the errors and what was awkward (R5), and the spend, within the cap.
- A line records the date Stage 6b is judged done by the owner.

## Tests

None by a build agent: this check is the owner's. The checklist names the automated tests it relies on, from
`ts-0199-xz` to `ts-0212-km`.

## Notes

- IMPLEMENTATION_PLAN.md: Stage 6b, Success Criteria (Owner walkthrough; the owner's walkthrough also lists what to
  record). R5. Part L.
- What the checklist covers. The build agent fills in the exact commands once the earlier items are built.
  - **Set up.** The owner builds the bundled app (`tauri build`) from `<collide checkout>`. S6's fixture board copied
    to `<scratch board>`. Say which steps use the real `HOME` and the login keychain, and how to remove what they
    leave (the throwaway key, the endpoint token if wanted, the external client's config).
  - **Asks.** The Field request; two groups against the code; Undo; Save snapshot; restart; open it; "what should I
    work on next?".
  - **Agents.** Add opencode as an ACP agent (with a local model, as S6 did, or the owner's choice); Test; the
    sources; a throwaway key; a budget on Claude; the ACP words.
  - **External.** Rotate; `claude mcp add` with the fresh snippet in a scratch folder; one `claude -p` that calls
    `query`; remove it.
  - **Close.** Close the window during an ask; the error.
  - **Record.** The times, the errors, what was awkward, the spend, and the date.
- **Owner's choice, recommendation taken (2026-11-24): the bundled app.** Recommend the bundled app, as `ts-0197-uj`
  did: the keychain and the window's close are real only there. Alternative: the dev app, with the keychain checks moved
  to a later owner check.
- **Owner's choice, recommendation taken (2026-11-24): spend cap $0.50, time 60 minutes.** About six asks with haiku,
  one external `claude -p`, and an ACP agent on a local model at $0.
- Area: this item's own text. No code.
- The test rule, for anything automated this check leans on: no test touches the real home, real launchd, a real
  daemon, the keychain or the network. Each test that runs the `collide` binary clears the environment
  (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a temp root, as `host/tests/common/mod.rs` `TempRoot`
  does. Keychain writes in tests go through `MemorySecretStore`, never the login keychain. The owner's own run here
  uses the bundled app, the real `HOME` and the login keychain by choice, where the checklist says so.
- Stage 6b item (revision 26 scope, split 2026-11-24). Items of Stage 6b run one at a time, in id order, because they
  share `app/`, `core`, `host` and the generated files.
- Nothing a build agent runs here starts a model. The owner's walkthrough does, within the cap.
- Owner decisions (2026-11-24): the owner took the recommendations at the split.

### Owner checklist

Written by the build agent once `ts-0213-as` is done.
