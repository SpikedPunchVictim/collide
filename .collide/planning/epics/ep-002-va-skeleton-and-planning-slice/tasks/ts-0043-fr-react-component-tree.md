---
id: ts-0043-fr
kind: task
status: done
title: "React component tree"
owners: []
links:
  - { type: blocked-by, to: ts-0042-vn }
  - { type: blocked-by, to: ts-0037-vg }
---

## Goal

In `app/`, the React component tree for Field, Tracks, Queue, run detail (Output, Time breakdown, Review), Compare and Experiments, following S6's surfaces, with typed props and placeholder rendering, wired to the reducer of `ts-0042-vn`. The Queue item's context view renders the six sections of the context record, showing a missing one as `Not available in this view`. Bodies that need later stages are stubs with tags.

## Success criteria

- The app builds and renders each surface with placeholders.
- The Queue context view shows all six sections, and a section the record lacks shows `Not available in this view`.
- Every component's props are typed from the generated types, and `tsc --noEmit` passes.
- Every stub carries a tag that belongs to a stage.

## Tests

Write first: a component smoke test per surface (it renders with placeholder props); the context-view test with each section missing.

## Notes

- IMPLEMENTATION_PLAN.md, 0.11 (the surfaces; the context view is a data contract), 0.15 (Pass 1: the React component tree), Stage 2: Skeleton (the React tree for Field, Tracks, Queue, run detail, Compare and Experiments). S6's record: `.collide/planning/spikes/sp-0009-bb-interface-prototype/record.md`.
- Area: `app/` (components).
- Done when the criteria hold.
- Built:
  - UI base (plan 0.15, shadcn/ui; owner, 2026-11-15): Tailwind v4 through `@tailwindcss/vite` in `app/vite.config.ts` (plus an `@` alias to `app/src`, mirrored in `app/tsconfig.json`), `app/components.json` (style `radix-vega`, base colour `neutral`, CSS variables), `app/src/lib/utils.ts` (`cn` = `clsx` + `tailwind-merge`), and `app/src/index.css` (the theme: every colour a CSS variable, shadcn's neutral values, a system sans-serif and a monospace stack). A comment at the top of `index.css` says the values await the approachable mockup pass (0.11, "Who it is for"). `main.tsx` imports the CSS.
  - shadcn components, added with `pnpm dlx shadcn@4.22.2 add card tabs badge button` and reviewed: `app/src/components/ui/{card,tabs,badge,button}.tsx`. The CLI wrote `import { cn } from "cn"` (a third-party package) and added `cn` to `package.json`; I pointed the four imports at `@/lib/utils`, removed the `cn` package, and kept the helper the brief asked for.
  - Dependencies added to `app/` (all exact, all published at least two weeks before 2026-11-15): `tailwindcss` 4.3.3, `@tailwindcss/vite` 4.3.3, `clsx` 2.1.1, `tailwind-merge` 3.7.0, `class-variance-authority` 0.7.1, `radix-ui` 1.8.1 (the umbrella package the copied components import from). `tailwindcss` and `@tailwindcss/vite` are dev dependencies. `shadcn` itself is not a dependency (see decisions). `pnpm-lock.yaml` changed to match.
  - The tree, `app/src/components/`: `field/FieldSurface.tsx` (tasks by plain-word category, select then Launch, the empty board's way in), `tracks/TracksSurface.tsx` (a track of phase segments for each run, named by its id label, state in words, finished runs, and Compare), `queue/QueueSurface.tsx` (a list grouped by kind and a detail pane: the command with untrusted input flagged or "input not available until approved", the cache countdown, the answers) and `queue/ContextView.tsx` (the six sections), `run/RunDetail.tsx` with `OutputPane.tsx`, `TimeBreakdown.tsx` and `ReviewPane.tsx`, `compare/ComparePane.tsx`, `experiments/ExperimentsSurface.tsx`, `LensBanner.tsx` (the agent's lens and highlight), `words.ts` (plain words for every state, kind and answer, total over the generated tags so a new tag fails `pnpm typecheck`), `types.ts`, `placeholders.ts` and `placeholders-detail.ts` (placeholder props), `stub-actions.ts`.
  - Wiring to the reducer of ts-0042-vn: `app/src/AppShell.tsx` takes `state` and `dispatch` and shows the surface `state.screen.surface` names. A surface tab sends `user/surface_shown`, a task's Select sends `user/task_toggled`, Clear grouping sends `user/lens_cleared`, and a chip's Undo (shown only while `canUndo`) sends `user/undo`. The agent's lens, highlights, chips and `open` of a run on Tracks (which shows the run's detail) render from the state. `App.tsx` is `useReducer(reducer, ...)` over `AppShell`, started with the placeholder corpus loaded.
  - Props are typed from the generated types: `Task`, `QueueItem`, `RunSummary`, `PhaseSummary`, `OutputEntry`, `ContextRecord`, and the state, kind and answer tags are read off them in `types.ts` (the generated index exports a nested type only through its parent, and each parent under an `Output` name). Where no generated type exists the prop type is local and says so in a comment: the time breakdown row (Stage 9c's metrics), the review's report and findings (Stage 9a), and an experiment (Stage 15).
  - Stubs, with the stage that fills each (registered in `stub-tags.json`; the app's own `unimplemented` is `app/src/unimplemented.ts`, the SDK's function copied, because the app reaches the SDK package for types only): `ui: launch` 9a, `ui: queue answer` 9a, `ui: review decision` 9a, `ui: open item` 9a, `ui: compare runs` 9c, `ui: experiment start` 15. They are the bodies of the buttons that need a later stage, so a pressed placeholder button throws its tag, and rendering never reaches one.
  - Tests, written first (red on the missing modules): `app/test/context-view.test.tsx` (18: the six sections in the plan's order with the agent's words last and marked unverified; each of the six missing in turn, both as `not_available` and left out of the record, shows `Not available in this view` once while the other five still show; an empty record shows six; text is shown as text), `app/test/surfaces.test.tsx` (15: a smoke test for Field including the empty board and the Launch count, Tracks, Queue including untrusted and missing input and an empty Queue, run detail with each of its three tabs and each pane, hostile output shown as text, Compare, Experiments), `app/test/app-shell.test.tsx` (9: the shell shows each of the four surfaces the state names, a selected task is counted, an agent's `show_groups` shows its title and a chip with Undo, `open` on Tracks shows the run detail, no chip means no Undo, and each stub action throws its own tag). The context-view criterion was seen failing on its own assertion by making ContextView hide a missing section (13 failures), then restoring it.
  - Gates, from `app/`: `pnpm test` 134 passed (9 files, 42 of them new), `pnpm typecheck` clean, `pnpm build` ok (CSS 32 kB, JS 323 kB). From the root: `cargo test -p collide-checks` 41 passed (the stub-tag checks). The names check finds none of the four withheld names in `app/` or `stub-tags.json`.
- Decisions for the owner:
  - (1) The base style is the CLI's `radix-vega` (its classic look), with the neutral values and no icon library. The newest CLI's presets default to `Nova` with the Geist and Inter fonts and `lucide-react`; I took none of those, so the interface uses the system sans-serif and no icons yet. Recommendation: keep it until the approachable mockup pass picks a type face and icons; any of those is a small change to `index.css` and `components.json`. This changes what a user sees, so it is flagged.
  - (2) `shadcn/tailwind.css` (which the CLI's own template imports) is not used. It needs the `shadcn` package, whose dependencies add about 300 packages to the lockfile for one stylesheet. The copied components use only three of its custom variants (`data-active`, `data-horizontal`, `data-vertical`), so `index.css` holds those three. Alternative: add `shadcn` as a dependency and import the stylesheet. Recommendation: keep the three variants and copy more from that file when a component needs one.
  - (3) The generated shadcn files import `cn` from `@/lib/utils`, not from the `cn` package the CLI's registry names (an unrelated library of unknown maintenance). Recommendation: keep the `clsx` + `tailwind-merge` helper, which is what the brief and shadcn's documented setup use. Later `shadcn add` runs will write the same `from "cn"` import again, and it needs the same edit.
  - (4) A person cannot open a run or a Queue item yet: the reducer has no user action for it, only the agent's `open`. The Open and item buttons call the stub `ui: open item`, and the shell shows the run's detail or the Queue item when the agent has opened it. Alternative: add `user/item_opened` to the reducer now. I held back because it changes ts-0042-vn's finished work and has to decide whether a person's open drops the Undo snapshot like their other edits. Recommendation: add it in Stage 9a, where the Queue's keyboard triage (`j`, `k`, `i`, `o`) needs it anyway, and let a person's open drop the snapshot as the other own changes do.
  - (5) Which run-detail tab is open is the component's own state, started from a `tab` prop, and is not in the reducer; S6 kept it there. Recommendation: move it into the reducer in Stage 9b with the other run detail state (the output's follow position, the filters), because no user action of this item needs it.
  - (6) Placeholder copy follows 0.11's rule where it was free: "Tasks", "Runs" and "Waiting on you" for the surfaces; "Ready to start" and "Working" for states, and a state is a word before it is colour; ids are small muted text under the title; monospace only for a command, the diff and the agent's raw call. The Tracks surface's name for the Runs tab is "Runs", not "Tracks", for a product manager; the plan calls the surface Tracks. Recommendation: confirm in the mockup pass.
  - (7) A stub's stage is one per tag, so `ui: open item` is 9a although Tracks' open is 9b (the Queue half is built first and the 9b item can reuse the tag's body). Alternative: two tags, `ui: open run` and `ui: open queue item`. Recommendation: split them if the 9a item does not also do Tracks.
- Not done by design:
  - The real-time axis on Tracks (segment width is duration, the now line, hatched waits, gates, zoom, the projected remainder): Stage 9b. Every segment is the same width here.
  - Live and paged output, Jump to latest working, expandable tool calls, the raw JSONL toggle, the breakdown's real rows, filters and timeline: Stage 9b. Output and Time breakdown render placeholders.
  - Launch with the run preview, answering a Queue item, keyboard triage, the Queue's mini track and recent activity, the review's real report, findings and diff, and Accept, Send back and Discard working: Stage 9a.
  - The owner client is not connected: the shell shows placeholder data. Stage 3c connects it, and Stage 6a gives the Field its live tasks, filter bar, task drawer and board switcher.
  - Compare's by-tool rows and note box and "where time goes": Stage 9c. The experiment's real preview and result, and running one: Stage 15.
  - Sanitised markdown for task and agent text (R5) is not built: text is shown as text only. Stage 6a.
  - The theme's values: they await the approachable mockup pass (0.11, "Who it is for"); no palette was invented. No light and dark switch is wired, though the dark values are in `index.css`.
  - A browser check: the render was verified by `renderToString` tests and a production build, not by looking at the page in a browser.
- Main session (2026-11-15): decision 6 reversed before commit. The surfaces keep the plan's names (Field, Tracks, Queue) in the shell, the headings and the tests, because renaming a surface is the owner's call; plainer names are asked of the owner with the approachable mockup pass (plan 0.11, "Who it is for"). Decisions 1 to 5 and 7 are accepted as recommended (the owner takes recommendations on code-level choices).
