#!/bin/sh
# Q6: under plain -p in a git worktree, which of these load: the repo's .claude/settings.json
# hooks, .mcp.json servers, .claude/skills, CLAUDE.md, user plugins?
#   a  plain launch line in the worktree (work/wt), no approval of .mcp.json anywhere.
#   b  same, after the builder adds "enableAllProjectMcpServers": true to the worktree's
#      .claude/settings.json (can a builder approve its own MCP servers?).
# Tags in the marks: WT = worktree copy, WT-LOCAL = worktree settings.local.json,
# BASE / MAIN-LOCAL = the main checkout's copies (they should not appear).
. "$(dirname "$0")/lib.sh"
ensure_fixture
WT="$WORK/wt"

echo "################ a: plain -p in the worktree"
loading_run a "$WT" $BASE_FLAGS --session-id "$(new_uuid)"

echo; echo "################ b: worktree settings.json also sets enableAllProjectMcpServers=true"
cp "$WT/.claude/settings.json" "$OUT/settings.json.orig"
node "$S1/make-settings.mjs" WT '{"enableAllProjectMcpServers":true}' >"$WT/.claude/settings.json"
loading_run b "$WT" $BASE_FLAGS --session-id "$(new_uuid)"
cp "$OUT/settings.json.orig" "$WT/.claude/settings.json"

echo; echo "## spend"
summarize --quiet "$OUT"/*.jsonl
