# S7: `AgentAdapter` against ACP (paper check)

Record: `.collide/planning/spikes/sp-0006-kq-agentadapter-acp/record.md`. Nothing here ships.

## What is here

| path | what |
|---|---|
| `driver.mjs` | A minimal ACP client over stdio (Node, no npm packages). It spawns `opencode acp`, speaks JSON-RPC and logs every message in both directions, with a monotonic timestamp. |
| `opencode.json` | The opencode config: one provider, `ollama`, at Ollama's OpenAI-compatible endpoint (`http://127.0.0.1:11434/v1`), model `qwen2.5:7b`. `edit` and `bash` permissions are `ask`. |
| `run.sh` | Resets `work/`, copies the config in and runs the driver. Output: `out/live-run.txt` and `out/live-run.jsonl`. |
| `out/live-run.jsonl` | The log of the live run. One line per message: `{t, dir, msg}` with `dir` one of `out` (client to agent), `in` (agent to client), `stderr`, `event` (spawn, exit, scenario markers). |
| `out/live-run.txt` | The driver's own printed summary. |
| `work/` | opencode's isolated state (`config`, `data`, `state`, `cache`, `home`), the scratch directory the agent works in, and `work/spec/` (the ACP schema files that the record reads, with `dump.mjs`). Not for committing. |

## Requirements

- `~/.opencode/bin/opencode` (1.1.56 was used; override with `OPENCODE_BIN`).
- A running Ollama server on `127.0.0.1:11434` with `qwen2.5:7b` already pulled. The run pulls nothing and starts no server.
- Node 24 (no dependencies).

## Isolation

The driver gives opencode a clean environment: `PATH` plus `HOME`, `XDG_CONFIG_HOME`, `XDG_DATA_HOME`,
`XDG_STATE_HOME` and `XDG_CACHE_HOME`, all under `work/`, and `OPENCODE_DISABLE_CLAUDE_CODE=1` so that it does not
read `~/.claude`. Nothing in the real home directory is read or written by opencode. (Checked by comparing the newest
file mtimes under `~/.config/opencode`, `~/.local/share/opencode`, `~/.local/state/opencode` and `~/.cache/opencode`
before and after; see the record.)

## Run

```
./run.sh                 # every scenario, in order
./run.sh init allow      # chosen scenarios
```

Scenarios: `init` (initialize, session/new, session/list), `allow` (a write that needs permission, answered
`allow_once`), `reject` (the same, answered `reject_once`), `cancel-perm` (cancel while a permission request is
pending), `cancel-run` (cancel 3 s into a running `sleep 47`, then a second turn), `config` (modes, models, `set_mode`,
`set_model`), `apierror` (a model Ollama does not have), `exit-pending` (close stdin with a permission request
pending), `exit` (close stdin while idle), `load` (new process, `session/load` of the first session, then a question
about its earlier content).

A turn on a cold 7B model can take about a minute; a warm full run takes about 70 s. Run it with nothing else using
`work/`: two overlapping runs share the scratch directory and the log and spoil both.

## Reading the ACP schema

`work/spec/` holds `schema.json` and `schema.unstable.json` from the `schema-v1.31.0` release of
`agentclientprotocol/agent-client-protocol` (published 2026-11-10), downloaded with
`gh release download schema-v1.31.0 -R agentclientprotocol/agent-client-protocol`.
`node work/spec/dump.mjs schema.json <Def> [<Def> …]` prints a definition in one line.
