#!/bin/sh
# S9 addendum hook (spike-local, loaded only through --settings). Runs for PreToolUse and PostToolUse on Bash.
# Behaviour comes from $S9_HOOK_MODE. Appends one line per call to $S9_MARKS (event, tool, mode).
in=$(cat)
echo "$(echo "$in" | jq -c '{hook_event_name, tool_name, keys: (keys|join(","))}') mode=${S9_HOOK_MODE:-}" >> "${S9_MARKS:-/dev/null}"
case "${S9_HOOK_MODE:-}" in
  deny-json) printf '%s\n' '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"S9-DENY-JSON the json hook refuses this tool call"}}'; exit 0 ;;
  deny-exit2) echo "S9-DENY-EXIT2 the exit-2 hook refuses this tool call" >&2; exit 2 ;;
  ask) printf '%s\n' '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"ask","permissionDecisionReason":"S9-ASK the hook wants the host to decide"}}'; exit 0 ;;
  err1) echo "S9-ERR1 the hook crashed" >&2; exit 1 ;;
  badjson) echo '{"hookSpecificOutput": not json S9-BADJSON'; exit 0 ;;
  sleep) sleep 5; exit 0 ;;
  post-block) printf '%s\n' '{"decision":"block","reason":"S9-POST the post hook objects to this tool result"}'; exit 0 ;;
  post-exit2) echo "S9-POST-EXIT2 the post hook objects" >&2; exit 2 ;;
  *) exit 0 ;;
esac
