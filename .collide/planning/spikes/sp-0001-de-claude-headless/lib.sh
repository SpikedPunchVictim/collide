# Shared settings for the S1 scripts. Source it; do not run it.
set -eu

S1="$(cd "$(dirname "$0")" && pwd)"
WORK="$S1/work"
MODEL="${S1_MODEL:-haiku}"
BUDGET="${S1_BUDGET:-0.25}"

# out dir per script: out/<script name without .sh>
OUT="$S1/out/$(basename "$0" .sh)"
rm -rf "$OUT"
mkdir -p "$OUT"

# Flags every measured call carries unless the script says otherwise.
BASE_FLAGS="-p --input-format stream-json --output-format stream-json --verbose --model $MODEL --max-budget-usd $BUDGET"

drive() { node "$S1/driver.mjs" "$@"; }
summarize() { node "$S1/summarize.mjs" "$@"; }

ensure_fixture() {
  [ -f "$WORK/.fixture-ok" ] || "$S1/00-fixture.sh"
}

# jq over the "<-" messages of a driver log
events() { jq -c "select(.dir == \"<-\") | .msg | $2" "$1"; }

new_uuid() { uuidgen | tr 'A-Z' 'a-z'; }

# Questions 6, 7 and 9: what loaded. The prompt asks the model to repeat canaries; the init
# event and the marker file ($S1_MARKS, written by hooks and the canary MCP server) do not
# depend on the model.
CANARY_PROMPT="Without using any tools, answer in one line of the form: canaries=<every S1 canary word that appears in your instructions or memory files, comma-separated, or none>; skills=<every available skill whose name starts with s1-, or none>; mcp=<every tool whose name starts with mcp__s1, or none>"

loading_run() { # name cwd claude-flags...   (extra driver options, e.g. --env K=V: $LR_DRIVER_OPTS)
  name=$1; cwd=$2; shift 2
  marks="$OUT/$name.marks"; : >"$marks"
  # shellcheck disable=SC2086 # LR_DRIVER_OPTS is a word list on purpose
  drive --log "$OUT/$name.jsonl" --cwd "$cwd" --env "S1_MARKS=$marks" ${LR_DRIVER_OPTS:-} --turn "$CANARY_PROMPT" -- "$@"
  init='select(.type=="system" and .subtype=="init")'
  echo "-- marks (hooks, MCP starts, InstructionsLoaded):"
  sed -E 's/ env=\[[^]]*\]//; s/"session_id":"[^"]*",?//; s/"transcript_path":"[^"]*",?//; s#'"$S1"'/##g' "$marks" | sort | uniq -c | sed 's/^/   /'
  echo "-- init.mcp_servers: $(events "$OUT/$name.jsonl" "$init | .mcp_servers | map(\"\(.name)=\(.status)\") | join(\" \")" | head -1)"
  echo "-- init.skills (s1-*): $(events "$OUT/$name.jsonl" "$init | .skills | map(select(test(\"s1\"))) | join(\" \")" | head -1)"
  echo "-- init.tools (mcp__s1*): $(events "$OUT/$name.jsonl" "$init | .tools | map(select(startswith(\"mcp__s1\"))) | join(\" \")" | head -1)"
  echo "-- init.plugins: $(events "$OUT/$name.jsonl" "$init | .plugins | map(.name) | join(\" \")" | head -1)"
  echo "-- init.memory_paths: $(events "$OUT/$name.jsonl" "$init | .memory_paths" | head -1 | sed "s#$S1/##g")"
  echo "-- init.agents: $(events "$OUT/$name.jsonl" "$init | .agents | join(\" \")" | head -1)"
  echo "-- model answer: $(events "$OUT/$name.jsonl" 'select(.type=="result") | .result')"
  echo "-- $(summarize --quiet "$OUT/$name.jsonl" | sed 's/^= [^:]*: //')"
}
