---
id: ts-0203-xn
kind: task
status: ready
title: "App: the ask box and the chip on the Field"
owners: []
links:
  - { type: blocked-by, to: ts-0202-ab }
---

## Goal

The owner can ask from the top bar and see the change on the Field: the ask box shows one line per call, the chip
says what changed, Undo takes it back, and a failure says why. This is the first time an ask draws on the screen,
over fake streams and through the bridge with fake Claude.

What exists.
- The top bar (`ts-0179-ke`), the Field (`ts-0181-ft`), the filter bar and Save as a view (`ts-0190-tz`), the
  layouts (`ts-0187-hb`), the CSP and markdown (`ts-0183-ee`).
- `ask.start` and `ask.cancel` in `app/src/owner/methods.ts`; the daemon's ask path and the shell's notices
  (`ts-0201-us`); the tools and the reducer on the Field (`ts-0202-ab`).
- `app/src/components/LensBanner.tsx` (`ts-0042-vn`).
- The mock: `askBtnHtml`, `askPopHtml`, `bigAsk`, `applyAsk`, `achipHtml`, `cancelAsk`, `endAsk`, `undoAsk`;
  notes 47, 49, 50, 52 and 53.

What this adds.
- The Ask button in the top bar and its box (note 47): examples, the request, one line per call written by Collide
  (`query tasks: 12 rows`, `show_groups: 4 groups, 11 tasks`), and Cancel. The ask goes on when the box is closed.
  Each ask starts fresh; the box is not a chat.
- The chip (note 49): on the surface the ask changed, above what it changed. Collide's statement first (the
  expression and count, or the groups and tasks), then the agent's title and words marked unverified, the source
  (agent and model, or "external client") and the time. Undo takes back the last change only, and a person's own
  change ends it.
- The box closes when the ask succeeds. A failure keeps it open with the plain text of 0.11 and the agent's closing
  words; a red dot shows on the button if the box was closed (note 52).
- Save as View on a `set_view` chip: the chips' CEL, `layout.groupBy` and `layout.sort`, saved through
  `task.save_view` as a personal view, under a name the person gives.
- `highlight` draws a ring and a chip on the marked tasks (note 53). A `show_groups` change shows its chip here; the
  lens's board is `ts-0204-ea`'s.
- The flag (note 50): a `show_groups` ask with no read shows "No files were read in this ask" in amber. A Read, Grep
  or Glob (Claude) or a `read` or `search` tool call (ACP) clears it. For an external client the chip says its
  reads are not visible.

## Success criteria

- Over a fake owner: the lines, the chip, Undo, and a person's change ending Undo each behave as above.
- The box closes on success; each failure text of 0.11 that this item can reach shows with the box open; the red
  dot shows when the box was closed.
- The flag shows for a `show_groups` ask with no read, and not after a Read, a Grep, a Glob, an ACP `read` or an
  ACP `search`; an external change says its reads are not visible.
- Save as View writes a personal view whose `where`, `layout.groupBy` and `layout.sort` reopen the same Field.
- In a browser, through the bridge and a real daemon on a temp root, one ask over fake Claude draws a chip on the
  Field, and Undo removes it.
- `pnpm test` passes.

## Tests

Write these first.
- `app/test/`: the box, the lines, the chip, Undo, the failure texts, the red dot, the flag cases, Save as View.
- `app/browser/`: one ask over fake Claude through the bridge; the chip; Undo.
- The S6 ports: `agent.test.ts` "ask state" (two tests) and "Save as view keeps the view the change made";
  `agent-ui.test.tsx` "agent chips" (a Field sort in place of S6's Tracks sort, and Save as view asks for a name);
  `mcp.test.ts` "launches the configured agent, relays its tool calls and the end, and the change is credited to
  it" (the browser test) and "reports an agent that read but changed nothing, and a failure".

## Notes

- IMPLEMENTATION_PLAN.md:
  - 0.11: The ask box; Chips; the flag ("A lens made from task text alone"). Revision 26: the chip sits on the
    surface the ask changed, and the box closes on success (note 49, decided).
  - Stage 6b: The ask box; Success Criteria (the flag). Part L.
- Builds on `ts-0179-ke` (the top bar), `ts-0181-ft`, `ts-0187-hb` and `ts-0190-tz`, and names them.
- **Owner's choice, recommendation taken (2026-11-24): the agent's closing words.** Recommend the text of the last
  assistant message in the stream, cut at 600 characters, as plain text with control characters stripped (0.9).
  Alternative: every assistant message; longer, and the chip is not a chat.
- **Owner's choice, recommendation taken (2026-11-24): which agent the box uses.** Recommend `ask.agent` from Settings,
  named in the box, with no picker here, as the mock draws it; the Agents page's "The Ask box uses" (`pickAskAgent`,
  `ts-0210-mz`) sets it. Alternative: a picker in the box too; a second place to choose.
- Area: `app/src/components/` (a new `ask/` folder, the top bar, the Field), `app/src/ui/`, `app/test/`,
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
