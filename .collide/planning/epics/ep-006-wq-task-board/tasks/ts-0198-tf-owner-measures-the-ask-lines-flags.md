---
id: ts-0198-tf
kind: task
status: done
title: "Owner measures: the ask line's flags and opencode's listener"
owners: []
links:
  - { type: blocked-by, to: ts-0222-gx }
---

## Goal

The owner measures, with plain command lines and no Collide, what the ask line's flags do under `-p`, before any
builder item builds on them. `ts-0200-gs` pins the line from these results, `ts-0208-dx` builds the ACP profile from
them, and `ts-0210-mz` writes the Agents page's words from them.

What exists.
- The ask line in 0.3, read from `claude --help` 2.1.325 and never run. S6 ran a looser line (record, Launching
  Claude Code).
- S6's opencode set-up: a project-local `opencode.json`, `HOME` and four `XDG_*` folders of its own,
  `OPENCODE_DISABLE_CLAUDE_CODE=1`, and the local model `ollama/qwen2.5:7b` (record, Bring your own agent).

What this adds.
- Five results, recorded here as owner measurements: `--add-dir`, the outside read, `--restricted`, opencode's
  listener, and opencode's permission configuration.

A build agent never runs it. It never runs `claude` or `opencode`. The checklist below was written at the split from
the plan. The owner runs it and records the results here, with what was awkward.

## Success criteria

- `--add-dir`: whether a file in a second folder is readable under the ask line's flags, with and without the flag.
  Owner-run, with the `claude` version.
- The outside read: whether a Read of a file outside every readable folder is refused without a prompt under
  `--permission-prompts none`, and whether the file's marker reaches the answer. Owner-run.
- `--restricted`: whether it confines reads (the outside read refused, the added folder still readable). Recorded
  as "adopt" or "do not adopt". `--bare` is not tried. Owner-run.
- opencode's listener: `lsof` by the pid of a started `opencode acp` shows a listening port, or none. Owner-run,
  with the opencode version.
- opencode's permission configuration: whether a project-local block that denies edit and bash stops a write and a
  `true`. With a local model at $0, or recorded as not measured. Owner-run.
- The spend and the time are recorded, within the cap.
- A line records the date, and which item each result changes: `ts-0200-gs` (`--restricted`), `ts-0208-dx` (a
  permission configuration supplied or not), `ts-0210-mz` ("starts in this folder" or "can read only").

## Tests

None by a build agent: this check is the owner's.

## Notes

- IMPLEMENTATION_PLAN.md:
  - 0.3: Claude Code's launch line (`--add-dir`, `--permission-prompts none`, `--restricted`); ACP profile
    (opencode's default is unrestricted; An ACP agent's listener).
  - Stage 6b: "Measured here, for the ask line"; Success Criteria, Measurements recorded. R5.
- **Owner's choice, recommendation taken (2026-11-24): when to measure.** Recommend first, before `ts-0199-xz`: each
  result changes what a later item builds, and the lines need no Collide. Alternative: at the end, as the plan's order
  reads. Then `ts-0200-gs` pins the line without `--restricted`, the page says "starts in this folder", and a fix item
  follows if it confines.
- **Owner's choice, recommendation taken (2026-11-24): spend cap $0.50, time 30 minutes.** Each `claude -p` line passes
  `--max-budget-usd 0.05`, and there are at most eight. opencode runs only with a local model, at $0; with none, its
  permission result is "not measured".
- `claude` uses the real `HOME`, for its login, by choice. Unset every `CLAUDE*` variable first. opencode uses a
  temp `HOME` and `XDG_*` folders, as S6 did, so the owner's own opencode config is not read or changed.
- Area: this item's own text. No code.
- Stage 6b item (revision 26 scope, split 2026-11-24). Items of Stage 6b run one at a time, in id order, because they
  share `app/`, `core`, `host` and the generated files. The next item waits for these results.
- Nothing a build agent runs here starts a model. The owner's lines do, within the cap.
- Owner decisions (2026-11-24): the owner took the recommendations at the split.

### Owner checklist

Written at the split. Check each flag against `claude --help` and `opencode --help` of the installed versions first.

1. Set up a scratch folder `<S>` (made with `mktemp -d`): `<S>/a/a.txt` holds `MARK-A`, `<S>/b/b.txt` holds
   `MARK-B`, `<S>/out/out.txt` holds `MARK-OUT`. `<S>/settings.json` holds `{"autoMemoryEnabled":false}` and
   `<S>/mcp.json` holds `{"mcpServers":{}}`.
2. The base line, run in `<S>/a` with the prompt on stdin:
   `claude -p --output-format stream-json --verbose --model haiku --setting-sources "" --settings <S>/settings.json
   --mcp-config <S>/mcp.json --strict-mcp-config --permission-prompts none --tools Read,Grep,Glob
   --allowedTools Read,Grep,Glob --no-session-persistence --max-budget-usd 0.05`.
   Prompt: "Read the file <path> and print its first line. If you cannot read it, print CANNOT."
