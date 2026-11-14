#!/bin/sh
# Q1: (a) does the stream repeat usage per content block, compared with the same session's transcript;
#     (b) does a manual /compact sent on stdin give system/compact_boundary, and with what metadata.
# One session, three turns: read two files (parallel tool calls give several content blocks), /compact, a follow-up.
. "$(dirname "$0")/lib.sh"
D="$WORK/idle"
SID=$(new_uuid); echo "$SID" > "$OUT/session-id"
drive --log "$OUT/run.jsonl" --cwd "$D" --perm allow --timeout-s 240 \
  --turn "Read ledger1.js and ledger2.js completely. Then reply with one line: the last function name of each file." \
  --turn "/compact" \
  --turn "Reply with exactly one word: PONG" -- \
  $BASE_FLAGS --setting-sources user --session-id "$SID"
summarize "$OUT/run.jsonl" | grep -vE '^\.\. note \{"started'
echo "-- usage rows by message.id (id, blocks, input, cache_read, cache_write, write_1h, write_5m, output):"
usage_by_id "$OUT/run.jsonl" | sed 's/^/   /'
echo "-- stream vs transcript:"
node "$S9/usage-compare.mjs" "$OUT/run.jsonl" "$SID"
echo "-- compact-related events:"
events "$OUT/run.jsonl" 'select(.type=="system" and (.subtype|tostring|test("compact"))) | .' | cut -c1-900
events "$OUT/run.jsonl" 'select(.type=="user" and ((.message.content|tostring)|test("compact";"i"))) | {type, subtype, isSynthetic, content: (.message.content|tostring|.[0:300])}'
echo "-- result events:"
events "$OUT/run.jsonl" 'select(.type=="result") | {subtype, is_error, result: (.result|tostring|.[0:120]), usage: (.usage|{input_tokens,cache_read_input_tokens,cache_creation_input_tokens,output_tokens}), cost: .total_cost_usd}'
