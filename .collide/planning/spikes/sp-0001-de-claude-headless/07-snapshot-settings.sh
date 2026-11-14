#!/bin/sh
# Q7: settings from a snapshot. The reviewer runs in the builder's worktree (work/wt, whose
# CLAUDE.md, settings hooks, .mcp.json, skills and agent were edited: tag WT) with settings,
# MCP config, agents and a plugin built from main's commit (work/snapshot: tag SNAP).
# What still loads from the worktree?
#   a  the plan's line: --setting-sources user --settings SNAP --mcp-config SNAP --strict-mcp-config
#      --plugin-dir SNAP-plugin
#   b  a + --agents SNAP --agent collide-test-builder (the reviewer as it would really run)
#   c  a + --disable-slash-commands
#   d  a, with claudeMdExcludes for the worktree in the snapshot settings
#   e  a + --agents/--agent + --safe-mode
#   f  a, but --setting-sources "" (no user settings either)
#   g  a, with a canary in the auto-memory directory (keyed to the main checkout)
#   h  g, with autoMemoryEnabled=false in the snapshot settings
#   i  g, with CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 in claude's environment
#   j  g, with --restricted (ignores user, project and local settings files) instead of --setting-sources
. "$(dirname "$0")/lib.sh"
ensure_fixture
WT="$WORK/wt"; SNAP="$WORK/snapshot"
SNAPFLAGS="--settings $SNAP/settings.json --mcp-config $SNAP/mcp.json --strict-mcp-config --plugin-dir $SNAP/plugin/s1snap"
AGENTFLAGS="--agents $SNAP/agents.json --agent collide-test-builder"

echo "################ a: --setting-sources user + snapshot settings/MCP/plugin"
loading_run a "$WT" $BASE_FLAGS --setting-sources user $SNAPFLAGS --session-id "$(new_uuid)"

echo; echo "################ b: a + --agents/--agent from the snapshot"
loading_run b "$WT" $BASE_FLAGS --setting-sources user $SNAPFLAGS $AGENTFLAGS --session-id "$(new_uuid)"

echo; echo "################ c: a + --disable-slash-commands"
loading_run c "$WT" $BASE_FLAGS --setting-sources user $SNAPFLAGS --disable-slash-commands --session-id "$(new_uuid)"

echo; echo "################ d: a, snapshot settings carry claudeMdExcludes for the worktree"
node "$S1/make-settings.mjs" SNAP "{\"claudeMdExcludes\":[\"$WT/CLAUDE.md\",\"$WT/**/CLAUDE.md\",\"$WT/.claude/**\"]}" >"$OUT/settings-excl.json"
loading_run d "$WT" $BASE_FLAGS --setting-sources user --settings "$OUT/settings-excl.json" \
  --mcp-config "$SNAP/mcp.json" --strict-mcp-config --plugin-dir "$SNAP/plugin/s1snap" --session-id "$(new_uuid)"

echo; echo "################ e: a + --agents/--agent + --safe-mode"
loading_run e "$WT" $BASE_FLAGS --setting-sources user $SNAPFLAGS $AGENTFLAGS --safe-mode --session-id "$(new_uuid)"

echo; echo "################ f: a, but --setting-sources \"\""
loading_run f "$WT" $BASE_FLAGS --setting-sources "" $SNAPFLAGS --session-id "$(new_uuid)"

echo; echo "################ g: a, with a canary in the auto-memory directory init.memory_paths names"
# The directory is keyed to the main checkout (work/repo), not to the worktree, and lives under
# ~/.claude/projects/. This variant writes one MEMORY.md there and removes it afterwards; it
# refuses to run if the file already exists. Variants h, i and j run while the canary is there.
MEMDIR=$(events "$OUT/a.jsonl" 'select(.type=="system" and .subtype=="init") | .memory_paths.auto // empty' | head -1 | tr -d '"')
case "$MEMDIR" in
  "$HOME"/.claude/projects/*s1-claude-headless-work-repo/memory/) ;;
  *) echo "unexpected memory dir '$MEMDIR'; skipping g"; MEMDIR= ;;
esac
if [ -n "$MEMDIR" ] && [ ! -e "$MEMDIR/MEMORY.md" ]; then
  mkdir -p "$MEMDIR"
  printf -- '- The S1 memory canary word is MEMORY-CANARY-IBIS.\n' >"$MEMDIR/MEMORY.md"
  loading_run g "$WT" $BASE_FLAGS --setting-sources user $SNAPFLAGS --session-id "$(new_uuid)"
  echo; echo "################ h: g, with autoMemoryEnabled=false in the snapshot settings"
  node "$S1/make-settings.mjs" SNAP '{"autoMemoryEnabled":false}' >"$OUT/settings-nomem.json"
  loading_run h "$WT" $BASE_FLAGS --setting-sources user --settings "$OUT/settings-nomem.json" \
    --mcp-config "$SNAP/mcp.json" --strict-mcp-config --plugin-dir "$SNAP/plugin/s1snap" --session-id "$(new_uuid)"
  echo; echo "################ i: g, with CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 in the environment instead"
  LR_DRIVER_OPTS="--env S1_PASS=CLAUDE_CODE_DISABLE_AUTO_MEMORY --env CLAUDE_CODE_DISABLE_AUTO_MEMORY=1"
  loading_run i "$WT" $BASE_FLAGS --setting-sources user $SNAPFLAGS --session-id "$(new_uuid)"
  LR_DRIVER_OPTS=
  echo; echo "################ j: g, with --restricted --tools Bash,Read,Grep,Glob instead of --setting-sources user"
  loading_run j "$WT" $BASE_FLAGS --restricted --tools "Bash,Read,Grep,Glob" $SNAPFLAGS --session-id "$(new_uuid)"
  rm -f "$MEMDIR/MEMORY.md"
  echo "-- removed $MEMDIR/MEMORY.md"
fi

echo; echo "## spend"
summarize --quiet "$OUT"/*.jsonl
