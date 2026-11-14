#!/bin/bash
# Q2: cache tier seen in every log under out/ (claude.ai login, apiKeySource "none"): 1-hour and 5-minute
# cache writes, from assistant usage (per distinct message.id) and from result.model_usage_since_last_result.
cd "$(dirname "$0")"
echo "apiKeySource values in init events: $(cat out/*/*.jsonl | jq -r 'select(.dir=="<-" and .msg.type=="system" and .msg.subtype=="init") | .msg.apiKeySource' | sort | uniq -c | tr '\n' ' ')"
echo "ANTHROPIC_* variable names in this environment: $(env | sed -n 's/^\(ANTHROPIC[A-Z_]*\)=.*/\1/p' | tr '\n' ' ')(none listed means none)"
cat out/*/*.jsonl | jq -s -r '
  [.[] | select(.dir=="<-" and .msg.type=="assistant") | .msg.message | {id, u: .usage}] | unique_by(.id) |
  "assistant messages (distinct ids)=\(length)  ephemeral_1h writes=\(map(.u.cache_creation.ephemeral_1h_input_tokens // 0)|add)  ephemeral_5m writes=\(map(.u.cache_creation.ephemeral_5m_input_tokens // 0)|add)  messages with any 5m write=\(map(select((.u.cache_creation.ephemeral_5m_input_tokens // 0) > 0))|length)"'
echo "results whose model_usage_since_last_result shows a 5m write:"
cat out/*/*.jsonl | jq -c 'select(.dir=="<-" and .msg.type=="result") | .msg | select([.model_usage_since_last_result[]?.cacheCreationEphemeral5mInputTokens] | add > 0) | {local_command: (.local_command // false), num_turns, w5m: [.model_usage_since_last_result[]?.cacheCreationEphemeral5mInputTokens], w1h: [.model_usage_since_last_result[]?.cacheCreationEphemeral1hInputTokens]}' | sort | uniq -c
