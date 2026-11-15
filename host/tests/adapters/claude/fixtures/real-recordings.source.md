# real-recordings/

Streams of the real `claude` 2.1.325 (model `claude-haiku-5-5`, `~/.local/bin/claude`) on the adapter's own launch line,
recorded on 2026-11-15 by `host/tests/adapters/claude/real_claude.rs` (ts-0070-mk), each stream one `.jsonl` file named
for its case: `hello`, `permission-allow`, `permission-deny`, `tools-missing`, `hooks`, `resume-first`, `resume-second`.

These are not the hand-built fixtures of `recordings/`: they hold what the binary wrote, after the redaction rule in
`real.rs` (`Redactor`) and nothing else. The rule, in short: the run's directories become `/work/repo`, `/work/snap-*` and
`/work`; the home becomes `/home/user`; the user name becomes `user`; an e-mail address becomes `user@example.com`; a
`session_id` becomes the fixtures' stand-in `7c1b0e52-9f3a-4d2e-8b61-0a5d3c9e4f17` (a second session ends in 8); other UUIDs
and the `msg_`, `toolu_` and `req_` ids count up as stand-ins; the messaging socket path becomes `/work/cc-socks/1.sock`;
account keys (`email`, `accountUuid`, `organizationUuid`, ...) are removed; the figures and reset times in a
`rate_limit_info` become fixed values. A line that still held `/Users/`, `/private/`, `/var/folders`, `/tmp/`, `sk-ant-`,
`eyJ`, the user name or the home after the rule was not written (the run fails). The prompts are the test's own text.

What they are for: `Shapes` (in `real.rs`) reads them and keeps, for each kind of line, the paths all recorded lines of
that kind have and the types they have. The live contract test fails a live line that lacks one of those paths or has
another type there. The default tests replay them through `ClaudeSession` and check them for anything that names the
machine or the account.

To record again, see the head of `real_claude.rs`. Timing fields (`timestamp`, `duration_*`, `ttft_*`) are what the run had.
