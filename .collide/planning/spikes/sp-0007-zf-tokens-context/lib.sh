# Shared settings for the S9 scripts. Source it; do not run it.
set -eu

S9="$(cd "$(dirname "$0")" && pwd)"
WORK="$S9/work"
MODEL="${S9_MODEL:-haiku}"
BUDGET="${S9_BUDGET:-0.25}"

# out dir per script: out/<script name without .sh>
OUT="$S9/out/$(basename "$0" .sh)"
if [ -z "${S9_KEEP_OUT:-}" ]; then rm -rf "$OUT"; fi
mkdir -p "$OUT"

# Flags every measured call carries unless the script says otherwise (S1's launch line, less the
# permission prompt tool: S9 scripts either use tools that need no prompt or pass --perm to the driver).
BASE_FLAGS="-p --input-format stream-json --output-format stream-json --verbose --model $MODEL --max-budget-usd $BUDGET"

drive() { node "$S9/driver.mjs" "$@"; }
summarize() { node "$S9/summarize.mjs" "$@"; }

# jq over the "<-" messages of a driver log
events() { jq -c "select(.dir == \"<-\") | .msg | $2" "$1"; }

new_uuid() { uuidgen | tr 'A-Z' 'a-z'; }

# One line per distinct assistant message.id of a driver log: context size and usage.
# context = input + cache_read + cache_creation (the prompt the model saw for that response).
usage_by_id() {
  jq -r 'select(.dir=="<-" and .msg.type=="assistant") | .msg
    | [.message.id, (.message.content|map(.type)|join("+")), .message.usage.input_tokens, .message.usage.cache_read_input_tokens, .message.usage.cache_creation_input_tokens, (.message.usage.cache_creation.ephemeral_1h_input_tokens // "-"), (.message.usage.cache_creation.ephemeral_5m_input_tokens // "-"), .message.usage.output_tokens] | @tsv' "$1"
}

# Timeline of a driver log: one token per assistant message.id (its context size), plus compaction
# events and results, in stream order. Example: "14392 36653 [compacting] [compact_boundary auto pre=67460 post=14858] R 38148 R"
timeline() {
  jq -r -s '[.[] | select(.dir=="<-") | .msg] | reduce .[] as $m ({out: [], last: ""};
    if $m.type=="assistant" and ($m.message.id != .last) then .out += [($m.message.usage | ((.input_tokens+.cache_read_input_tokens+.cache_creation_input_tokens)|tostring))] | .last = $m.message.id
    elif $m.type=="system" and $m.subtype=="status" and $m.status=="compacting" then .out += ["[compacting]"]
    elif $m.type=="system" and $m.subtype=="compact_boundary" then .out += ["[compact_boundary " + $m.compact_metadata.trigger + " pre=" + ($m.compact_metadata.pre_tokens|tostring) + " post=" + ($m.compact_metadata.post_tokens|tostring) + "]"]
    elif $m.type=="result" then .out += ["R"]
    else . end) | .out | join(" ")' "$1"
}
