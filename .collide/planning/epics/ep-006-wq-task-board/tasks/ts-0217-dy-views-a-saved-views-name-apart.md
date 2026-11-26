---
id: ts-0217-dy
kind: task
status: ready
title: "Views: a saved view's name apart from its file name"
owners: []
links:
  - { type: blocked-by, to: ts-0215-gt }
---

## Goal

A person saves a view under the words they type, spaces included: "Slow spots" is saved, listed, picked and named on
the command line as "Slow spots". The file it is saved in gets a name Collide makes from those words,
`slow-spots.yaml`. The plan's view example has always worked this way (`edge-mvp.yaml` holds `name: MVP`); the save
did not.

What exists.
- `host/src/views.rs` `save_view` refuses a name that is not a safe token (`collide_core::resolve::is_safe_token`:
  1 to 64 letters, digits, `.`, `_` or `-`) and writes `<name>.yaml`. `ts-0196-uw` found the refusal: S6's "Slow
  spots" cannot be saved, and `app/browser/s6.spec.ts` saves "Slow-spots" instead.
- Loading already takes a view's name from its YAML `name:`, not from its file name (`personal_candidates`,
  `repo_candidates`). A second view of a name already taken is listed with an error (`assemble`, `shadowed`).
- Share with team (`host/src/board_init.rs`) copies the person's view files by file name, with their bytes.
- The CLI resolves `--view <name>` in the daemon, by the whole name or by the part after its last `/` (Stage 4,
  Built-in and pack views).

What this adds.
- **The name.** The words typed, with spaces at the ends trimmed: 1 to 64 characters, no control characters and no
  `/`, which marks a pack's view (`collide/plan/spikes`). A name a built-in view has (`ready`, `bottlenecks`) is
  refused, as is the picker's All tasks, matched without regard to case. Each refusal says what to change.
- **The file name**, made from the name: lowercased; each run of characters other than `a-z`, `0-9`, `.` and `_` made
  one `-`; `-` and `.` trimmed from the ends; cut to 64 characters. A name that leaves nothing (one in another
  script) gets `view-` and a short hash of the name. The file name is always a safe token.
- **Where it is saved.** A file of the folder that holds a view of that exact name is that view's file: it is
  replaced only with `overwrite`, as now, whatever its file name. Otherwise the made file name is used. If that file
  is there and holds another name, or does not parse, the save is refused and asks for another name ("Slow Spots" and
  "slow spots" are two names and one file). The link checks and the atomic write stay as they are.
- **The command line** takes the name quoted: `collide tasks --view "Slow spots"`. A name no view has lists the
  names, each quoted.
- **S6's spec** saves "Slow spots" and detects it by that exact name. Its refusal check and the `Slow-spots`
  workaround go.

## Success criteria

- "Slow spots" saves as `slow-spots.yaml` with `name: Slow spots`, and `task.views` and `task.list` with the view
  name it "Slow spots".
- Saving "Slow spots" again is refused unless with overwrite, which replaces the same file.
- A hand-written `perf.yaml` holding `name: Slow spots` is the file an overwrite of "Slow spots" replaces; no second
  file appears.
- "Slow Spots" beside a saved "Slow spots" is refused and asks for another name; no file changes.
- A name with `/`, a control character, more than 64 characters, only spaces, or a built-in's name is refused, and
  says what to change.
- A view saved before this item (name equal to its file name) loads, lists and saves as before.
- `browser/s6.spec.ts` saves "Slow spots" and detects; no step in it refuses a space.

## Tests

- `host/tests/` (a temp root): the cases above, each against the folder on disk; the hash fallback; a link at the made
  file name still refused; Share with team copies the file under its made name.
- The CLI over a temp root: `collide tasks --view "Slow spots"` lists the view's tasks; an unknown name lists the
  names quoted.
- `app/browser/s6.spec.ts` (bridge): saves "Slow spots" and detects it.
- A unit test of the file-name rule: spaces, case, runs of other characters, the trims, the 64 cut, the hash
  fallback, and that each result passes `is_safe_token`.

## Notes

- IMPLEMENTATION_PLAN.md: Stage 4 (Views; Built-in and pack views); Stage 6a (Save as View); revision 31.
- **An item decision: names that differ only in case.** Recommend refusing the second, since one file name holds
  both and two views a person cannot tell apart help no one. Alternative: a numbered file name, `slow-spots-2.yaml`.
- **An item decision: the hash.** Use a hash crate the workspace already has. If none fits, stop and name it to the
  owner; no new dependency without them.
- Leave `is_safe_token` as it is: other names (packs, board ids) use it.
- The test rule: no test touches the real home, real launchd, a real daemon, the keychain or the network. Each test
  that runs the `collide` binary clears the environment (`env_clear()`) and sets `HOME` and `COLLIDE_DAEMON_ROOT` to a
  temp root, as `host/tests/common/mod.rs` `TempRoot` does. A test never stubs or bypasses a safety gate (the trust
  check, `env_clear`, the link checks) to make a result appear.
- Nothing here runs a real `claude`. New dependencies: none expected.
- Stage 6a item (revision 31, 2026-11-26), from `ts-0196-uw`'s finding. It runs after `ts-0215-gt` and before
  `ts-0197-uj`, whose S6 walkthrough saves "Slow spots".
- Owner decision (2026-11-26): a view keeps the name typed, and Collide makes its file name (chosen over allowing
  spaces in file names, and over keeping the rule and stating it).
