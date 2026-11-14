#!/bin/sh
# Q2: --permission-prompts host. What a permission request looks like on stdout, and how the
# host answers it on stdin. The driver allows the first can_use_tool request and denies the second.
# Shell `echo` is auto-allowed as read-only (measured: no prompt), so the calls write files.
#   a  --permission-prompts host alone
#   b  a + an initialize control_request first (what the Agent SDK sends)
#   c  a + --permission-prompt-tool stdio (not in --help; the bundled Agent SDK passes it)
#   d  --permission-prompt-tool stdio alone
#   e  d, but the host closes stdin instead of answering the first request (a host that died)
. "$(dirname "$0")/lib.sh"
PLAIN="$WORK/plain"; mkdir -p "$PLAIN"
PROMPT="Use the Bash tool twice, as two separate calls one after the other (not in parallel). First run exactly: touch s1-first.txt. Then run exactly: touch s1-second.txt. Then reply with one line saying what each call returned."

run() { # name, extra driver flags..., -- , extra claude flags...
  name=$1; shift
  rm -f "$PLAIN"/s1-first.txt "$PLAIN"/s1-second.txt
  echo; echo "################ $name"
  drive --log "$OUT/$name.jsonl" --cwd "$PLAIN" --perm allow,deny --turn "$PROMPT" "$@"
  summarize "$OUT/$name.jsonl"
  echo "-- can_use_tool requests (stdout), verbatim:"
  events "$OUT/$name.jsonl" 'select(.type=="control_request")'
  echo "-- our answers (stdin), verbatim:"
  jq -c 'select(.dir=="->" and .msg.type=="control_response") | .msg' "$OUT/$name.jsonl"
  echo "-- system/permission_denied events:"
  events "$OUT/$name.jsonl" 'select(.type=="system" and .subtype=="permission_denied") | {tool_name, message}'
  echo "-- tool results the model saw:"
  events "$OUT/$name.jsonl" 'select(.type=="user") | .message.content[] | select(.type=="tool_result") | {is_error, content}'
  echo "-- result.permission_denials: $(events "$OUT/$name.jsonl" 'select(.type=="result") | .permission_denials | map(.tool_input.command)')"
  echo "-- files on disk: $(cd "$PLAIN" && ls s1-*.txt 2>/dev/null | tr '\n' ' ')"
}

run a-host-only -- $BASE_FLAGS --permission-prompts host --session-id "$(new_uuid)"
run b-host-plus-initialize --init -- $BASE_FLAGS --permission-prompts host --session-id "$(new_uuid)"
run c-host-plus-prompt-tool-stdio -- $BASE_FLAGS --permission-prompts host --permission-prompt-tool stdio --session-id "$(new_uuid)"
# `claude --help` says --permission-prompts defaults to "host". Control: the hidden flag alone.
run d-prompt-tool-stdio-only -- $BASE_FLAGS --permission-prompt-tool stdio --session-id "$(new_uuid)"
# The host goes away while a request is pending: stdin closes with no answer (--perm close).
run e-stdin-closed-while-pending --perm close --timeout-s 90 -- $BASE_FLAGS --permission-prompt-tool stdio --session-id "$(new_uuid)"

echo; echo "## spend"
summarize --quiet "$OUT"/*.jsonl
