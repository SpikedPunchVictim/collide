# S6: interface prototype

A throwaway, clickable prototype of Collide's Field, Tracks and Queue, plus Compare and Experiments, over fake data.
It exists to run usability sessions (see `SESSION-PROTOCOL.md`). By default it talks to no agent, no repository and no
network. The one exception is the in-app agent below, which only runs when you ask it and only on this machine.
Results and decisions are in `record.md`.

## Install and run

Needs Node 24 and pnpm 11. Nothing is installed globally. The package store is local (`.npmrc`, `.pnpm-store/`).

```sh
pnpm install
pnpm dev          # http://127.0.0.1:5173/
```

Other scripts: `pnpm test` (pilot walkthroughs, UI tests, and the in-app agent: reducer, tools, MCP round trip,
settings endpoint), `pnpm typecheck`, `pnpm build`.

## In-app agent

The dev server also hosts an MCP endpoint, `http://127.0.0.1:5173/mcp` (streamable HTTP, this machine only, no
sign-in), with five tools: `query`, `set_view`, `show_groups`, `highlight`, `open`. None launches, accepts, edits or
deletes. The open page runs each call, so keep a tab open at `http://127.0.0.1:5173/` while an agent works. The demo
data for it (a pantry-tracker epic E-09 with 12 ready tasks, nine finished runs, and the repository they are about in
`fixtures/repo/`) is loaded with the app.

- **Ask box** (top bar): type a request, for example "Show me the tracks that have taken the longest in their build
  portion" or "Can you show me tasks that can be grouped together, where code changes would affect similar files?".
  Each tool call shows as a line, then a chip "Agent: <title>" appears with Undo and Save as view (or Save snapshot
  for a grouping). Needs `claude` on the PATH (Claude Code) for the default agent; it runs `claude -p` with haiku,
  a $0.25 budget and the Collide tools plus Read, Grep and Glob on `fixtures/repo` only.
- **Settings, "Agents"** (key 5): the agents you bring. Kinds: Claude Code, ACP agent (for example opencode:
  command `opencode`, arguments `acp`), External. Saved to `out/agents.json` (git-ignored).
- **Connecting an external agent.** Add an External entry on the Agents page; it shows the endpoint and snippets to
  copy. Collide launches nothing for it and writes to no agent's config:

  ```sh
  claude mcp add --transport http collide http://127.0.0.1:5173/mcp
  ```

  or, for any MCP client, `{ "mcpServers": { "collide": { "type": "http", "url": "http://127.0.0.1:5173/mcp" } } }`.
  For opencode, `opencode.json` in this folder has a project-local `mcp` entry and a local Ollama model, as in S7;
  keep opencode's state out of your own config by pointing `HOME` and the `XDG_*` variables at a folder under `work/`.
- Every ask is logged with its argv and timings to `out/asks/` (git-ignored).

## Facilitator mode

Facilitator mode is off by default. Turn it on with either:

- the URL flag `http://127.0.0.1:5173/?facilitator=1`, or
- Ctrl+Shift+F in the page (toggles it).

`?scenario=4` (or any scenario id) also preloads that scenario. The timer still waits for Start.

The panel holds: participant id, scenario picker with the task card, Start and Stop, an error counter, notes, the
active variant for each open question, speed and pause, the attempts table, and the log export (Copy JSON, Download,
Save to out/sessions).

| Key | Action |
| --- | --- |
| Alt+S | start or stop the timer |
| Alt+E | add an error |
| Ctrl+Shift+F | show or hide facilitator mode |

## Keys in the app

| Key | Where | Action |
| --- | --- | --- |
| 1 2 3 4 | everywhere | Field, Tracks, Queue, Experiments |
| j k | Queue | next, previous item |
| a d r | Queue | the item's accept, deny or retry-style action (what they do depends on the kind and the variant) |
| o | Queue | open the item's run (variant: track at the moment, track, or output) |
| i | Queue | show or hide the context view |
| u | Queue | undo the last answer |
| Esc | everywhere | disarm, close the context view, or go back |
| [ ] = - | Tracks | pan and zoom |

## Layout

- `src/reducer.ts`: one pure reducer. Every user action goes through it, so pilots run without a DOM.
- `src/sim.ts`, `src/gen.ts`: the fake run clock and the scripted agent runs.
- `src/corpus.ts`: the invented task corpus (140 tasks, epics E-01 to E-08, branches).
- `src/scenarios.ts`: the nine scenarios and three probes, with start states and end-state detectors.
- `src/items.ts`: Queue kinds, their `a`/`d`/`r` tables per variant, and the context view contents.
- `src/ui/`: React components.
- `src/agentReducer.ts`, `src/agentDemo.ts`, `src/agent/`: the in-app agent's reducer actions with Undo, its demo data,
  the pure tool runner (`exec.ts`) and the page bridge (`page.ts`).
- `server/`: the Vite plugin: MCP endpoint, bridge to the page over the HMR websocket, agent launchers, settings file.
- `fixtures/repo/`: the invented repository the demo tasks are about.
- `test/`: pilot walkthroughs (`pilot.test.ts`), UI tests (`ui.test.tsx`), protocol check (`protocol.test.ts`), and for
  the in-app agent `agent.test.ts`, `agent-ui.test.tsx`, `mcp.test.ts`, `agents-server.test.ts`.
- `out/sessions/`: saved session logs from the dev server (git-ignored). Copy logs worth keeping to `sessions/`.

## Not built

Command palette and the Plan menu, the Planning view, a virtualised output list, a real New session flow and pack
catalogue, a real settings system (see `record.md`). All data is invented; no name or id is taken from a real project.
