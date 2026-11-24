---
id: ts-0210-mz
kind: task
status: ready
title: "App: the Agents page in Settings"
owners: []
links:
  - { type: blocked-by, to: ts-0209-kb }
---

## Goal

Settings gets its Agents page: a card per entry with each field's source, the words for folders, tool kinds, budget
and timeout, Test this agent with the probe, the choice of the ask agent, last use and spend this week, and Claude's
two version chips.

What exists.
- Settings (`ts-0193-gk`), whose Agents tab says "This page arrives with the in-app agent, in Stage 6b."
- `core/src/agent_layers.rs` (`AgentView` with each field's layer) and `core/src/settings.rs` (agent entries, the
  checks, `ask.agent`, `ask.budget_usd`, `ask.timeout_s`); `config.get` and `config.set`, which take
  `agents.<id>.<field>`.
- The notice for a repo budget an ACP agent cannot enforce: on the wire, not shown (`ts-0105-jv`).
- `daemon.agent_versions` (`host/src/claude_versions.rs`, `ts-0068-wx`): the version in use and the published one.
- `ask.test`, the probe and the acknowledgement (`ts-0209-kb`); the listener port in the ask record (`ts-0208-dx`);
  the ask records (`ts-0201-us`). The results of `ts-0198-tf`.
- The mock: `agentsHtml`, `agentCardHtml`, `testAgent`, `pickAskAgent`; notes 55 and 56. S6's Agents page
  (`src/ui/Settings.tsx`).

What this adds.
- A card per entry, for Claude Code, ACP and External (note 55): each field with its layer; the folders and tool
  kinds in words ("starts in this folder", or "can read only" if `ts-0198-tf` showed reads confined), and for an
  ACP agent that its own settings decide what it can really do; budget and timeout with their sources and "budget:
  Claude Code only; ACP and External agents get a timeout only"; the repo-budget notice; Details.
- The choice of the ask agent (`ask.agent`); adding an ACP or External entry, and removing one, through
  `config.set`; a settings error shown with its text.
- Last use and spend this week per entry, from the ask records, through one new read method.
- Claude's two version chips, from `daemon.agent_versions`.
- Test this agent (note 56): the result's fields, the failure text, and the probe. A red result shows its words and
  Acknowledge; the ask box shows a standing warning while a red agent is in use. A port an ACP agent opened is named.

## Success criteria

- Over a fake owner: each field shows its layer; the budget and timeout words are on the page.
- Test this agent against a fake agent reports the endpoint reached, the time to the first tool call, and the cost
  or "not reported"; each failure of the test names its cause; a red result offers Acknowledge, which records it.
- Choosing the ask agent writes `ask.agent`; the ask box names it.
- Last use and spend this week show from records; the version chips show from `daemon.agent_versions`.
- In a browser, through the bridge, an ACP entry pointing at the fake ACP child is added and tested.
- `pnpm test` and `cargo test --workspace` pass.

## Tests

Write these first.
- `app/test/`: the cards, the layers, the words, the choice, add and remove, the errors, Test and its failures, the
  red result and Acknowledge, the standing warning, the version chips, last use and spend.
- `host/tests/`: the usage method over records on a temp root, with a week's boundary.
- `app/browser/`: add an ACP entry and test it, over the fake ACP child.
- The S6 ports, to this page's save model: `agent.test.ts` "settings actions"; `agent-ui.test.tsx` "an edit marks
  the settings unsaved and the ask box names the chosen agent"; `mcp.test.ts` "settings endpoint" as `config.set`
  and `config.get` round trips, and as the settings checks' texts (a bad budget, a missing folder, External as
  the ask agent, a bad env line).

## Notes

- IMPLEMENTATION_PLAN.md:
  - 0.7: agent entries and the Agents page (layers, words, Test, the probe, last use and spend). 0.3: Two version
    chips. Stage 6b: The Agents page; Success Criteria (the Agents page). Part L.
- Builds on `ts-0193-gk` (Settings), and names it. Carried: `ts-0105-jv`'s notice.
- **Decision for the item, with a recommendation: the usage method.** Recommend `ask.usage`, answered by the daemon
  from the records: per entry, the last use and the spend of the last seven days, with the cost's quality. The
  shell passes it on unchanged. Alternative: fields on `config.get`'s agent rows; it mixes records into settings.
- **Owner's choice, recommendation taken (2026-11-24): a settings change on this page.** Recommend each field written
  with `config.set` when it is set, as the Secrets tab of `ts-0193-gk` writes a secret, with no unsaved state.
  Alternative: S6's unsaved state and one Save.
- Area: `app/src/components/settings/` (a new agents part), `app/src/components/ask/`, `core/src/ask.rs`,
  `host/src/daemon.rs`, the shell, the generated files, `app/test/`, `app/browser/`.
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
