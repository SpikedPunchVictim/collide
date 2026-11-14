#!/bin/sh
# S9 UserPromptSubmit hook (spike-local, loaded only through --settings). Blocks a prompt that contains
# BLOCKME, in the way chosen by $S9_HOOK_MODE: json | exit2 | stop. Any other prompt passes (exit 0, no output).
# Appends one line per call to $S9_MARKS: the hook's input keys and the mode.
in=$(cat)
echo "$(echo "$in" | jq -c '{keys: keys, hook_event_name, prompt_len: (.prompt|length)}') mode=${S9_HOOK_MODE:-}" >> "${S9_MARKS:-/dev/null}"
case "$in" in
  *BLOCKME*) ;;
  *) exit 0 ;;
esac
case "${S9_HOOK_MODE:-json}" in
  json) printf '%s\n' '{"decision":"block","reason":"S9-HOOK-REASON: prompt blocked by the json hook"}'; exit 0 ;;
  exit2) echo "S9-HOOK-STDERR: prompt blocked by the exit-2 hook" >&2; exit 2 ;;
  stop) printf '%s\n' '{"continue":false,"stopReason":"S9-HOOK-STOPREASON: the hook stopped the session"}'; exit 0 ;;
esac
