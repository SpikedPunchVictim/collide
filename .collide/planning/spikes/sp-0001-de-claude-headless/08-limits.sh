#!/bin/sh
# Q8: usage and rate limits. No limit is triggered here (that would need an exhausted account).
# Read only. Three sources:
#   1  the installed claude binary: the stream-json schemas for rate_limit_event, the assistant
#      message's error field and the result subtypes, and the code that builds the limit message
#   2  the rate_limit_event that 01-multiturn.sh recorded live (run 01 first)
#   3  local transcripts under ~/.claude/projects: the SHAPE of any assistant message that
#      reported a limit. Keys and types only; the text is masked; no paths or content are printed.
# Spends nothing.
. "$(dirname "$0")/lib.sh"
BIN=$(python3 -c 'import os,shutil;print(os.path.realpath(shutil.which("claude")))')
echo "## binary: $BIN"

python3 -I - "$BIN" <<'EOF'
import mmap, re, sys
with open(sys.argv[1], 'rb') as f:
    d = mmap.mmap(f.fileno(), 0, access=mmap.ACCESS_READ)

def show(title, pat, before=0, after=600, n=1):
    print(f"\n### {title}")
    hits = list(re.finditer(pat, d))[:n]
    if not hits:
        print("   (not found)")
    for m in hits:
        print("   " + d[max(0, m.start() - before):m.end() + after].decode('utf-8', 'replace'))
    return hits

h = show('stream-json rate_limit_event message', rb'u\(\{type:T\("rate_limit_event"\),rate_limit_info:[\w$]+\(\)[^}]*\}\)\.describe\("[^"]*"\)', after=0)
if h:
    name = re.search(rb'rate_limit_info:([\w$]+)\(\)', h[0].group(0)).group(1)
    show('its rate_limit_info schema', rb'\b' + re.escape(name) + rb'=p\(\(\)=>u\(\{status:', after=700)
show('assistant message: the error field', rb'u\(\{type:T\("assistant"\),message:\w+\(\),parent_tool_use_id:o\(\)\.nullable\(\),error:\w+\(\)\.optional\(\)', after=0)
show('assistant error values', rb'\w+=p\(\(\)=>B\(\["authentication_failed","oauth_org_not_allowed"[^\]]*\]\)\)', after=0)
show('result error subtypes', rb'u\(\{type:T\("result"\),subtype:B\(\["error_during_execution"[^\]]*\]\)', after=0)
show('success result: is_error and api_error_status (a turn that ends on an API error)', rb'is_error:O\(\),api_error_status:k\(\)\.int\(\)\.nullable\(\)\.optional\(\)', after=0)
show('how a 429 with quota info becomes the limit message', rb'return [\w$]+\(\{content:[\w$]+\+[\w$]+\([\w$]+,[\w$]+,[\w$]+\),error:"rate_limit"', before=120, after=220)
show('the "resets" suffix of the limit message (resetsAt formatted as local time)', rb'w=_\?` \\xB7 resets \$\{_\}`', before=160, after=0)
EOF

echo; echo "## live rate_limit_event from out/01-multiturn (no limit reached; utilization not printed)"
if ls "$S1"/out/01-multiturn/*.jsonl >/dev/null 2>&1; then
  for f in "$S1"/out/01-multiturn/*.jsonl; do
    events "$f" 'select(.type=="rate_limit_event") | {type, info_keys: (.rate_limit_info|keys), status: .rate_limit_info.status, rateLimitType: .rate_limit_info.rateLimitType, resetsAt: .rate_limit_info.resetsAt, resetsAt_utc: (.rate_limit_info.resetsAt|todate), has_uuid: (.uuid != null), has_session_id: (.session_id != null)}'
  done | sort | uniq -c
else
  echo "   (none: run 01-multiturn.sh first)"
fi

echo; echo "## transcripts: shape of assistant messages that reported a limit (masked)"
python3 -I - "$HOME/.claude/projects" <<'EOF'
import json, os, re, sys, collections

def shape(v):
    if isinstance(v, dict): return {k: shape(x) for k, x in sorted(v.items())}
    if isinstance(v, list): return [shape(x) for x in v[:1]]
    if isinstance(v, bool): return 'bool'
    if isinstance(v, (int, float)): return 'number'
    if v is None: return 'null'
    return 'str'

def mask(t):
    t = re.sub(r'\([^)]*\)', '(<tz>)', t)
    t = re.sub(r'resets [^(]*', 'resets <time> ', t)
    t = re.sub(r'your [\w -]+? limit', 'your <window> limit', t)
    return t[:80]

seen = collections.Counter()
files = 0
for root, _, names in os.walk(sys.argv[1]):
    for n in names:
        if not n.endswith('.jsonl'): continue
        hit = False
        with open(os.path.join(root, n), errors='replace') as fh:
            for ln in fh:
                if '"error":"rate_limit"' not in ln: continue
                try: o = json.loads(ln)
                except ValueError: continue
                if o.get('type') != 'assistant' or o.get('error') != 'rate_limit': continue
                hit = True
                msg = o.get('message') or {}
                texts = [mask(c.get('text', '')) for c in msg.get('content') or [] if isinstance(c, dict) and c.get('type') == 'text']
                q = o.get('quotaLimits') or {}
                row = {
                    'version': o.get('version'), 'type': o['type'], 'error': o['error'],
                    'isApiErrorMessage': o.get('isApiErrorMessage'), 'apiErrorStatus': o.get('apiErrorStatus'),
                    'apiError': o.get('apiError'), 'top_keys': sorted(o.keys()),
                    'quotaLimits': shape(q), 'quotaLimits.status': q.get('status'),
                    'quotaLimits.rateLimitType': q.get('rateLimitType'), 'text': texts,
                }
                seen[json.dumps(row, sort_keys=True)] += 1
        files += hit
print(f"   transcript files with a limit message: {files}")
for row, n in seen.most_common():
    print(f"   x{n} {row}")
EOF
