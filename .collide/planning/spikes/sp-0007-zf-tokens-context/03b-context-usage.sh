#!/bin/bash
# Q3 extra: the tool's own breakdown of the start weight, from a get_context_usage control request
# (context-usage.mjs), for four lines. Prints each category's tokens, the total and the window.
. "$(dirname "$0")/lib.sh"
D="$WORK/empty"
NOMEM='{"autoMemoryEnabled":false}'
DIS="CronCreate ScheduleWakeup RemoteTrigger PushNotification EnterWorktree ExitWorktree Workflow SendMessage"
cu() { # name claude-flags...
  name=$1; shift
  node "$S9/context-usage.mjs" "$D" -- -p --input-format stream-json --output-format stream-json --verbose --model haiku --max-budget-usd 0.25 --session-id "$(new_uuid)" "$@" > "$OUT/$name.json" 2> "$OUT/$name.err"
  echo "== $name: $(jq -r '.response.response | "total=\(.totalTokens) window=\(.maxTokens) (raw \(.rawMaxTokens), source \(.autocompactSource))  " + (.categories | map(select(.kind=="used" or .kind=="deferred" or .kind=="buffer") | "\(.name)=\(.tokens)\(if .isDeferred then "(deferred)" else "" end)") | join("; "))' "$OUT/$name.json")"
  echo "   other keys: $(jq -r '.response.response | keys - ["categories","gridRows"] | join(",")' "$OUT/$name.json")"
}
cu user --setting-sources user
cu none --setting-sources ""
cu none-clean --setting-sources "" --settings "$NOMEM" --disallowedTools $DIS
cu user-minus-mcp-hooks-mem --setting-sources user --strict-mcp-config --settings '{"disableAllHooks":true,"autoMemoryEnabled":false}'
