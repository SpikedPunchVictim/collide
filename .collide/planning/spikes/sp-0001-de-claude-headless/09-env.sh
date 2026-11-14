#!/bin/sh
# Q9: environment.
#   a  nested: claude started with this shell's CLAUDE* variables left set (CLAUDECODE etc.)
#   b  clean: the same launch through claude-clean.sh
# For each: refusals or warnings (stderr, system events), the session id actually used, and the
# CLAUDE* / ANTHROPIC_* variables seen by a settings hook, an MCP server and a Bash tool call.
# Values are printed only for variables that hold no secret; the rest print as <set>.
. "$(dirname "$0")/lib.sh"
D="$WORK/plain/env"; mkdir -p "$D"
cat >"$D/envdump.sh" <<'EOF'
env | grep -E '^(CLAUDE|ANTHROPIC_)' | sort | while IFS='=' read -r k v; do
  case $k in
    CLAUDECODE|CLAUDE_CODE_ENTRYPOINT|CLAUDE_CODE_CHILD_SESSION|CLAUDE_CODE_SESSION_ATTENDED|CLAUDE_EFFORT|CLAUDE_CODE_SESSION_ID) echo "$k=$v" ;;
    *) echo "$k=<set>" ;;
  esac
done
EOF
node "$S1/make-settings.mjs" ENV >"$OUT/settings.json"
printf '{ "mcpServers": { "s1env": { "command": "node", "args": ["%s", "s1env"] } } }\n' "$S1/mcp-canary.mjs" >"$OUT/mcp.json"
PROMPT="Use the Bash tool to run exactly: sh envdump.sh   Then reply with its output verbatim and nothing else."

echo "## this shell (a child of the Claude Code session running the spike):"
sh "$D/envdump.sh" | sed 's/^/   /'

for MODE in a-nested b-clean; do
  SID=$(new_uuid)
  case $MODE in a-nested) KEEP=--keep-env ;; *) KEEP= ;; esac
  echo; echo "################ $MODE (session-id=$SID)"
  marks="$OUT/$MODE.marks"; : >"$marks"
  drive --log "$OUT/$MODE.jsonl" --cwd "$D" $KEEP --env "S1_MARKS=$marks" --perm allow --turn "$PROMPT" -- \
    $BASE_FLAGS --permission-prompt-tool stdio --settings "$OUT/settings.json" --mcp-config "$OUT/mcp.json" --session-id "$SID"
  summarize "$OUT/$MODE.jsonl" | grep -vE '^\.\. note \{"started'
  echo "-- stderr lines: $(jq -r 'select(.dir=="stderr") | .msg.line' "$OUT/$MODE.jsonl" | wc -l | tr -d ' ')"
  echo "-- system subtypes: $(events "$OUT/$MODE.jsonl" 'select(.type=="system") | .subtype' | sort | uniq -c | tr -s ' ' | tr '\n' ' ')"
  echo "-- session ids on stdout: $(events "$OUT/$MODE.jsonl" '.session_id // empty' | sort -u | tr '\n' ' ') (asked for $SID)"
  echo "-- init: $(events "$OUT/$MODE.jsonl" 'select(.type=="system" and .subtype=="init") | {apiKeySource, permissionMode, messaging_socket_path, model, plugins: (.plugins|length), mcp: (.mcp_servers|map(.name))}' | head -1)"
  echo "-- transcript written: $(ls "$HOME/.claude/projects/"*s1-claude-headless-work-plain-env/"$SID".jsonl >/dev/null 2>&1 && echo yes || echo no)"
  echo "-- settings hook saw: $(grep '^SessionStart' "$marks" | sed 's/.*env=//')"
  echo "-- MCP server saw:    $(grep '^mcp-start' "$marks" | sed 's/.*env=//')"
  echo "-- Bash tool saw:"
  events "$OUT/$MODE.jsonl" 'select(.type=="user") | .message.content[]? | select(.type=="tool_result") | .content' | sed 's/\\n/\n/g; s/^"//; s/"$//' | sed 's/^/   /'
done

echo; echo "## spend"
summarize --quiet "$OUT"/*.jsonl
