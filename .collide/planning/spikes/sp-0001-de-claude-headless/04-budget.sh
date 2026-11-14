#!/bin/sh
# Q4: --max-budget-usd set tiny. What the stream and the exit code show when it is exceeded.
#   a  budget 0.0001, two turns: the first turn exceeds it; is the second turn still taken?
#   b  budget 0.002, one turn that needs several tool calls: does it stop mid-turn?
#   c  a session that has spent X (one turn, budget 0.25) is resumed in a new process with budget
#      1.5 X, above what one resumed turn costs (about 0.8 X) but below the session's total after
#      it (about 1.8 X): does the budget count the session's earlier cost or only this process's?
. "$(dirname "$0")/lib.sh"
D="$WORK/plain/budget"; mkdir -p "$D"
for f in one two three four; do echo "file $f" >"$D/$f.txt"; done
FLAGS="-p --input-format stream-json --output-format stream-json --verbose --model $MODEL"

show() {
  summarize "$OUT/$1.jsonl"
  echo "-- results: $(events "$OUT/$1.jsonl" 'select(.type=="result") | {subtype, is_error, num_turns, stop_reason, total_cost_usd, errors, result}')"
}

echo "################ a: --max-budget-usd 0.0001, two turns"
drive --log "$OUT/a.jsonl" --cwd "$D" --timeout-s 120 --turn "Reply with exactly: ONE" --turn "Reply with exactly: TWO" -- \
  $FLAGS --max-budget-usd 0.0001 --session-id "$(new_uuid)"
show a

echo; echo "################ b: --max-budget-usd 0.002, one turn with several sequential tool calls"
drive --log "$OUT/b.jsonl" --cwd "$D" --timeout-s 180 \
  --turn "Read one.txt, then two.txt, then three.txt, then four.txt with the Read tool, one call per message, strictly one after the other. Then reply with their contents joined by commas." -- \
  $FLAGS --max-budget-usd 0.002 --session-id "$(new_uuid)"
show b

echo; echo "################ c: resume a session with a budget between this process's spend and the session's total"
SID=$(new_uuid)
drive --log "$OUT/c-first.jsonl" --cwd "$D" --timeout-s 120 --turn "Reply with exactly: FIRST" -- \
  $FLAGS --max-budget-usd 0.25 --session-id "$SID"
SPENT=$(events "$OUT/c-first.jsonl" 'select(.type=="result") | .total_cost_usd' | tail -1)
CAP=$(node -e "console.log((Number(process.argv[1]) * 1.5).toFixed(6))" "$SPENT")
echo "-- first process: session total_cost_usd=$SPENT; resuming with --max-budget-usd $CAP"
drive --log "$OUT/c-resume.jsonl" --cwd "$D" --timeout-s 120 --turn "Reply with exactly: SECOND" -- \
  $FLAGS --max-budget-usd "$CAP" --resume "$SID"
show c-resume
echo "-- model_usage_since_last_result cost of the resumed turn: $(events "$OUT/c-resume.jsonl" 'select(.type=="result") | [.model_usage_since_last_result // {} | .[] | .costUSD] | add')"

echo; echo "## spend (can exceed the cap: the check runs after a call is paid for)"
summarize --quiet "$OUT"/*.jsonl
