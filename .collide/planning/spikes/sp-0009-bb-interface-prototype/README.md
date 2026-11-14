# S6: interface prototype

A throwaway, clickable prototype of Collide's Field, Tracks and Queue, plus Compare and Experiments, over fake data.
It exists to run usability sessions (see `SESSION-PROTOCOL.md`). It talks to no agent, no repository and no network.
Results and decisions are in `record.md`.

## Install and run

Needs Node 24 and pnpm 11. Nothing is installed globally. The package store is local (`.npmrc`, `.pnpm-store/`).

```sh
pnpm install
pnpm dev          # http://127.0.0.1:5173/
```

Other scripts: `pnpm test` (pilot walkthroughs and UI tests), `pnpm typecheck`, `pnpm build`.

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
- `test/`: pilot walkthroughs (`pilot.test.ts`), UI tests (`ui.test.tsx`), protocol check (`protocol.test.ts`).
- `out/sessions/`: saved session logs from the dev server (git-ignored). Copy logs worth keeping to `sessions/`.

## Not built

Command palette and the Plan menu, the Planning view, a virtualised output list, a real New session flow and pack
catalogue, real agents. All data is invented; no name or id is taken from a real project.
