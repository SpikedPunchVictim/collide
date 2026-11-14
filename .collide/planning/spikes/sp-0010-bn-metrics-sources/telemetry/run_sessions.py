#!/usr/bin/env python3
"""S8 telemetry runs: a few small real `claude -p` sessions that call tools, with OpenTelemetry exported to a local OTLP
receiver and the stream-json output timestamped on arrival. Writes out/telemetry/<name>.{stream,otlp}.jsonl.

  run_sessions.py [kind:count ...]   kinds: serial parallel subagent parallel-agents (default: serial:3 parallel:2 subagent:2 parallel-agents:2)
Cost control: model haiku, --max-budget-usd 0.25 per run, no session persistence, no settings sources, no MCP.
The environment of the calling Claude Code session is stripped of CLAUDE*/OTEL* variables first (S1's claude-clean idea).
"""
import json
import os
import subprocess
import sys
import threading
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
from otlp_receiver import Receiver  # noqa: E402

OUT = os.path.join(ROOT, "out", "telemetry")
CWD = os.path.join(ROOT, "work", "tel")

# A command that prints its own start and end (ns, wall clock): the ground truth for the command inside the tool call.
# perl starts in milliseconds; /usr/bin/python3 is a shim that took 0.3 to 3.5 s to start on this busy machine.
TIMED_PL = 'use Time::HiRes qw(time sleep);my $a=time;sleep($ARGV[0]);printf("T %d %d\\n",$a*1e9,time*1e9);\n'


def timed(sec):
    return "perl timed.pl %s" % sec


PROMPTS = {
    "serial": ("Use the Bash tool three times, one call at a time, running exactly these commands in this order and nothing else: "
               "(1) %s (2) %s (3) %s . Then use the Read tool on notes.txt. Then reply with the single word DONE." % (timed(0.5), timed(2), timed(1))),
    "parallel": ("In ONE single message, make two Bash tool calls at the same time (parallel tool calls) running exactly: "
                 "(A) %s and (B) %s . After both finish reply with the single word DONE." % (timed(2), timed(3))),
    "parallel-agents": ("In ONE single message, make two Agent tool calls at the same time (parallel calls, subagent_type general-purpose), each telling its "
                        "subagent to run exactly this one Bash command and report its output: %s . After both return, reply with the single word DONE." % timed(2)),
    "subagent": ("Use the Agent tool (subagent_type general-purpose) once, telling it to run exactly this one Bash command and report its output: "
                 "%s . After the subagent returns, run exactly this Bash command yourself: %s . Then reply with the single word DONE." % (timed(2), timed(1))),
}


def clean_env(port):
    env = {k: v for k, v in os.environ.items() if not k.startswith(("CLAUDE", "OTEL_", "ANTHROPIC_"))}
    env.update({
        "CLAUDE_CODE_ENABLE_TELEMETRY": "1",
        "OTEL_METRICS_EXPORTER": "otlp", "OTEL_LOGS_EXPORTER": "otlp", "OTEL_TRACES_EXPORTER": "otlp",
        "CLAUDE_CODE_ENHANCED_TELEMETRY_BETA": "1",
        "OTEL_EXPORTER_OTLP_PROTOCOL": "http/json",
        "OTEL_EXPORTER_OTLP_ENDPOINT": "http://127.0.0.1:%d" % port,
        "OTEL_LOGS_EXPORT_INTERVAL": "1000", "OTEL_METRIC_EXPORT_INTERVAL": "2000", "OTEL_TRACES_EXPORT_INTERVAL": "1000",
        "OTEL_RESOURCE_ATTRIBUTES": "collide.run=s8",
    })
    return env


def run(name, kind):
    os.makedirs(OUT, exist_ok=True)
    os.makedirs(CWD, exist_ok=True)
    open(os.path.join(CWD, "notes.txt"), "w").write("S8 scratch file: one line to read.\n")
    open(os.path.join(CWD, "timed.pl"), "w").write(TIMED_PL)
    rcv = Receiver().start()
    claude = os.path.expanduser("~/.local/bin/claude")
    argv = [claude, "-p", PROMPTS[kind], "--output-format", "stream-json", "--verbose", "--include-partial-messages",
            "--model", "haiku", "--max-budget-usd", "0.25", "--no-session-persistence", "--setting-sources", "",
            "--settings", '{"autoMemoryEnabled":false}', "--allowedTools", "Bash", "Read", "Agent", "Task"]
    stream_path = os.path.join(OUT, name + ".stream.jsonl")
    t0 = time.time_ns()
    p = subprocess.Popen(argv, cwd=CWD, env=clean_env(rcv.port), stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    err = []
    threading.Thread(target=lambda: err.append(p.stderr.read()), daemon=True).start()
    with open(stream_path, "w") as f:
        for raw in p.stdout:
            f.write(json.dumps({"recv_ns": time.time_ns(), "line": raw.decode("utf8", "replace").rstrip("\n")}) + "\n")
    rc = p.wait()
    t_exit = time.time_ns()
    time.sleep(3)  # let the exporters flush (intervals are 1 s)
    rows = rcv.snapshot()
    rcv.stop()
    with open(os.path.join(OUT, name + ".otlp.jsonl"), "w") as f:
        for r in rows:
            f.write(json.dumps(r) + "\n")
    cost = None
    for line in open(stream_path):
        try:
            d = json.loads(json.loads(line)["line"])
        except Exception:
            continue
        if d.get("type") == "result":
            cost = d.get("total_cost_usd")
    print("%s: exit %s, %.1f s, %d OTLP posts (%s), cost $%s, stderr %r" % (
        name, rc, (t_exit - t0) / 1e9, len(rows), sorted({r["path"] for r in rows}), cost, (err[0][:120] if err else b"")))
    return cost


if __name__ == "__main__":
    plan = sys.argv[1:] or ["serial:3", "parallel:2", "subagent:2", "parallel-agents:2"]  # kind:count
    total = 0.0
    names = []
    for item in plan:
        kind, _, n = item.partition(":")
        for i in range(int(n or 1)):
            name = "%s-%d" % (kind, i + 1)
            names.append(name)
            total += run(name, kind) or 0
    print("total cost $%.4f for %d runs" % (total, len(names)))
    json.dump({"total_cost_usd": total, "runs": names}, open(os.path.join(OUT, "spend.json"), "w"))
