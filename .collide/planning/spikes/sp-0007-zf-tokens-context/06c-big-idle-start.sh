#!/bin/sh
# Q6, second (larger) pair, part 1: like 06a but with eight files read (about 100K tokens of context).
# Read-only on the files in work/q4. 06d-big-pair.sh needs at least 61 minutes after it.
. "$(dirname "$0")/lib.sh"
D="$WORK/q4"
SID=$(new_uuid)
echo "$SID" > "$OUT/session-id"
drive --log "$OUT/start.jsonl" --cwd "$D" --timeout-s 240 \
  --turn "Read ledger1.js, ledger2.js, ledger3.js and ledger4.js completely (all lines)." \
  --turn "Now read ledger5.js, ledger6.js, ledger7.js and ledger8.js completely (all lines). Then answer in eight lines, one per file: the file name and the name of its last function. Nothing else." -- \
  $BASE_FLAGS --max-budget-usd 0.6 --setting-sources user --session-id "$SID"
date +%s > "$OUT/ended-at"
date > "$OUT/ended-at.txt"
summarize "$OUT/start.jsonl" | grep -vE '^\.\. note'
echo "timeline: $(timeline "$OUT/start.jsonl")"
events "$OUT/start.jsonl" 'select(.type=="result") | .result'
echo "session=$SID ended=$(cat "$OUT/ended-at.txt")"
