#!/bin/sh
# Q3: --agents <file> built from a .claude/agents/*.md-style definition, selected with --agent.
#   a  no clash (plain dir): does a file path work under --print? Which model runs when the
#      definition says `model: sonnet` and we pass --model haiku?
#   b  control for a: no --model, default model haiku from --settings. Does `model: sonnet` apply?
#   c  name clash: the worktree has its own .claude/agents/collide-test-builder.md (WTAGENT).
#   d  control for c: worktree only, no --agents. Proves the worktree definition is selectable.
# Each agent's prompt says to begin every reply with its marker word (SNAPAGENT / WTAGENT).
. "$(dirname "$0")/lib.sh"
ensure_fixture
AGENTS="$WORK/snapshot/agents.json"
ASK="Say hello in three words."
FLAGS_NO_MODEL="-p --input-format stream-json --output-format stream-json --verbose --max-budget-usd $BUDGET"

report() {
  summarize "$OUT/$1.jsonl" | grep -E '^(<- (system/init|assistant|result)|== exit|!!)'
  echo "-- init.agents (s1-relevant): $(events "$OUT/$1.jsonl" 'select(.type=="system" and .subtype=="init") | .agents' | head -1)"
  echo "-- init.model: $(events "$OUT/$1.jsonl" 'select(.type=="system" and .subtype=="init") | .model' | head -1)"
  echo "-- result.modelUsage models: $(events "$OUT/$1.jsonl" 'select(.type=="result") | .modelUsage | keys')"
  echo "-- result.result: $(events "$OUT/$1.jsonl" 'select(.type=="result") | .result')"
}

echo "################ a: --agents FILE --agent collide-test-builder --model haiku (plain dir)"
drive --log "$OUT/a.jsonl" --cwd "$WORK/plain" --turn "$ASK" -- \
  $BASE_FLAGS --agents "$AGENTS" --agent collide-test-builder --session-id "$(new_uuid)"
report a

echo; echo "################ b: same, no --model; default model haiku via --settings"
drive --log "$OUT/b.jsonl" --cwd "$WORK/plain" --turn "$ASK" -- \
  $FLAGS_NO_MODEL --settings '{"model":"haiku"}' --agents "$AGENTS" --agent collide-test-builder --session-id "$(new_uuid)"
report b

echo; echo "################ c: name clash in the worktree, --agents FILE --agent collide-test-builder --model haiku"
drive --log "$OUT/c.jsonl" --cwd "$WORK/wt" --turn "$ASK" -- \
  $BASE_FLAGS --agents "$AGENTS" --agent collide-test-builder --session-id "$(new_uuid)"
report c

echo; echo "################ d: worktree, no --agents, --agent collide-test-builder --model haiku"
drive --log "$OUT/d.jsonl" --cwd "$WORK/wt" --turn "$ASK" -- \
  $BASE_FLAGS --agent collide-test-builder --session-id "$(new_uuid)"
report d

echo; echo "## argv check: the agent prompt is not in argv (only the file path is)"
jq -r 'select(.dir=="note" and .msg.args) | .msg.args | join(" ")' "$OUT/a.jsonl" | grep -c SNAPAGENT || true

echo; echo "## spend"
summarize --quiet "$OUT"/*.jsonl
