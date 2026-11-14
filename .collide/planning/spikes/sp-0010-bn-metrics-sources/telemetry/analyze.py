#!/usr/bin/env python3
"""Compare Claude Code's OpenTelemetry (logs and trace spans) with timings derived from the stream-json output, per tool call,
and work out the breakdown's timing choices (model-time gap threshold, nested calls, share denominator).

  analyze.py [out/telemetry]
Reads <name>.stream.jsonl (stream lines stamped on arrival) and <name>.otlp.jsonl (what the receiver got), written by run_sessions.py.
"""
import collections
import glob
import json
import os
import re
import statistics
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from otlp_receiver import log_records, spans  # noqa: E402

MS = 1e6


def ms(x):
    return x / MS


def load(base):
    stream = []
    for line in open(base + ".stream.jsonl"):
        r = json.loads(line)
        try:
            stream.append((r["recv_ns"], json.loads(r["line"])))
        except Exception:
            pass
    otlp = [json.loads(l) for l in open(base + ".otlp.jsonl")]
    return stream, otlp


def stream_calls(stream):
    """tool_use_id -> arrival times and truth parsed from the command's own output."""
    calls = {}
    resp = []  # one entry per model response: message id, start (message_start arrival), stop (message_stop arrival), parent
    cur = None
    prev_input = stream[0][0] if stream else None
    for t, d in stream:
        ty = d.get("type")
        if ty == "stream_event":
            e = d["event"]
            if e["type"] == "message_start":
                cur = {"start": t, "stop": None, "parent": d.get("parent_tool_use_id"), "prev_input": prev_input}
                resp.append(cur)
            elif e["type"] == "message_stop" and cur:
                cur["stop"] = t
            elif e["type"] == "content_block_start" and (e.get("content_block") or {}).get("type") == "tool_use":
                calls.setdefault(e["content_block"]["id"], {})["block_start"] = t
        elif ty == "assistant":
            for c in d["message"]["content"]:
                if c["type"] == "tool_use":
                    x = calls.setdefault(c["id"], {})
                    x.update({"name": c["name"], "use": t, "parent": d.get("parent_tool_use_id")})
        elif ty == "user":
            c = d["message"]["content"]
            if isinstance(c, list):
                for b in c:
                    if b.get("type") == "tool_result":
                        x = calls.setdefault(b["tool_use_id"], {})
                        x["result"] = t
                        m = re.search(r"T (\d{15,}) (\d{15,})", json.dumps(b.get("content")))
                        if m and x.get("name") == "Bash" and x.get("parent") is None:
                            x["cmd"] = (int(m.group(1)), int(m.group(2)))
            prev_input = t
    return calls, resp


