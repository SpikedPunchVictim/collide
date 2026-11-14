#!/bin/bash
# Addendum to S9: how a PreToolUse deny, a hook error, a PostToolUse block, a host deny and a disallowed tool
# look in stream-json. Hooks come only from a spike-local --settings file (--setting-sources ""); every call
# carries --max-budget-usd 0.05 and --strict-mcp-config. Tool called: Bash `touch s9-x.txt` in work/p9 (needs an
# approval, so a host sees can_use_tool under --permission-prompt-tool stdio; the file shows whether it ran).
# Usage: ./07-hook-tool-deny.sh [case-name ...]   (default: all)
S9_BUDGET=0.05
. "$(dirname "$0")/lib.sh"
D="$WORK/p9"; mkdir -p "$D"
PROMPT='Run `touch s9-x.txt` with the Bash tool, then reply with one short sentence saying what happened.'
STDIO=(--permission-prompt-tool stdio)

# settings <file> <event> <timeout|""> : one Bash-matched command hook for <event>
settings() {
  local to=""; [ -n "$3" ] && to=", \"timeout\": $3"
  cat > "$1" <<JSON
{"autoMemoryEnabled":false,"hooks":{"$2":[{"matcher":"Bash","hooks":[{"type":"command","command":"$S9/hooks/tool-hook.sh"$to}]}]}}
JSON
}
# runcase <name> <hook-mode|none> <event> <timeout|""> <perm list|""> <extra claude flags...>
runcase() {
  local name="$1" mode="$2" ev="$3" to="$4" perm="$5"; shift 5
  local sf="$OUT/$name.settings.json" marks="$OUT/$name.marks"; : > "$marks"
  settings "$sf" "$ev" "$to"
  rm -f "$D/s9-x.txt"
  local p=(); [ -n "$perm" ] && p=(--perm "$perm")
  S9_DENY_MESSAGE="S9-HOSTDENY the host refuses this tool call" \
  drive --log "$OUT/$name.jsonl" --cwd "$D" --env "S9_HOOK_MODE=$mode" --env "S9_MARKS=$marks" ${p[@]+"${p[@]}"} --turn "${PROMPT_OVERRIDE:-$PROMPT}" -- \
    $BASE_FLAGS --setting-sources "" --strict-mcp-config --settings "$sf" "$@" > /dev/null
  echo "################ $name  (hook mode=$mode event=$ev timeout=${to:-default} perm=${perm:-none} flags: $*)"
  node "$S9/07-summarize.mjs" "$OUT/$name.jsonl"
  echo "  file created by the tool: $([ -f "$D/s9-x.txt" ] && echo YES || echo no)   hook calls: $(wc -l < "$marks" | tr -d ' ') [$(jq -r '.hook_event_name' "$marks" 2>/dev/null | sed 's/ mode=.*//' | tr '\n' ' ')]"
  echo
}
ONLY=("$@"); all=0; [ $# -eq 0 ] && all=1
w() { [ $all -eq 1 ] && return 0; for x in ${ONLY[@]+"${ONLY[@]}"}; do [ "$x" = "$1" ] && return 0; done; return 1; }

w c0-control-allow   && runcase c0-control-allow   none        PreToolUse ""  allow "${STDIO[@]}"
w c1-deny-json       && runcase c1-deny-json       deny-json   PreToolUse ""  allow "${STDIO[@]}"
w c2-deny-exit2      && runcase c2-deny-exit2      deny-exit2  PreToolUse ""  allow "${STDIO[@]}"
w c3a-ask-allow      && runcase c3a-ask-allow      ask         PreToolUse ""  allow "${STDIO[@]}"
w c3b-ask-deny       && runcase c3b-ask-deny       ask         PreToolUse ""  deny  "${STDIO[@]}"
w c4a-err1           && runcase c4a-err1           err1        PreToolUse ""  allow "${STDIO[@]}"
w c4b-badjson        && runcase c4b-badjson        badjson     PreToolUse ""  allow "${STDIO[@]}"
w c4c-timeout        && runcase c4c-timeout        sleep       PreToolUse 1   allow "${STDIO[@]}"
w c5a-post-block     && runcase c5a-post-block     post-block  PostToolUse "" allow "${STDIO[@]}"
w c5b-post-exit2     && runcase c5b-post-exit2     post-exit2  PostToolUse "" allow "${STDIO[@]}"
w c6a-host-deny      && runcase c6a-host-deny      none        PreToolUse ""  deny  "${STDIO[@]}"
w c6b-auto-deny      && runcase c6b-auto-deny      none        PreToolUse ""  ""
w c7a-disallow-tool  && runcase c7a-disallow-tool  none        PreToolUse ""  allow "${STDIO[@]}" --disallowedTools Bash
w c7b-disallow-rule  && runcase c7b-disallow-rule  none        PreToolUse ""  allow "${STDIO[@]}" --disallowedTools 'Bash(touch:*)'

# Probes: does the model see the stderr of a failing hook, or the text of an invalid-JSON hook?
ASKPROMPT='Run `touch s9-x.txt` with the Bash tool. Then quote verbatim every hook or system message you received about that call, or write NONE.'
w c4a2-err1-model-quote && PROMPT_OVERRIDE="$ASKPROMPT" runcase c4a2-err1-model-quote err1    PreToolUse "" allow "${STDIO[@]}"
w c4b2-badjson-model-quote && PROMPT_OVERRIDE="$ASKPROMPT" runcase c4b2-badjson-model-quote badjson PreToolUse "" allow "${STDIO[@]}"
# Same cases with --include-hook-events: do PreToolUse/PostToolUse hooks emit hook_started/hook_response?
HEV=(--include-hook-events)
w h1-deny-json-hev   && runcase h1-deny-json-hev   deny-json   PreToolUse ""  allow "${STDIO[@]}" "${HEV[@]}"
w h2-deny-exit2-hev  && runcase h2-deny-exit2-hev  deny-exit2  PreToolUse ""  allow "${STDIO[@]}" "${HEV[@]}"
w h3-ask-hev         && runcase h3-ask-hev         ask         PreToolUse ""  allow "${STDIO[@]}" "${HEV[@]}"
w h4a-err1-hev       && runcase h4a-err1-hev       err1        PreToolUse ""  allow "${STDIO[@]}" "${HEV[@]}"
w h4b-badjson-hev    && runcase h4b-badjson-hev    badjson     PreToolUse ""  allow "${STDIO[@]}" "${HEV[@]}"
w h4c-timeout-hev    && runcase h4c-timeout-hev    sleep       PreToolUse 1   allow "${STDIO[@]}" "${HEV[@]}"
w h5a-post-block-hev && runcase h5a-post-block-hev post-block  PostToolUse "" allow "${STDIO[@]}" "${HEV[@]}"
w h5b-post-exit2-hev && runcase h5b-post-exit2-hev post-exit2  PostToolUse "" allow "${STDIO[@]}" "${HEV[@]}"
w h0-control-hev     && runcase h0-control-hev     none        PreToolUse ""  allow "${STDIO[@]}" "${HEV[@]}"
exit 0
