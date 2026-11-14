#!/bin/bash
# Q4: the effect of three variables set in a phase's environment, on what the stream shows.
# Base line: the plan's "config: none", cleanest usable form (03, variant i). The variables are passed
# deliberately: driver --env K=V reaches claude-clean.sh, which strips every CLAUDE* name not listed in
# S9_PASS, so CLAUDE_CODE_AUTO_COMPACT_WINDOW is kept that way.
#   A  CLAUDE_CODE_AUTO_COMPACT_WINDOW: unset, 30000, 100000, 120000, 150000. Five turns that read 8 files (context grows to about 100K).
#   B  BASH_MAX_OUTPUT_LENGTH: unset, 5000. One Bash call that prints about 49K characters.
#   C  MAX_MCP_OUTPUT_TOKENS: unset, 1000. One MCP tool call that returns 150000 characters.
. "$(dirname "$0")/lib.sh"
D="$WORK/q4"
LINE=(--setting-sources "" --settings '{"autoMemoryEnabled":false}' --disallowedTools CronCreate ScheduleWakeup RemoteTrigger PushNotification EnterWorktree ExitWorktree Workflow SendMessage)
export S9_PASS="CLAUDE_CODE_AUTO_COMPACT_WINDOW"

echo "######## A. CLAUDE_CODE_AUTO_COMPACT_WINDOW (binary: values below the minimum are clamped; threshold = window - 33000, minimum window 100000, see 04b)"
Q4="$WORK/q4"
for W in unset 30000 100000 120000 150000; do
  envopt=(); [ "$W" = unset ] || envopt=(--env "CLAUDE_CODE_AUTO_COMPACT_WINDOW=$W")
  drive --log "$OUT/A-$W.jsonl" --cwd "$Q4" --timeout-s 300 ${envopt[@]+"${envopt[@]}"} \
    --turn "Read ledger1.js and ledger2.js completely. Reply with the last function name of each." \
    --turn "Read ledger3.js and ledger4.js completely. Reply with the last function name of each." \
    --turn "Read ledger5.js and ledger6.js completely. Reply with the last function name of each." \
    --turn "Read ledger7.js and ledger8.js completely. Reply with the last function name of each." \
    --turn "Reply with exactly one word: PONG" -- $BASE_FLAGS --max-budget-usd 0.5 "${LINE[@]}" --session-id "$(new_uuid)" >/dev/null
  echo "== window=$W  $(summarize --quiet "$OUT/A-$W.jsonl" | sed 's/^= [^:]*: //')"
  echo "   timeline (context per response, R = result): $(timeline "$OUT/A-$W.jsonl")"
  echo "   stderr/warnings: $(jq -r 'select(.dir=="stderr") | .msg.line' "$OUT/A-$W.jsonl" | head -3 | tr '\n' '|')"
done

echo; echo "######## B. BASH_MAX_OUTPUT_LENGTH (and, for comparison, the settings key bashOutputMaxChars the binary documents)"
# case = <label>:<seq upper bound>:<env value or unset>:<settings json>
for CASE in "9000-default:9000:unset:{}" "9000-env5000:9000:5000:{}" "3000-default:3000:unset:{}" "3000-env5000:3000:5000:{}" "3000-setting5000:3000:unset:{\"bashOutputMaxChars\":5000}"; do
  IFS=: read -r LABEL N V SET <<EOF2
$CASE
EOF2
  envopt=(); [ "$V" = unset ] || envopt=(--env "BASH_MAX_OUTPUT_LENGTH=$V")
  setj=$(echo "{\"autoMemoryEnabled\":false}" | jq -c --argjson x "$SET" '. + $x')
  drive --log "$OUT/B-$LABEL.jsonl" --cwd "$D" --timeout-s 120 ${envopt[@]+"${envopt[@]}"} \
    --turn "Use the Bash tool to run exactly: seq 1 $N . Then reply with the single word DONE." -- \
    $BASE_FLAGS --setting-sources "" --settings "$setj" --disallowedTools CronCreate ScheduleWakeup RemoteTrigger PushNotification EnterWorktree ExitWorktree Workflow SendMessage --allowedTools Bash --session-id "$(new_uuid)" >/dev/null
  echo "== $LABEL (seq 1 $N prints $(seq 1 $N | wc -c | tr -d ' ') chars)  $(summarize --quiet "$OUT/B-$LABEL.jsonl" | sed 's/^= [^:]*: //')"
  events "$OUT/B-$LABEL.jsonl" 'select(.type=="user") | .message.content[]? | select(.type=="tool_result") | [.is_error, (.content|tostring|length), (.content|tostring|.[0:70])] | tostring' | sed 's/^/   tool_result [is_error, chars, head]: /'
  events "$OUT/B-$LABEL.jsonl" 'select(.type=="user") | .tool_use_result? | select(type=="object") | "persistedOutputSize=\(.persistedOutputSize) stdout_chars=\(.stdout|length)"' | sed 's/^/   tool_use_result: /'
  echo "   next response ctx: $(timeline "$OUT/B-$LABEL.jsonl")"
done

echo; echo "######## C. MAX_MCP_OUTPUT_TOKENS (the 25000-token default is in the binary)"
for CASE in "150k-default:150000:unset" "150k-1000:150000:1000" "20k-default:20000:unset" "20k-1000:20000:1000"; do
  IFS=: read -r LABEL N V <<EOF2
$CASE
EOF2
  envopt=(); [ "$V" = unset ] || envopt=(--env "MAX_MCP_OUTPUT_TOKENS=$V")
  drive --log "$OUT/C-$LABEL.jsonl" --cwd "$D" --timeout-s 120 ${envopt[@]+"${envopt[@]}"} \
    --turn "Call the tool mcp__s9big__big once with chars=$N. Then reply with the single word DONE." -- \
    $BASE_FLAGS "${LINE[@]}" --mcp-config "$WORK/mcp-big.json" --strict-mcp-config --allowedTools mcp__s9big__big --session-id "$(new_uuid)" >/dev/null
  echo "== $LABEL  $(summarize --quiet "$OUT/C-$LABEL.jsonl" | sed 's/^= [^:]*: //')"
  events "$OUT/C-$LABEL.jsonl" 'select(.type=="user") | .message.content[]? | select(.type=="tool_result") | select((.content|tostring|test("tool_reference"))|not) | [.is_error, (.content|tostring|length), (.content|tostring|.[0:90])] | tostring' | sed 's/^/   tool_result [is_error, chars, head]: /'
  echo "   timeline: $(timeline "$OUT/C-$LABEL.jsonl")"
done
