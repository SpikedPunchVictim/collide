#!/bin/sh
# Q6, part 1: start the session that will sit idle. Reads four files (about 20K tokens of context on
# top of the start weight), under the plan's config (--setting-sources user). Records the end time;
# 06b-idle-resume.sh needs at least 61 minutes after it.
. "$(dirname "$0")/lib.sh"
D="$WORK/idle"
[ -f "$D/ledger1.js" ] || { echo "missing fixture $D (see 00-fixture.sh)"; exit 1; }
SID=$(new_uuid)
echo "$SID" > "$OUT/session-id"
drive --log "$OUT/start.jsonl" --cwd "$D" --timeout-s 180 \
  --turn "Read ledger1.js, ledger2.js, ledger3.js and ledger4.js completely (all lines). Then answer in four lines, one per file: the file name and the name of its last function. Nothing else." -- \
  $BASE_FLAGS --max-budget-usd 0.4 --setting-sources user --session-id "$SID"
date +%s > "$OUT/ended-at"
date > "$OUT/ended-at.txt"
summarize "$OUT/start.jsonl" | grep -vE '^\.\. note'
usage_by_id "$OUT/start.jsonl"
events "$OUT/start.jsonl" 'select(.type=="result") | .result'
echo "session=$SID ended=$(cat "$OUT/ended-at.txt")"
