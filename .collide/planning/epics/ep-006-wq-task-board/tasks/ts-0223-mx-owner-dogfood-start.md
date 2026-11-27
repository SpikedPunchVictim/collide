---
id: ts-0223-mx
kind: task
status: ready
title: "Owner: dogfood start"
owners: []
links:
  - { type: blocked-by, to: ts-0214-rz }
---

## Goal

The owner starts using Collide, with their real `HOME`, to build Collide: small items, run and reviewed through the
app and its in-app agent. Friction found is filed as items. It also closes the keychain check `ts-0197-uj` carried.

What exists.
- After Stage 6b the app has the board, the thin loop (`collide run`, the Run button), the in-app agent and Settings
  with secrets in the login keychain (`ts-0193-gk`, `host/src/secret_store.rs`).
- `ts-0197-uj`, the owner's run, step 36: Part B overrode `HOME`, so the keychain was not found ("Keychain Not Found";
  macOS reads `$HOME/Library/Keychains`). Steps 36, 37, 39 and 41's delete were skipped (owner, option A, 2026-11-27)
  and carried here.

What this adds.
- **The keychain check, with the real `HOME`.** A throwaway secret set from Settings shows "Set", is seen in the login
  keychain by its attributes only (`security find-generic-password` with no `-w` or `-g`), is in no file under the
  daemon's root, still shows "Set" after a restart, and is deleted at the end.
- **Dogfooding.** The owner opens the Collide checkout as a board in their real Collide and runs small items through
  the loop. Each friction point is filed as an item, with what was done, what was seen and what was expected.
- An owner checklist, written by an agent before the owner starts and run by the agent first as far as it can without
  the real `HOME`, as `ts-0197-uj`'s was. It lists what the real `HOME` changes (the real daemon root, a launchd job,
  the keychain) and how each is undone.

## Success criteria

- The keychain steps pass, or each failure is filed.
- At least three items built through Collide end to end (run, review, commit), each recorded with its time and what
  was awkward.
- The friction list is filed as items, each with the plan stage it belongs to.

## Tests

- None of its own: the owner's run is the test.

## Notes

- IMPLEMENTATION_PLAN.md: Stage 6b (after the walkthrough); revision 33.
- Owner decision (2026-11-27): dogfooding starts after Stage 6b, chosen over before 6b and over carrying only the
  keychain check.
- Spend: the Claude runs are on the owner's own account. The owner sets a cap when they start; the checklist asks for
  it, and no agent starts a real `claude` without it.
- It uses the owner's real Collide root. Before it, `ts-0222-gx` lets the owner remove the leftover check boards.
- Stage 6b item (revision 33, 2026-11-27). It runs after `ts-0214-rz`.
