#!/bin/sh
# Q5: the final message as the phase report; resume by session id in a new process; cancel
# mid-turn with the interrupt control request, SIGINT and SIGTERM.
. "$(dirname "$0")/lib.sh"
D="$WORK/plain/report"; mkdir -p "$D"
printf 'quokka is the first word of this file\n' >"$D/notes.txt"
# Claude Code blocks a foreground `sleep N` command (measured: "Blocked: sleep 41.5 followed by ..."),
# so the long-running tool call is a script that sleeps.
printf 'sleep "$1"\necho slept\n' >"$D/slow.sh"
HOSTED="$BASE_FLAGS --permission-prompt-tool stdio"

echo "################ a: final message as the report"
SID=$(new_uuid); echo "session-id=$SID"
drive --log "$OUT/a-report.jsonl" --cwd "$D" \
  --turn "First say 'Reading now.' Then read notes.txt with the Read tool. Then reply with exactly three lines: REPORT, then the first word of the file, then END." -- \
  $HOSTED --session-id "$SID"
summarize "$OUT/a-report.jsonl" | grep -E '^(<- (assistant|result|user)|== exit)'
echo "-- result.result: $(events "$OUT/a-report.jsonl" 'select(.type=="result") | .result')"
echo "-- text of the last assistant message: $(events "$OUT/a-report.jsonl" 'select(.type=="assistant") | [.message.content[] | select(.type=="text") | .text] | join("")' | grep -v '^""$' | tail -1)"
echo "-- every assistant text block: $(events "$OUT/a-report.jsonl" 'select(.type=="assistant") | .message.content[] | select(.type=="text") | .text' | tr '\n' ' ')"

echo; echo "################ b: resume by session id in a new process"
drive --log "$OUT/b-resume.jsonl" --cwd "$D" --turn "What was the first word of notes.txt? Reply with only that word." -- \
  $HOSTED --resume "$SID"
summarize "$OUT/b-resume.jsonl" | grep -vE '^\.\. note'
echo "-- session_id values on stdout: $(events "$OUT/b-resume.jsonl" '.session_id // empty' | sort -u | tr '\n' ' ')"
echo "-- SessionStart hook names: $(events "$OUT/b-resume.jsonl" 'select(.type=="system" and .subtype=="hook_started") | .hook_name' | sort -u | tr '\n' ' ')"

sleeper() { echo "Use the Bash tool to run exactly this command in the foreground and nothing else: sh slow.sh $1 . Then reply DONE."; }
survivors() { echo "-- processes still running 'sleep $1' after exit: $(pgrep -f "sleep $1" | wc -l | tr -d ' ')"; }

echo; echo "################ c: interrupt control request mid-tool, then a second turn"
drive --log "$OUT/c-interrupt.jsonl" --cwd "$D" --perm allow --cancel interrupt --cancel-on-tool Bash --cancel-delay-ms 3000 \
  --turn "$(sleeper 41.5)" --turn "Reply with exactly: AFTER" -- $HOSTED --session-id "$(new_uuid)"
summarize "$OUT/c-interrupt.jsonl" | grep -vE '^\.\. note \{"started'
echo "-- control_response to the interrupt: $(events "$OUT/c-interrupt.jsonl" 'select(.type=="control_response" and .response.request_id=="host-interrupt-1") | .response')"
echo "-- results: $(events "$OUT/c-interrupt.jsonl" 'select(.type=="result") | {subtype, is_error, stop_reason, result, errors}')"
echo "-- tool_result after interrupt: $(events "$OUT/c-interrupt.jsonl" 'select(.type=="user") | .message.content[]? | select(.type=="tool_result") | {is_error, content: (.content|tostring|.[0:160])}')"
survivors 41.5

for SIG in SIGINT SIGTERM; do
  case $SIG in SIGINT) S=42.5 ;; SIGTERM) S=43.5 ;; esac
  echo; echo "################ d: $SIG to claude mid-tool, then resume the session in a new process"
  SID=$(new_uuid); echo "session-id=$SID"
  # stdin stays open for 15 s after a result, so an exit before that is the signal's doing.
  drive --log "$OUT/d-$SIG.jsonl" --cwd "$D" --perm allow --cancel "$SIG" --cancel-on-tool Bash --cancel-delay-ms 3000 \
    --hold-after-cancel-s 15 \
    --turn "$(sleeper $S)" -- $HOSTED --session-id "$SID"
  summarize "$OUT/d-$SIG.jsonl" | grep -vE '^\.\. note \{"started'
  survivors $S
  drive --log "$OUT/d-$SIG-resume.jsonl" --cwd "$D" --turn "Did your last Bash command finish? Answer in one short sentence." -- \
    $HOSTED --resume "$SID"
  summarize "$OUT/d-$SIG-resume.jsonl" | grep -E '^(<- (assistant|result)|== exit|!!)'
done

echo; echo "## spend"
summarize --quiet "$OUT"/*.jsonl
