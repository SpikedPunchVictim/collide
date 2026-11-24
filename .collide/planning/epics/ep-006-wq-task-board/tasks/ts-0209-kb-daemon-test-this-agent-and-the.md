---
id: ts-0209-kb
kind: task
status: ready
title: "Daemon: Test this agent and the confinement probe"
owners: []
links:
  - { type: blocked-by, to: ts-0208-dx }
---

## Goal

`ask.test` runs the smallest ask and reports what happened, and the confinement probe finds an agent whose own
settings let it write and run commands. A red agent is refused as the ask agent until the person acknowledges it,
and `query` withholds untrusted bodies from it.

What exists.
- `core/src/ask.rs`: `AskTestParams`, `AskTestDaemonParams`, `AskTestResult` (endpoint reached, first tool call,
  finish, cost, folders, tool kinds, failure, probe) and `ConfinementProbe` (red, green, "not tested").
- The shell forwards `ask.test` with an ask token (`ts-0201-us`). The ask path for Claude and ACP (`ts-0201-us`,
  `ts-0208-dx`). The withheld-body hook of `query` (`ts-0207-en`).

What this adds.
- The daemon's `ask.test`: the agent is told to call `query` once for one task and say done. The result: whether
  the endpoint was reached with the ask's token, the time to the first tool call, the finish time, the cost (Claude)
  or "not reported" (ACP), the readable folders and tool kinds in effect, and a plain failure text, such as
  `claude was not found on PATH` or `the agent did not call a tool within 180 s`.
- The probe, in the same test: a temp folder made for it; the agent is asked to write a file there and to run
  `true`. Red when the file exists, or the stream shows a completed edit or command that Collide did not refuse.
  Red reads "this agent is not confined; its own settings let it write and run commands".
- The last result per entry, kept in the ask records; "not tested" when there is none.
- A red entry is refused by `ask.start` with "this agent is not confined; acknowledge it in Agents first", until
  the person acknowledges it. The acknowledgement is recorded with the entry, in the user layer only. While a red
  entry is in use, the ask's registration marks its bodies withheld.
- The fake ACP child gains two steps: write a file, and run a command.

## Success criteria

- Over fake Claude: Test reports the endpoint reached, the time to the first tool call, the finish time and the
  cost. Over the fake ACP child: the cost is "not reported".
- Each failure of the test names its cause: the command missing, no tool call within the timeout, the endpoint not
  reached.
- Over a fake agent that writes a file and runs `true`, Test is red; over one that does neither, green.
- A red entry is refused as the ask agent until acknowledged, and accepted after; a repo layer cannot acknowledge.
- `query` withholds untrusted bodies for a red agent's ask.
- `cargo test --workspace` and `pnpm test` pass.

## Tests

Write these first.
- `host/tests/`: `ask.test` over fake Claude and the fake ACP child; the three failures; the probe red and green;
  the refusal and the acknowledgement; the repo-layer refusal; the last result kept and read.
- `app/test/`: a red ask's `query` with `fields` asking for a body returns none.

## Notes

- IMPLEMENTATION_PLAN.md:
  - 0.7: Test this agent; The probe for confinement. 0.3: ACP profile (opencode's default is unrestricted).
  - 0.11: `query` withholds untrusted bodies for a red agent. Stage 6b: Success Criteria (the Agents page; the
    probe and the listener). Part L.
- **Decision for the item, with a recommendation: where the acknowledgement lives.** Recommend a new agent field,
  `acknowledged_unconfined` (a date), user layer only, written through `config.set`; `repo_file_refusals` refuses
  it in a repo. Alternative: a daemon file of acknowledgements; it is not shown with the entry's other fields.
- **Decision for the item, with a recommendation: how a red ask's bodies are withheld.** Recommend the daemon's
  answer to `ask.start` carrying the flag, and the shell's registration passing it in each `ui_tools.request`'s
  `caller`. Alternative: the window asks the daemon; a second round trip per call.
- **Owner's choice, recommendation taken (2026-11-24): probing Claude.** Recommend running the probe for Claude too: its
  line has no write or command tool, so a green result checks the line. Alternative: Claude marked "not needed".
- Area: `host/src/daemon.rs` and the ask runner, `core/src/ask.rs`, `core/src/settings.rs`, the shell, the fake
  ACP child, `host/tests/`, `app/src/ui/`.
- Dependencies: none new.
- The test rule: no test touches the real home, real launchd, a real daemon, the keychain or the network. Each test
  that runs the `collide` binary clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to
  a temp root, as `host/tests/common/mod.rs` `TempRoot` does. A temp repository sets `GIT_CONFIG_GLOBAL` and
  `GIT_CONFIG_SYSTEM` to empty files. Keychain writes go through `MemorySecretStore` (`host/src/secret_store.rs`),
  never the login keychain. Browser tests run the installed Chrome with a temp profile (`app/browser/support/
  chrome.ts`), and the bridge's daemon on a temp root (`app/browser/support/world.ts`), with fake agents only. A test
  never stubs or bypasses a safety gate: the real-agent opt-in, the confinement probe, `env_clear`.
- Stage 6b item (revision 26 scope, split 2026-11-24). Items of Stage 6b run one at a time, in id order, because they
  share `app/`, `core`, `host` and the generated files. No builder runs `tauri build`.
- Nothing here runs a real `claude`, `opencode` or any model, or spends money.
- Owner decisions (2026-11-24): the owner took the recommendations at the split.
