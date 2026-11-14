#!/bin/sh
# Q3: start weight (context of the first response) under different config flags, in an empty scratch repo
# (no project config), with the same one-word prompt. Also prints what system/init lists.
#   a user      --setting-sources user                      (the plan's config: user)
#   b none      --setting-sources ""                        (the plan's config: none)
#   c none+     b + --strict-mcp-config + autoMemoryEnabled false + --disable-slash-commands
#   e,f,g  b plus one flag each (which flag moves the number); h  b again (run-to-run noise)
#   i  e plus the plan's 0.3 disallowed tool list (the cleanest usable line); j  a again
#   k,l,m,n  user plus: strict MCP; hooks off; auto memory off; all three (attributes the user-config weight)
#   d floor     c + --tools ""                              (no tools at all; a floor, not usable by an agent)
. "$(dirname "$0")/lib.sh"
D="$WORK/empty"
PROMPT="Without using tools, reply in one line exactly as: claudemd=<yes|no> (yes only if your instructions contain the phrase 'Incremental progress over big bangs')"
NOMEM='{"autoMemoryEnabled":false}'
run() { # name claude-flags...
  name=$1; shift
  drive --log "$OUT/$name.jsonl" --cwd "$D" --timeout-s 120 --turn "$PROMPT" -- $BASE_FLAGS "$@" >/dev/null
  first=$(jq -r 'select(.dir=="<-" and .msg.type=="assistant") | .msg.message.usage | [.input_tokens,.cache_read_input_tokens,.cache_creation_input_tokens]|@tsv' "$OUT/$name.jsonl" | head -1)
  ctx=$(echo "$first" | awk '{print $1+$2+$3}')
  init='select(.type=="system" and .subtype=="init")'
  echo "== $name: first-response context=$ctx (input/read/write: $first)"
  echo "   tools=$(events "$OUT/$name.jsonl" "$init | .tools | length" | head -1) mcp_servers=$(events "$OUT/$name.jsonl" "$init | .mcp_servers | map(.name+\"=\"+.status) | join(\",\")" | head -1)"
  echo "   plugins=$(events "$OUT/$name.jsonl" "$init | .plugins | length" | head -1) agents=$(events "$OUT/$name.jsonl" "$init | .agents | length" | head -1) skills=$(events "$OUT/$name.jsonl" "$init | .skills | length" | head -1) slash_commands=$(events "$OUT/$name.jsonl" "$init | .slash_commands | length" | head -1) capabilities=$(events "$OUT/$name.jsonl" "$init | .capabilities | length" | head -1)"
  echo "   hooks started: $(events "$OUT/$name.jsonl" 'select(.type=="system" and .subtype=="hook_started") | .hook_name' | sort | uniq -c | tr '\n' ' ')"
  echo "   answer: $(events "$OUT/$name.jsonl" 'select(.type=="result") | .result')   $(summarize --quiet "$OUT/$name.jsonl" | sed 's/^= [^:]*: //')"
}
run a-user --setting-sources user
run b-none --setting-sources ""
run c-none-plus --setting-sources "" --strict-mcp-config --settings "$NOMEM" --disable-slash-commands
run e-b-nomem --setting-sources "" --settings "$NOMEM"
run f-b-nocmds --setting-sources "" --disable-slash-commands
run g-b-strict --setting-sources "" --strict-mcp-config
run h-b-again --setting-sources ""
run i-nomem-disallow --setting-sources "" --settings "$NOMEM" --disallowedTools CronCreate ScheduleWakeup RemoteTrigger PushNotification EnterWorktree ExitWorktree Workflow SendMessage
run j-user-again --setting-sources user
run k-user-strict --setting-sources user --strict-mcp-config
run l-user-nohooks --setting-sources user --settings '{"disableAllHooks":true}'
run m-user-nomem --setting-sources user --settings "$NOMEM"
run n-user-strict-nohooks-nomem --setting-sources user --strict-mcp-config --settings '{"disableAllHooks":true,"autoMemoryEnabled":false}'
run d-floor --setting-sources "" --strict-mcp-config --settings "$NOMEM" --disable-slash-commands --tools ""
echo
echo "-- a-user init breakdown (names):"
for k in tools mcp_servers plugins agents; do
  echo "   $k: $(events "$OUT/a-user.jsonl" "select(.type==\"system\" and .subtype==\"init\") | .$k | map(if type==\"object\" then (.name // .) else . end | tostring) | join(\" \")" | head -1 | cut -c1-1500)"
done
echo "   skills (count by prefix): $(events "$OUT/a-user.jsonl" 'select(.type=="system" and .subtype=="init") | .skills | map(split(":")[0] | if (. | test(" ")) then "x" else . end) | group_by(.) | map("\(.[0])=\(length)") | join(" ")' | head -1 | cut -c1-900)"
echo "   capabilities: $(events "$OUT/a-user.jsonl" 'select(.type=="system" and .subtype=="init") | .capabilities | join(",")' | head -1)"
