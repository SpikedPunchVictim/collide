# S9: tokens and context

Spike S9 for Collide. The record is `.collide/planning/spikes/sp-0007-zf-tokens-context/record.md`. Everything runs `claude` through
`claude-clean.sh` (strips every `CLAUDE*` variable except the names in `S9_PASS`), with `--model haiku` and
`--max-budget-usd` on every call. `driver.mjs`, `summarize.mjs`, `spend.sh` and `claude-clean.sh` are copies of S1's.

Setup: `./00-fixture.sh` makes the scratch repos under `work/` (ignored by git): `idle` (ledger1-4.js), `q4`
(ledger1-8.js), `empty`, and `work/mcp-big.json`. In the recorded run, `q4/ledger5-8.js` were `sed` copies of
`ledger1.js` with renamed functions; the fixture script writes the same size of file with slightly different numbers.
`work/hook-settings.json` is written by `05-hook-block.sh`; `07-hook-tool-deny.sh` uses the scratch repo `work/p9` and writes one settings file per case into `out/07-hook-tool-deny/`.

| script | question | what it does |
|---|---|---|
| `06a-idle-start.sh` | Q6 | starts the idle session (four files read) and records the time. Run it first |
| `01-usage-compact.sh` | Q1 | reads files, sends `/compact` on stdin, compares the stream with the transcript (`usage-compare.mjs`) |
| `03-start-weight.sh` | Q3 | first-response context under 14 flag combinations, with `system/init` counts |
| `03b-context-usage.sh` | Q3 | the `get_context_usage` control request (`context-usage.mjs`): tokens per category |
| `04-env.sh` | Q4 | the three variables (auto-compact window, Bash cap, MCP cap; `mcp-big.mjs` is the test MCP server) |
| `04b-window-clamp.sh` | Q4 | how the binary clamps `CLAUDE_CODE_AUTO_COMPACT_WINDOW`, and its precedence over the user's settings |
| `05-hook-block.sh` | Q5 | a `UserPromptSubmit` hook (`hooks/block-prompt.sh`) blocks a resumed prompt, three styles |
| `07-hook-tool-deny.sh` | addendum | a `PreToolUse` or `PostToolUse` hook (`hooks/tool-hook.sh`, modes by `S9_HOOK_MODE`) denies, errors, times out or blocks a Bash call; compared with a host deny, an auto-deny and `--disallowedTools`; second half repeats the hook cases with `--include-hook-events`. `07-summarize.mjs` prints each log's shapes. Output `out/07-hook-tool-deny.txt` and `out/07-hook-tool-deny/` |
| `cache-tier.sh` | Q2 | cache writes by tier over every log in `out/` |
| `usage-aggregate.mjs`, `result-usage-check.mjs` | Q1 | stream against transcript over every session; `result.usage` against per-id sums |
| `06b-idle-resume.sh` | Q6 | needs 61 minutes after `06a`: cold resume, fresh session with a handoff note, warm resume |
| `06c-big-idle-start.sh`, `06d-big-pair.sh` | Q6 | the same pair with eight files read (about 114K tokens) |
| `06-run-pair-when-ready.sh`, `06-run-big-pair-when-ready.sh` | Q6 | wait until the 61 minutes have passed, then run `06b` or `06d` |
| `spend.sh` | | notional spend per script from the driver logs (not the control-request runs, `03b` and `04b`, which have no driver log) |

`./run-all.sh` runs the lot in order (a little over an hour, because of the wait). Output goes to `out/`
(ignored by git).
