#!/bin/sh
# Q1: multi-turn stream-json with --session-id; is --verbose required?
. "$(dirname "$0")/lib.sh"
PLAIN="$WORK/plain"; mkdir -p "$PLAIN"

echo "## A. -p --output-format stream-json, text prompt, NO --verbose"
rc=0
( cd "$PLAIN" && "$S1/claude-clean.sh" -p --output-format stream-json --model "$MODEL" --max-budget-usd "$BUDGET" \
    "Reply with exactly: ONE" >"$OUT/a.stdout" 2>"$OUT/a.stderr" ) || rc=$?
echo "exit=$rc"; echo "stderr:"; cat "$OUT/a.stderr"; echo "stdout lines: $(wc -l <"$OUT/a.stdout")"
summarize "$OUT/a.stdout"

echo; echo "## B. -p --input-format stream-json --output-format stream-json, NO --verbose (one turn on stdin)"
drive --log "$OUT/b.jsonl" --cwd "$PLAIN" --timeout-s 120 --turn "Reply with exactly: ONE" -- \
  -p --input-format stream-json --output-format stream-json --model "$MODEL" --max-budget-usd "$BUDGET"
summarize "$OUT/b.jsonl"

echo; echo "## C. two user turns on stdin, --verbose, --session-id"
SID=$(new_uuid); echo "session-id=$SID"
drive --log "$OUT/c.jsonl" --cwd "$PLAIN" \
  --turn "Remember the codeword PLUM-42. Reply with exactly: OK" \
  --turn "What was the codeword? Reply with only the codeword." -- \
  $BASE_FLAGS --session-id "$SID"
summarize "$OUT/c.jsonl"
echo "distinct session_id values on stdout: $(events "$OUT/c.jsonl" '.session_id // empty' | sort -u | tr '\n' ' ')"
echo "result texts: $(events "$OUT/c.jsonl" 'select(.type=="result") | .result' | tr '\n' ' ')"
echo "per-result total_cost_usd (cumulative?): $(events "$OUT/c.jsonl" 'select(.type=="result") | .total_cost_usd' | tr '\n' ' ')"
echo "init model: $(events "$OUT/c.jsonl" 'select(.type=="system" and .subtype=="init") | .model')"

echo; echo "## D. same, with an initialize control_request first (what the Agent SDK sends)"
drive --log "$OUT/d.jsonl" --cwd "$PLAIN" --init --turn "Reply with exactly: OK" -- $BASE_FLAGS --session-id "$(new_uuid)"
summarize "$OUT/d.jsonl"
echo "initialize response keys: $(events "$OUT/d.jsonl" 'select(.type=="control_response") | .response.response | keys' | head -1)"

echo; echo "## spend"
summarize --quiet "$OUT"/b.jsonl "$OUT"/c.jsonl "$OUT"/d.jsonl