3. `--add-dir`: `<S>/b/b.txt` with the base line, then with `--add-dir <S>/b`.
4. The outside read: `<S>/out/out.txt` with `--add-dir <S>/b`. Note any prompt, the tool result, and whether
   `MARK-OUT` is in the answer.
5. `--restricted`: steps 3 and 4 again with `--restricted` added.
6. opencode's listener: with the temp `HOME` and `XDG_*` folders, start `opencode acp` in `<S>/a`, then
   `lsof -a -p <pid> -iTCP -sTCP:LISTEN -nP`. Stop it by its pid.
7. opencode's permissions, only with a local model: in `<S>/a`, ask opencode (`opencode run`, or what its help
   lists) to write `x.txt` and run `true`, first with no permission block, then with one that denies edit and bash.
   Note whether `x.txt` appears.
8. Record: each result, the versions, the spend from each stream's final `result`, the time, what was awkward, and
   the date. Remove `<S>` and the temp opencode folders by their exact paths.

### Results, 2026-11-27

Run by Claude on the owner's instruction (owner, 2026-11-27: "You run it, within the cap"), not by a build agent.
Versions: `claude` 2.1.332 (`--model haiku` answered as `claude-haiku-5-5`), opencode 1.1.56, ollama 0.40.16 with the
local model `qwen2.5:7b`. `<S>` is a `mktemp -d` folder; every line ran in `<S>/a`.

| Line | Flags beyond the base line | File | Answer | Tool result |
| --- | --- | --- | --- | --- |
| control | none | `<S>/a/a.txt` | `MARK-A` | read |
| 3, without | none | `<S>/b/b.txt` | `MARK-B` | read |
| 3, with | `--add-dir <S>/b` | `<S>/b/b.txt` | `MARK-B` | read |
| 4 | `--add-dir <S>/b` | `<S>/out/out.txt` | `MARK-OUT` | read |
| 5, without | `--restricted` | `<S>/b/b.txt` | `CANNOT` | refused, in `permission_denials` |
| 5, with | `--restricted --add-dir <S>/b` | `<S>/b/b.txt` | `MARK-B` | read |
| 5, outside | `--restricted --add-dir <S>/b` | `<S>/out/out.txt` | `CANNOT` | refused, in `permission_denials` |

- **`--add-dir`:** without `--restricted` it changes nothing: the second folder is readable with and without it.
- **The outside read:** not refused. No prompt, no denial, and `MARK-OUT` reached the answer. `--allowedTools Read`
  allows Read on any path, and `--permission-prompts none` denies only what would prompt.
- **`--restricted`: adopt.** The outside read is refused with no prompt, the tool result says "`<S>/out/out.txt` is
  outside `<S>/a`, `<S>/b`; --restricted confines the file tools to the working directory.", the refusal is listed in
  the result's `permission_denials`, and the answer has no marker. The `--add-dir` folder stays readable.
- **opencode's listener:** `opencode acp` listens on TCP `127.0.0.1:4096` from its start, before any ACP message, and
  still after `initialize` and `session/new`. One process, no children. Whether a second one takes another port when
  4096 is in use: not measured.
- **opencode's permission configuration:** with no `permission` block, the model wrote `x.txt` and ran `true`. With
  `{"edit":"deny","bash":"deny"}` it had no write or shell tool: no `x.txt`, and no bash call. It then used the
  `task` tool to start a sub-agent ("general"), and that sub-agent called `webfetch` on `https://opencode.ai/docs`,
  which went through. The block denies what it names and nothing else: `webfetch` and `task` stay allowed.
- **Spend and time:** the seven claude lines cost $0.0031 together (from each stream's final `result`), 48 s in all.
  opencode ran the local model at $0: the listener 13 s, the two runs 56 s and 84 s.
- **Not as the checklist had it:** a control line read `<S>/a/a.txt` first (seven claude lines in all, under the
  eight allowed). The listener step also sent `initialize` and `session/new`, which start no model. opencode used a
  copy of the S7 spike's own opencode cache and config folders, so it fetched no package, with
  `OPENCODE_DISABLE_AUTOUPDATE`, `OPENCODE_DISABLE_LSP_DOWNLOAD` and `OPENCODE_DISABLE_MODELS_FETCH` set and no API
  key in its environment. The `webfetch` above was the model's own call.
- **Awkward:** the deny run's answer reads as if the task was done ("... run the shell command `true`. DONE."). Only
  the tool log shows that nothing was written or run.
- `<S>` and the temp opencode folders were removed by their exact paths.

What each result changes:
- `ts-0200-gs`: the ask line carries `--restricted`. Without it, reads are not confined and `--add-dir` does nothing.
- `ts-0208-dx`: Collide supplies a permission configuration, since opencode's default lets a write and a shell command
  through. It must deny `webfetch` and `task` as well as `edit` and `bash`, or check every tool opencode lists, since
  a block covers only what it names. The listener is real: `127.0.0.1:4096` from the start.
- `ts-0210-mz`: the Agents page says "can read only" these folders for Claude under the ask line. For opencode it
  says what the supplied configuration denies.

