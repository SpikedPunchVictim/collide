#!/usr/bin/env python3
"""Compare a sampler's JSONL output with the workload's wait4 truth file.

  analyze_miss.py truth.jsonl samples.jsonl
Prints, per lifetime bucket: processes, share seen at least once, and the share of their CPU time the samples captured.
"""
import collections
import json
import sys

BUCKETS = [(0.01, "<10ms"), (0.1, "10-100ms"), (0.25, "0.1-0.25s"), (0.5, "0.25-0.5s"), (1.0, "0.5-1s"), (5.0, "1-5s"),
           (1e9, ">=5s")]


def bucket(life):
    for hi, name in BUCKETS:
        if life < hi:
            return name


def load(truth_path, samples_path):
    truth = [json.loads(l) for l in open(truth_path) if l.strip()]
    kids_cpu = collections.defaultdict(float)
    for r in truth:
        kids_cpu[r["ppid"]] += r["utime"] + r["stime"]
    for r in truth:
        r["life"] = (r["t_exit"] - r["t_spawn"]) / 1e9
        r["cpu_excl"] = max(0.0, r["utime"] + r["stime"] - kids_cpu.get(r["pid"], 0.0))
    by_pid = collections.defaultdict(list)  # pid -> [(t_ns, cpu_s)]
    ticks = 0
    for line in open(samples_path):
        if not line.startswith("{"):
            continue
        s = json.loads(line)
        ticks += 1
        for p in s["p"]:
            by_pid[p[0]].append((s["t"], (p[4] + p[5]) / 1e9))
    return truth, by_pid, ticks


def analyze(truth_path, samples_path):
    truth, by_pid, ticks = load(truth_path, samples_path)
    stats = collections.OrderedDict((name, {"n": 0, "seen": 0, "cpu": 0.0, "cap": 0.0}) for _, name in BUCKETS)
    for r in truth:
        b = stats[bucket(r["life"])]
        b["n"] += 1
        b["cpu"] += r["cpu_excl"]
        hits = [x for x in by_pid.get(r["pid"], []) if r["t_spawn"] - 50_000_000 <= x[0] <= r["t_exit"] + 50_000_000]
        if hits:
            b["seen"] += 1
            b["cap"] += min(max(h[1] for h in hits), r["cpu_excl"] + 0.02)  # a sample cannot exceed the exit total
    tot = {"n": 0, "seen": 0, "cpu": 0.0, "cap": 0.0}
    for b in stats.values():
        for k in tot:
            tot[k] += b[k]
    return stats, tot, ticks


def fmt(stats, tot):
    rows = ["%-10s %6s %8s %9s %9s %9s" % ("lifetime", "procs", "seen", "missed", "cpu_s", "cpu_capt")]
    for name, b in list(stats.items()) + [("all", tot)]:
        if b["n"] == 0:
            continue
        rows.append("%-10s %6d %7.1f%% %8.1f%% %9.2f %8.1f%%" % (
            name, b["n"], 100.0 * b["seen"] / b["n"], 100.0 * (b["n"] - b["seen"]) / b["n"], b["cpu"],
            100.0 * b["cap"] / b["cpu"] if b["cpu"] > 0 else float("nan")))
    return "\n".join(rows)


if __name__ == "__main__":
    st, tot, ticks = analyze(sys.argv[1], sys.argv[2])
    print("ticks", ticks)
    print(fmt(st, tot))