def pcts(v):
    v = sorted(v)
    if not v:
        return "n=0"
    q = lambda p: v[min(len(v) - 1, int(p * len(v)))]
    return "n=%d min %.0f p50 %.0f p95 %.0f max %.0f ms" % (len(v), v[0], q(.5), q(.95), v[-1])


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(HERE), "out", "telemetry")
    names = sorted(os.path.basename(p)[:-len(".stream.jsonl")] for p in glob.glob(os.path.join(out, "*.stream.jsonl")))
    # offsets collected across runs, relative to the command's own clock (Bash calls with a timed command)
    end_off = collections.defaultdict(list)    # source -> ms after the command's last print
    start_off = collections.defaultdict(list)  # source -> ms after the command's first instruction
    dur_err = collections.defaultdict(list)    # source -> duration minus the tool.execution span's, ms
    dur_vs_cmd = collections.defaultdict(list)
    gaps_pre, gaps_post, resp_err, resp_err_nopartial, ttft, resp_start_late = [], [], [], [], [], []
    log_lag, log_vs_span = [], []
    serial_queue = []
    tables = {}
    for name in names:
        stream, otlp = load(os.path.join(out, name))
        calls, resp = stream_calls(stream)
        sp = [s for _, s in spans(otlp)]
        for s in sp:
            s["st"], s["en"] = int(s["startTimeUnixNano"]), int(s["endTimeUnixNano"])
        by = collections.defaultdict(dict)
        for s in sp:
            tid = s["attrs"].get("tool_use_id")
            if s["name"] == "claude_code.tool":
                by[tid]["tool"] = s
            elif s["name"] == "claude_code.tool.execution":
                by[tid]["exec"] = s
        logs = {}
        for recv, t, a in log_records(otlp):
            if a.get("event.name") == "tool_result":
                logs[a["tool_use_id"]] = (t, int(a["duration_ms"]), recv)
        print("\n== %s: %d spans, %d tool calls in the stream, %d tool_result log records" % (name, len(sp), len(calls), len(logs)))
        print("   %-8s %-6s %9s %9s %9s %9s %9s | %8s %8s %8s %8s" % ("tool", "id", "cmd_ms", "span_ms", "log_ms", "stream_ms", "strm_blk", "end:span", "end:log", "end:strm", "start:st"))
        for tid, c in calls.items():
            e = by.get(tid, {}).get("exec")
            if not e or "result" not in c:
                continue
            lg = logs.get(tid)
            cmd = c.get("cmd")
            sdur = c["result"] - c["use"]
            row = [c.get("name"), tid[-5:], ms(cmd[1] - cmd[0]) if cmd else None, ms(e["en"] - e["st"]), lg[1] if lg else None, ms(sdur),
                   ms(c["result"] - c["block_start"]) if "block_start" in c else None]
            if cmd:
                end_off["span exec end"].append(ms(e["en"] - cmd[1]))
                end_off["tool_result log time"].append(ms(lg[0] * 1 - cmd[1]) if lg else 0)
                end_off["stream tool_result arrival"].append(ms(c["result"] - cmd[1]))
                start_off["span exec start"].append(ms(e["st"] - cmd[0]))
                start_off["stream assistant tool_use arrival"].append(ms(c["use"] - cmd[0]))
                if "block_start" in c:
                    start_off["stream content_block_start arrival"].append(ms(c["block_start"] - cmd[0]))
                dur_vs_cmd["span exec duration"].append(ms(e["en"] - e["st"]) - ms(cmd[1] - cmd[0]))
                dur_vs_cmd["stream (use to result)"].append(ms(sdur) - ms(cmd[1] - cmd[0]))
            dur_err["stream (use to result)"].append(ms(sdur) - ms(e["en"] - e["st"]))
            if lg:
                dur_err["log duration_ms"].append(lg[1] - ms(e["en"] - e["st"]))
                log_vs_span.append(ms(lg[0] - e["en"]))
                log_lag.append(ms(lg[2] - lg[0]))
            print("   %-8s %-6s %9s %9.0f %9s %9.0f %9s | %8s %8s %8s %8s" % (
                row[0], row[1], "%.0f" % row[2] if row[2] is not None else "-", row[3], row[4], row[5], "%.0f" % row[6] if row[6] is not None else "-",
                "%.0f" % ms(e["en"] - cmd[1]) if cmd else "-", "%.0f" % ms(lg[0] - cmd[1]) if cmd and lg else "-",
                "%.0f" % ms(c["result"] - cmd[1]) if cmd else "-", "%.0f" % ms(c["use"] - e["st"])))
        # serialised calls: a stream start that precedes the execution start
        execs = sorted(((by[t]["exec"]["st"], by[t]["exec"]["en"], t) for t in by if "exec" in by[t]), key=lambda x: x[0])
        for st_, en_, t in execs:
            c = calls.get(t, {})
            if "use" in c and c.get("parent") is None:
                serial_queue.append((name, ms(st_ - c["use"])))
        # model time per request (spans) against the stream, and the gaps around them
        llm = sorted((s for s in sp if s["name"] == "claude_code.llm_request" and s["attrs"].get("llm_request.context") == "interaction"), key=lambda s: s["st"])
        tops = sorted((s for s in sp if s["name"] == "claude_code.tool" and s["attrs"].get("agent_id") is None), key=lambda s: s["st"])
        inter = [s for s in sp if s["name"] == "claude_code.interaction"]
        main_resp = [r for r in resp if r["parent"] is None and r["stop"]]
        prev_end = inter[0]["st"] if inter else None
        events = sorted([(s["st"], s["en"], "llm") for s in llm] + [(s["st"], s["en"], "tool") for s in tops])
        for a, b in zip(events, events[1:]):
            if a[2] == "llm" and b[2] == "tool":
                gaps_post.append(ms(b[0] - a[1]))
            elif a[2] == "tool" and b[2] == "llm":
                gaps_pre.append(ms(b[0] - a[1]))
        for s in llm:
            r = min(main_resp, key=lambda r: abs(r["stop"] - s["en"]), default=None)
            if r and abs(r["stop"] - s["en"]) < 1.5e9:
                resp_err.append(ms((r["stop"] - r["start"]) - (s["en"] - s["st"])))
                ttft.append(int(s["attrs"]["ttft_ms"]))
                resp_err_nopartial.append(ms((r["stop"] - r["prev_input"]) - (s["en"] - s["st"])))
                resp_start_late.append(ms(r["start"] - s["st"]) - int(s["attrs"]["ttft_ms"]))
        tables[name] = (sp, inter)

    print("\n\n######## Summary across %d runs" % len(names))
    print("\nEnd of a tool call, relative to the command's own last print (Bash calls with a timed command):")
    for k, v in end_off.items():
        print("  %-36s %s" % (k, pcts(v)))
    print("Start of a tool call, relative to the command's own first instruction (perl start-up is inside it, so every row has that offset):")
    for k, v in start_off.items():
        print("  %-36s %s" % (k, pcts(v)))
    print("Duration error against the command's own time:")
    for k, v in dur_vs_cmd.items():
        print("  %-36s %s" % (k, pcts(v)))
    print("Duration error against the tool.execution span (the telemetry reference):")
    for k, v in dur_err.items():
        print("  %-36s %s" % (k, pcts(v)))
    print("tool_result log record time minus span end: %s; delivery lag (receiver clock minus record time, 1 s interval): %s" % (pcts(log_vs_span), pcts(log_lag)))
    q = [x[1] for x in serial_queue]
    print("\nStream tool_use arrival minus the execution start, top-level calls (negative = the call waited for an earlier one): %s" % pcts(q))
    print("  calls that started 500 ms or more after their tool_use line: %d of %d" % (sum(1 for x in q if x < -500), len(q)))
    print("\nModel time per request: stream message_start..message_stop minus the llm_request span: %s" % pcts(resp_err))
    print("  stream previous-input..message_stop minus the span (the estimate without partial messages): %s" % pcts(resp_err_nopartial))
    print("  stream message_start arrival minus (span start + ttft_ms): %s  [message_start marks first token, so the span's ttft_ms is the part the stream can't see]" % pcts(resp_start_late))
    print("  ttft_ms in the spans: %s" % pcts(ttft))
    print("Gaps around the model, from spans (top level only): tool end to next request start: %s; request end to tool start: %s" % (pcts(gaps_pre), pcts(gaps_post)))

    # share denominator and nesting
    print("\n\n######## Breakdown tables from the spans: plain sums against an exclusive (fair-share) attribution of wall-clock time")
    for name in names:
        sp, inter = tables[name]
        if not inter:
            continue
        ivs = sorted((i["st"], i["en"]) for i in inter)
        wall = sum(b - a for a, b in ivs)
        t0, t1 = ivs[0][0], ivs[-1][1]
        leaf, agents = [], []
        plain = collections.defaultdict(lambda: [0, 0.0])
        for s in sp:
            n = s["name"]
            if n == "claude_code.llm_request":
                cat = "model"
                leaf.append((cat, s["st"], s["en"]))
                plain[cat][0] += 1
                plain[cat][1] += s["en"] - s["st"]
            elif n == "claude_code.tool.execution":
                tool = next((x for x in sp if x["name"] == "claude_code.tool" and x["spanId"] == s.get("parentSpanId")), None)
                tn = tool["attrs"]["tool_name"] if tool else "?"
                plain[tn][0] += 1
                plain[tn][1] += s["en"] - s["st"]
                (agents if tn == "Agent" else leaf).append((tn, s["st"], s["en"]))
            elif n == "claude_code.tool.blocked_on_user":
                leaf.append(("permission/hook wait", s["st"], s["en"]))
                plain["permission/hook wait"][0] += 1
                plain["permission/hook wait"][1] += s["en"] - s["st"]
        bounds = sorted({t0, t1} | {x for _, a, b in leaf + agents for x in (a, b) if t0 <= x <= t1})
        excl = collections.defaultdict(float)
        for a, b in zip(bounds, bounds[1:]):
            if not any(x <= a and y >= b for x, y in ivs):
                continue
            act = [c for c, s_, e_ in leaf if s_ <= a and e_ >= b]
            if act:
                for c in act:
                    excl[c] += (b - a) / len(act)
            elif any(s_ <= a and e_ >= b for _, s_, e_ in agents):
                excl["Agent (self time)"] += b - a
            else:
                excl["other (no span running)"] += b - a
        total_plain = sum(v[1] for k, v in plain.items() if k != "permission/hook wait")
        print("\n== %s: %d interaction(s), wall %.2f s; plain sum of rows %.2f s (%.0f%% of wall)" % (name, len(ivs), wall / 1e9, total_plain / 1e9, 100.0 * total_plain / wall)) if False else None
        print("\n== %s: %d interaction(s), wall %.2f s; plain sum of rows %.2f s (%.0f%% of wall)" % (name, len(ivs), wall / 1e9, total_plain / 1e9, 100.0 * total_plain / wall))
        print("   %-24s %5s %10s %10s %10s %10s" % ("row", "calls", "sum_s", "of wall", "excl_s", "excl share"))
        for k in sorted(set(plain) | set(excl), key=lambda k: -max(plain[k][1] if k in plain else 0, excl.get(k, 0))):
            p = plain.get(k, [0, 0.0])
            print("   %-24s %5d %10.2f %9.0f%% %10.2f %9.0f%%" % (k, p[0], p[1] / 1e9, 100.0 * p[1] / wall, excl.get(k, 0) / 1e9, 100.0 * excl.get(k, 0) / wall))
        print("   exclusive shares add up to %.1f%% of wall" % (100.0 * sum(excl.values()) / wall))
        kids = [s for s in sp if s["attrs"].get("agent_id")]
        if kids:
            ids = sorted({s["attrs"]["agent_id"] for s in kids})
            print("   nested: %d spans carry an agent_id (%d subagents); their parent spans sit under the Agent tool's execution span: %s" % (
                len(kids), len(ids), all(any(p["spanId"] == k.get("parentSpanId") for p in sp) for k in kids)))


if __name__ == "__main__":
    main()
