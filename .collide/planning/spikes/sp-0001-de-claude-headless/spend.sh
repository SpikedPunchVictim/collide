#!/bin/sh
# Prints the notional spend per script from the logs in out/<script>/, and the total.
# Per claude process: what it added to its session's total_cost_usd (spent_usd from
# summarize.mjs; a --resume'd process starts from the session's earlier total). A process killed
# before its first result reports no cost.
set -u
S1="$(cd "$(dirname "$0")" && pwd)"
cd "$S1"
for d in out/*/; do
  name=$(basename "$d")
  ls "$d"*.jsonl >/dev/null 2>&1 || continue
  node summarize.mjs --quiet "$d"*.jsonl |
    sed -n 's/.*spent_usd=\([0-9.]*\).*/\1/p' |
    awk -v n="$name" '{ s += $1 } END { printf "%-26s $%.4f\n", n, s }'
done | awk '{ print } { v = $2; gsub(/\$/, "", v); t += v } END { printf "%-26s $%.4f\n", "total", t }'
