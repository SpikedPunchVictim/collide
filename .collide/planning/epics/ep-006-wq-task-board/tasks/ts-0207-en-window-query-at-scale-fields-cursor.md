---
id: ts-0207-en
kind: task
status: ready
title: "Window: query at scale, fields, cursor and bodies"
owners: []
links:
  - { type: blocked-by, to: ts-0206-nf }
---

## Goal

`query` pages a large board under the MCP output cap: no body unless asked, `fields`, a cursor, and a body only by
id, cut at 2 KB. Over about 1,500 tasks, every page stays under 20,000 tokens.

What exists.
- `query` over the tasks the window holds, with `filter` and `limit` and no body (`ts-0202-ab`).
- S6's `query` (`src/agent/exec.ts`), which returned every row with its body.

What this adds.
- `fields` (the row's keys to return), `limit` from 1 to 500 (default 100), `cursor` in and `next_cursor` out.
- A body only when `fields` asks for it and the filter names at most 20 ids, cut at 2 KB with its full size shown.
  More than 20 ids with a body asked is refused with a text that names the limit.
- The page ends before its estimated size reaches 20,000 tokens, not at `limit`, and the cursor goes on.
- A hook for a caller whose untrusted bodies are withheld: no body at all, whatever `fields` says. `ts-0209-kb`
  sets it for a red agent.
- A deterministic fixture of about 1,500 tasks, made in the test.

## Success criteria

- Over about 1,500 tasks: no body by default; `fields` honoured; every page under 20,000 estimated tokens; the
  pages together hold every row once; no page nears Claude's 25,000-token output cap.
- A body by id is returned, cut at 2 KB, with its full size. 21 ids with a body asked are refused.
- A caller with bodies withheld gets none.
- A cursor from another filter, or a stale one, is refused with plain text.
- `pnpm test` passes.

## Tests

Write these first.
- `app/test/`: the paging walk over the fixture; `fields`; the body by id and its cut; the 21-id refusal; the
  withheld caller; a bad cursor; the size of each page by the estimate, and by a stricter count.

## Notes

- IMPLEMENTATION_PLAN.md:
  - 0.11: The five tools ("A page fits under the output cap"). 0.3: the MCP output cap. R5.
  - Stage 6b: Success Criteria (`query` at scale). Part L.
- **Decision for the item, with a recommendation: the token estimate.** Recommend the page's UTF-8 bytes divided
  by 3, a cautious count for English and JSON; the test also checks a stricter count of bytes divided by 2.5.
  Alternative: a tokenizer library; a new npm dependency, and an Owner's choice.
- **Decision for the item, with a recommendation: the cursor.** Recommend an opaque string holding the filter's
  hash and the last id, refused when the filter differs. Alternative: an offset; it skips or repeats rows when
  tasks change between pages.
- Area: `app/src/ui/` (the `query` tool), `app/test/`.
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
