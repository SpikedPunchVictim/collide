#!/bin/bash
# Q6, part 2: the pair, for cost. Needs 06a-idle-start.sh to have run at least 61 minutes ago
# (the 1-hour cache has then expired for that session). Refuses to run earlier unless S9_FORCE=1.
#   A  cold resume: --resume <session> with a follow-up question (the 1-hour cache has expired).
#   B  fresh session, same config and cwd, whose first prompt is a short handoff note (the plan's
#      on_cold: fresh) followed by the same question.
#   C  (supplement) A's session resumed again right after A: a warm resume, for comparison.
# Both A and B use --setting-sources user (the plan's config: user) and the same model.
S9_KEEP_OUT=1
. "$(dirname "$0")/lib.sh"
START="$S9/out/06a-idle-start"
ended=$(cat "$START/ended-at"); SID=$(cat "$START/session-id")
idle=$(( $(date +%s) - ended ))
echo "idle for ${idle}s (need >= 3660s)"
if [ "$idle" -lt 3660 ] && [ -z "${S9_FORCE:-}" ]; then echo "too early; run later"; exit 1; fi
D="$WORK/idle"
OUTP="$S9/out/06b-idle-resume"; rm -rf "$OUTP"; mkdir -p "$OUTP"
Q="Which of the four ledger files had a last function named calc3_119, and what were the last function names of all four files? Answer in one line, without using tools."
NOTE="Handoff note from an earlier session in this directory. In that session you read ledger1.js, ledger2.js, ledger3.js and ledger4.js completely. Each file has 120 functions named calcN_0 to calcN_119 (N is the file number) and no other code. You reported the last function of each: ledger1.js calc1_119, ledger2.js calc2_119, ledger3.js calc3_119, ledger4.js calc4_119. Nothing else is open."
FLAGS="$BASE_FLAGS --max-budget-usd 0.5"
drive --log "$OUTP/A-cold-resume.jsonl" --cwd "$D" --timeout-s 180 --turn "$Q" -- $FLAGS --setting-sources user --resume "$SID" >/dev/null
drive --log "$OUTP/B-fresh-handoff.jsonl" --cwd "$D" --timeout-s 180 --turn "$NOTE
$Q" -- $FLAGS --setting-sources user --session-id "$(new_uuid)" >/dev/null
drive --log "$OUTP/C-warm-resume.jsonl" --cwd "$D" --timeout-s 180 --turn "$Q" -- $FLAGS --setting-sources user --resume "$SID" >/dev/null
for n in A-cold-resume B-fresh-handoff C-warm-resume; do
  echo "== $n: $(summarize --quiet "$OUTP/$n.jsonl" | sed 's/^= [^:]*: //')"
  echo "   answer: $(events "$OUTP/$n.jsonl" 'select(.type=="result") | .result')"
  echo "   first response (input, read, write 1h, write 5m, output): $(usage_by_id "$OUTP/$n.jsonl" | head -1 | cut -f3-8 | tr '\t' ' ')   ctx=$(usage_by_id "$OUTP/$n.jsonl" | head -1 | awk -F'\t' '{print $3+$4+$5}')"
  events "$OUTP/$n.jsonl" 'select(.type=="result") | .model_usage_since_last_result | to_entries[] | "   model_usage_since_last_result \(.key): in=\(.value.inputTokens) out=\(.value.outputTokens) cache_read=\(.value.cacheReadInputTokens) cache_write=\(.value.cacheCreationInputTokens) (1h=\(.value.cacheCreationEphemeral1hInputTokens) 5m=\(.value.cacheCreationEphemeral5mInputTokens)) costUSD=\(.value.costUSD)"' | tr -d '"'
done
