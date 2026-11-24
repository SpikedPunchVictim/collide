---
id: ts-0206-nf
kind: task
status: ready
title: "App: Describe what to show in the filter bar"
owners: []
links:
  - { type: blocked-by, to: ts-0205-qh }
---

## Goal

A box in the filter bar turns the person's words into the bar's chips: an ask scoped to the Field's view, answered
with `set_view`. The person keeps it, undoes it or saves it as a view. What a view cannot express, the agent says.

What exists.
- The filter bar, its chips, Advanced and Save as a view (`ts-0190-tz`, which leaves "Describe what to show" and its
  "From your words" chip to this stage).
- The ask path (`ts-0201-us`), `set_view` writing chips (`ts-0202-ab`), the chip, Undo and Save as View
  (`ts-0203-xn`).
- The mock: `askHtml` (the box), `parseAsk`, `runAsk`; note 16 (`notes-full.md`) and note 54.

What this adds.
- The box in the filter bar: "Describe what to show, in your words". It starts an ask with the ask agent (haiku by
  default), and the request Collide sends names the Field's view and its expression.
- The window answers only `query` and `set_view` in a Describe ask, and refuses any other tool with "Describe what
  to show only sets filters; use Ask for groups".
- The result: the bar's chips with a "From your words" chip, the expression under Advanced and the count. Keep
  (the filters stay, the chip goes), Undo, and Save as View, as for any ask.
- A request a view cannot express, such as a comparison with the rest of the corpus (0.10): the agent's closing
  words show as its statement, marked unverified, beside the nearest view it set. No chip shows that the agent did
  not set. If it set nothing, the box says so with the text of 0.11.

## Success criteria

- Over a fake agent: a request becomes chips and the CEL they write, with the count.
- Keep, Undo and Save as View act as for any ask.
- A request a view cannot express shows the agent's statement and the nearest view, with no chip the agent did not
  set.
- A `show_groups` call in a Describe ask is refused and draws nothing.
- `pnpm test` passes.

## Tests

Write these first.
- `app/test/`: over a fake owner, the box, the chips, the CEL, the count, Keep, Undo, Save as View, the statement
  case, and the refused tool.
- `app/browser/`: one Describe ask over fake Claude through the bridge; the chips in the bar.

## Notes

- IMPLEMENTATION_PLAN.md:
  - Stage 6b: Describe what to show (revision 25; note 16); Success Criteria (Describe what to show). Part L.
  - 0.10: what a view can express. 0.11: the ask box's failure texts.
- Builds on `ts-0190-tz` (the filter bar), and names it.
- **Decision for the item, with a recommendation: how a Describe ask is narrowed.** Recommend the window: it
  refuses tools other than `query` and `set_view` for an ask it started from this box. The pinned line stays one
  line. Alternative: a second launch line with a shorter `--allowedTools`; it changes 0.3's pinned line.
- **Decision for the item, with a recommendation: how the request names the view.** Recommend a fixed preamble that
  Collide writes before the person's words: the view's name and expression, and "answer with set_view only; say
  plainly what a view cannot express". Alternative: a new field on `ask.start`; a wire change for one box.
- Area: `app/src/components/field/` (the filter bar), `app/src/components/ask/`, `app/src/ui/`, `app/test/`,
  `app/browser/`.
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
