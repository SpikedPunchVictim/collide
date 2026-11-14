#!/bin/bash
# Q5: what the stream shows when a UserPromptSubmit hook blocks a resumed prompt.
# The hook comes from work/hook-settings.json (spike-local, passed with --settings, with
# --setting-sources "" so no user hook loads). Session: process 1 starts it with a prompt the hook passes;
# process 2 resumes it with a prompt containing BLOCKME. Three block styles (json, exit2, stop) plus a
# control (resume with a prompt the hook passes). A last case blocks the second turn of a live process.
. "$(dirname "$0")/lib.sh"
D="$WORK/empty"
sed "s#__HOOK__#$S9/hooks/block-prompt.sh#" "$S9/hooks/settings.json.tmpl" > "$WORK/hook-settings.json"
LINE=(--setting-sources "" --settings "$WORK/hook-settings.json")
show() { # log
  summarize "$1" | grep -vE '^\.\. note \{"started|^# '
  echo "   system events: $(events "$1" 'select(.type=="system") | .subtype + (if .hook_event then ":" + .hook_event else "" end)' | sort | uniq -c | tr '\n' ' ')"
  events "$1" 'select(.type=="system" and (.subtype|startswith("hook"))) | {subtype, hook_event, hook_name, outcome, exit_code, stdout: (.stdout|tostring|.[0:200]), stderr: (.stderr|tostring|.[0:200]), output: (.output|tostring|.[0:200])}' | sed 's/^/   hook event: /'
  events "$1" 'select(.type=="result") | {subtype, is_error, stop_reason, terminal_reason, result, errors, num_turns, permission_denials, usage_out: .usage.output_tokens, cost: .total_cost_usd}' | sed 's/^/   result: /'
  events "$1" 'select(.type=="user" and (.message.content|tostring|test("S9-HOOK"))) | {type, isSynthetic, isReplay, content: (.message.content|tostring|.[0:240])}' | sed 's/^/   user event echoing the hook text: /'
  events "$1" 'select(.type=="assistant") | (.message.content|map(.type)|join("+"))' | sort | uniq -c | sed 's/^/   assistant rows: /'
  echo "   stderr lines: $(jq -r 'select(.dir=="stderr") | .msg.line' "$1" | head -3 | tr '\n' '|')   exit: $(jq -c 'select(.dir=="exit") | .msg' "$1")"
}
for MODE in json exit2 stop control; do
  SID=$(new_uuid)
  marks="$OUT/$MODE.marks"; : > "$marks"
  hookmode=$MODE; [ "$MODE" = control ] && hookmode=json
  drive --log "$OUT/$MODE-1.jsonl" --cwd "$D" --env "S9_HOOK_MODE=$hookmode" --env "S9_MARKS=$marks" --turn "Reply with exactly: ONE" -- $BASE_FLAGS "${LINE[@]}" --session-id "$SID" > /dev/null
  PROMPT="Reply with exactly: TWO BLOCKME"; [ "$MODE" = control ] && PROMPT="Reply with exactly: TWO"
  drive --log "$OUT/$MODE-2.jsonl" --cwd "$D" --env "S9_HOOK_MODE=$hookmode" --env "S9_MARKS=$marks" --turn "$PROMPT" -- $BASE_FLAGS "${LINE[@]}" --resume "$SID" > /dev/null
  echo; echo "################ $MODE: resume with: $PROMPT   (session $SID)"
  show "$OUT/$MODE-2.jsonl"
  echo "   hook calls (marks): $(wc -l < "$marks" | tr -d ' ')"
done
echo; echo "################ same live process, second turn blocked (json)"
marks="$OUT/live.marks"; : > "$marks"
drive --log "$OUT/live.jsonl" --cwd "$D" --env "S9_HOOK_MODE=json" --env "S9_MARKS=$marks" --turn "Reply with exactly: ONE" --turn "Reply with exactly: TWO BLOCKME" --turn "Reply with exactly: THREE" -- $BASE_FLAGS "${LINE[@]}" --session-id "$(new_uuid)" >/dev/null
show "$OUT/live.jsonl"
