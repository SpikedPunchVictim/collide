#!/usr/bin/env python3
"""Summarise a measure-attach.sh run directory: process shape, the 1 s sampler against the 50 ms reference, extras.

  attach_report.py DIR
There is no wait4 truth on a real run, so the 50 ms sampler is the reference: it misses far fewer short processes
(S8 record, "Sampler overhead and missed processes"), so the 1 s sampler's miss share against it is a lower bound.
"""
import collections
import json
import os
import sys


def load(path):
    procs = {}  # (pid, start_ticks) -> dict
    ticks = rows = 0
    for line in open(path):
        if not line.startswith("{"):
            continue
        s = json.loads(line)
        ticks += 1
        for p in s["p"]:
            rows += 1
            k = (p[0], p[2])
            e = procs.setdefault(k, {"comm": p[3], "ppid": p[1], "first": s["t"], "last": s["t"], "cpu": 0, "rd": 0, "wr": 0, "rss": 0, "n": 0})
            e["last"] = s["t"]
            e["cpu"] = p[4] + p[5]
            e["rd"], e["wr"] = p[8], p[9]
            e["rss"] = max(e["rss"], p[6])
            e["n"] += 1
    return procs, ticks, rows


def summary_of(path):
    try:
        for line in open(path):
            if line.startswith("SUMMARY"):
                return json.loads(line.split("SUMMARY", 1)[1])
    except OSError:
        pass
    return None


def main():
    d = sys.argv[1]
    fine, fticks, _ = load(os.path.join(d, "samples-50.jsonl"))
    coarse, cticks, crows = load(os.path.join(d, "samples-1000.jsonl"))
    print("run directory:", d)
    for iv in (1000, 50):
        s = summary_of(os.path.join(d, "sampler-%d.summary" % iv))
        if s:
            print("sampler %4d ms: %d ticks, %d rows, CPU %.3f s over %.0f s = %.3f%% of one core; tick work p50 %d us p99 %d us" % (
                iv, s["ticks"], s["rows"], s["cpu_s"], s["wall_s"], s["cpu_pct_of_core"], s["work_us_p50"], s["work_us_p99"]))
    print("\nprocesses seen: %d by the 50 ms reference, %d by the 1 s sampler" % (len(fine), len(coarse)))
    missed = [k for k in fine if k not in coarse]
    print("1 s sampler missed %d of %d (%.1f%%) of the reference's processes [lower bound on the true miss share]" % (
        len(missed), len(fine), 100.0 * len(missed) / max(1, len(fine))))
    life = lambda e: (e["last"] - e["first"]) / 1e9
    buckets = [(0, 0.1, "<0.1s"), (0.1, 0.5, "0.1-0.5s"), (0.5, 1, "0.5-1s"), (1, 5, "1-5s"), (5, 30, "5-30s"), (30, 1e9, ">=30s")]
    print("\nobserved lifetimes in the 50 ms reference (a floor: a process is seen only from its first to its last sample):")
    print("%-9s %7s %7s %8s" % ("lifetime", "procs", "missed", "cpu_s"))
    for lo, hi, name in buckets:
        sel = [(k, e) for k, e in fine.items() if lo <= life(e) < hi]
        print("%-9s %7d %7d %8.1f" % (name, len(sel), sum(1 for k, _ in sel if k not in coarse), sum(e["cpu"] for _, e in sel) / 1e9))
    names = collections.Counter(e["comm"] for e in fine.values())
    print("\nmost common process names in the tree: %s" % ", ".join("%s x%d" % kv for kv in names.most_common(8)))
    ex_path = os.path.join(d, "extras.json")
    if os.path.exists(ex_path):
        ex = json.load(open(ex_path))
        print("\nsleep gaps seen: %s" % (ex["sleep_gaps"] or "none"))
        print("network per process in the tree (nettop, delta over the run, top 5 by bytes in): %s" % sorted(
            ((v["name"], v["bytes_in_delta"], v["bytes_out_delta"]) for v in ex["net"].values()), key=lambda x: -x[1])[:5])
        devs = [g["device"] for g in ex["gpu"] if g["device"] is not None]
        print("GPU: device utilization max %s%% over %d reads; tree processes with GPU time: %s" % (
            max(devs) if devs else "n/a", len(devs), sorted({p for g in ex["gpu"] for p in g["tree"]}) or "none"))
        if ex["docker"]:
            ids = sorted({r["id"] for r in ex["docker"]})
            print("Docker: %d containers matched the filter: %s" % (len(ids), ids))
        else:
            print("Docker: no containers matched, or no filter given")


if __name__ == "__main__":
    main()
