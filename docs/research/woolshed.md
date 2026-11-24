# Woolshed: what Collide takes from it

Research input for plan revision 29, 2026-11-24. Woolshed is closed source. Its public repository holds releases,
docs, a gallery of user-made layouts and install recipes. Only the docs were read. Nothing was built, run or
installed.

## 1. What it is

- A native app (Rust, GPUI, the Ghostty terminal engine) that runs Claude Code, Codex, Gemini and other agent CLIs as
  terminals side by side, under one roof.
- A background service, Woolshed Core (`hc`), holds the terminals. It outlives a closed window, a quit and an update.
  It listens on a local socket or named pipe, and its docs say plainly it is not a sandbox.
- Its labels are named `herdr.*`, so the core comes from herdr-core, which Collide studied and chose not to depend on
  (`docs/research/herdr.md`).
- It is built around agent sessions. It has no tasks, worktrees, workflows or review. It overlaps with Collide's
  Tracks, Queue and Comms only.
- Its pitch, knowing which agent needs you, is the promise of Collide's Queue.

## 2. Taken (owner, 2026-11-24)

### A. Done since you looked (Stage 9b)

- Each terminal has a turn counter that rises when the agent stops working.
- A client writes the shared label `herdr.seen=<turn>` only when a person is really looking: the window in front and
  that agent drawn on screen. It writes at most once a turn. Mark unread writes turn − 1. The last writer wins.
- So an agent that finished while the person was away shows as done until it is seen.
- States: working, needs you (blocked or error), done (finished since you looked), idle.
- Collide had no seen state. Revision 29 adds it to Tracks and Comms, kept by the daemon.

### B. A way back that always works (Stage 6a, `ts-0215-gt`)

- `ctrl-alt-shift-escape` always moves a broken layout or keys file aside and returns to the defaults.
- `woolshed check` loads and draws a mod once, so a broken one is found before it is used.
- Collide had neither. Revision 29 adds Start with defaults: a menu item and its key, owned by the shell, so it works
  with a blank page; Reset… moves the person's files into a dated folder and deletes nothing.

### C. One list of named actions (Stages 6a and 9d)

- Every action has a name and runs the same way from a key, the CLI (`woolshed do <action>`) and mods.
- Revision 29 names the app's keyed actions in one list. The palette (9d) lists it, and the in-app agent's tools use a
  name where they do the same thing. A CLI entry to app actions is not taken.

### D. A guide for agents, built by the app (Stage 12)

- On every start Woolshed writes an `AGENTS.md` for the running version into its config folder, beside generated API
  types (`.d.ts`). `woolshed --skill` and `hc --skill` print manuals written for agents.
- So "ask your agent to build me a layout" works without the person reading docs.
- Collide already has `collide guide` (Stage 4c), built from the CLI's own command list. Revision 29 adds the topics
  `pack` and `view`, built from the installed version's schemas and SDK. Collide still writes nothing into the
  person's agent config.

## 3. Seen, not taken

- **The Queue's extras.** A Next waiting key jumps to the agent that has waited longest. An answer from the list
  (`agent.answer`: allow, deny, a number, or a reply) without opening the terminal. Collide's Queue already sorts by
  wait (0.11) and answers in the drawer; the jump key can be raised when 9a is split.
- **Settle.** `tab.settle` hides a tab until it needs the person.
- **Board layout from the gallery.** Columns Needs you, Working and Done; time in state on each card; pinned first; a
  parent card shows its helpers; a recipe file with "Done when" criteria.
- **Small state in labels, bigger state in docs** (`doc.get`, `doc.set`).
- **Several machines.** An end-to-end sealed relay, Tailscale direct, a phone app, an SSH key vault, voice input,
  themes. Paid features, and out of Collide's scope.

## 4. Where Collide already holds up

- **Questions.** Woolshed finds an agent's question by reading its screen every 5 s: the last line ending in `?` in
  the bottom 24 lines. That breaks easily. Collide reads questions as data from hooks and ACP (0.3).
- **Trust.** Woolshed's mods are trusted JavaScript with no permission prompts, and say so. A gallery item is pinned to
  a commit in a `manifest.json`, and the agent is told to read every file before adding it. Collide's trust prompt and
  packs pinned by git tree id (0.6, 0.9) are stronger.
- **Agents outliving the window.** Collide's daemon and shim do this (0.2; S2).
- **What leaves the machine.** Woolshed's security page lists it (an update check, an install count). Collide's 0.9
  and R5 do the same job.
