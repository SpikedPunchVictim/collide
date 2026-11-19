# Using Collide from your own Claude Code session

You can drive Collide from any Claude Code session you start yourself, in your own terminal. The agent runs the
`collide` command. Every write goes through Collide's daemon, so the Collide app shows it as it lands, and a run
started this way appears on the loop page.

Collide writes nothing into your Claude config. You change two things yourself, and both are small.

## 1. Put `collide` on your `PATH`

Claude Code runs commands with your shell's `PATH`, so `collide` must be on it. The `collide` binary sits beside
`collide-daemon`, in the folder the Collide build or install puts them in. Either add that folder to your `PATH`, or
link the one binary from a folder that is on it:

```sh
ln -s <the folder holding collide>/collide ~/.local/bin/collide
```

Check it from a new shell: `collide --help` lists the commands.

A link works, because `collide` resolves it to the real file before it looks for `collide-daemon` beside it. A copy
does not, unless `collide-daemon` sits next to the copy.

## 2. Tell the agent where to look

Add one line to the repository's `CLAUDE.md`, or to your own (`~/.claude/CLAUDE.md`) if you want it everywhere:

```text
This repo is planned in Collide; run `collide guide`.
```

`collide guide` prints a short guide as plain text: what Collide is, the commands an agent may use, the forms of
`collide task`, the exit codes 10, 11 and 12, and the ops that wait for you. It needs no daemon, reads nothing and
writes nothing, so it prints the same in any folder. The agent reads it once and then works from it.

## What the agent can do

- List and read items: `collide tasks`, `collide task <id>`.
- File a follow-up, an item of the plan, with `collide task create <title>`.
- Link items, and change an item with an op its source declares.
- Start a run: `collide run <task-file>`.

`collide task new <prompt>` makes a scratch task in Collide's own store, in no file. It is not a follow-up, and the
guide says so.

## What stays yours

These ops are a person's: accept, defer, reject, resolve and reopen. When an agent asks for one, the command writes
nothing and exits 12. A confirmation waits in the app, and you answer Accept or Dismiss there.

A write to a checkout that is in the middle of a rebase, a merge or a live run is queued, and exits 11 if it did not
apply in time. A view with an error of its own exits 10.

## Agents Collide starts

You need none of the above for a run Collide starts. Every run's agent already has a `collide` first on its `PATH`, and
Claude's launch line allows `Bash(collide:*)`, so a headless run can file a follow-up without asking. Any other shell
command still asks the host, as before. A follow-up an agent files inside a run is private to you, with a link to the
run's task, until you keep it. A run's agent may not start a run, keep an item or do the ops that are yours.
