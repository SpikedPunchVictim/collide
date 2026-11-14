#!/bin/bash
# Q4 extra: what the binary does with CLAUDE_CODE_AUTO_COMPACT_WINDOW values, read from get_context_usage
# (maxTokens, autoCompactThreshold, autocompactSource) under the plan's config: none line.
. "$(dirname "$0")/lib.sh"
D="$WORK/empty"
export S9_PASS=CLAUDE_CODE_AUTO_COMPACT_WINDOW
for W in unset 30000 100000 200k 500000 2000000 abc; do
  if [ "$W" = unset ]; then unset CLAUDE_CODE_AUTO_COMPACT_WINDOW; else export CLAUDE_CODE_AUTO_COMPACT_WINDOW=$W; fi
  node "$S9/context-usage.mjs" "$D" -- -p --input-format stream-json --output-format stream-json --verbose --model haiku --max-budget-usd 0.25 --session-id "$(new_uuid)" --setting-sources "" > "$OUT/$W.json" 2> "$OUT/$W.err"
  echo "window=$W -> $(jq -r '.response.response | "maxTokens=\(.maxTokens) autoCompactThreshold=\(.autoCompactThreshold) source=\(.autocompactSource) buffer=\(.categories|map(select(.id=="autocompact_buffer"))|.[0].tokens)"' "$OUT/$W.json") $(head -c 200 "$OUT/$W.err" | tr '\n' ' ')"
done

echo "-- precedence under config: user (the user's settings set a window; source=settings without the variable)"
for W in unset 150000; do
  if [ "$W" = unset ]; then unset CLAUDE_CODE_AUTO_COMPACT_WINDOW; else export CLAUDE_CODE_AUTO_COMPACT_WINDOW=$W; fi
  node "$S9/context-usage.mjs" "$D" -- -p --input-format stream-json --output-format stream-json --verbose --model haiku --max-budget-usd 0.25 --session-id "$(new_uuid)" --setting-sources user > "$OUT/user-$W.json" 2> "$OUT/user-$W.err"
  echo "user config, window=$W -> $(jq -r '.response.response | "maxTokens=\(.maxTokens) autoCompactThreshold=\(.autoCompactThreshold) source=\(.autocompactSource)"' "$OUT/user-$W.json")"
done
