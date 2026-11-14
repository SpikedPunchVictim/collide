#!/usr/bin/env python3
"""S8 measurements 1 and 2: sampler overhead and missed short-lived processes, at several intervals.

Per repeat, one synthetic workload run with one Rust sampler per interval attached to the workload's root pid at the same
time, so every interval sees exactly the same processes. Each sampler's CPU is read twice: from its own getrusage (SUMMARY)
and from wait4 on the sampler process (independent). Misses are measured against the workload's wait4 truth file.
  01-overhead-miss.py [--intervals 1000,500,250,100,50] [--repeats 3] [--steps 7] [--docker]
Samplers running together add a little load to each other; the overhead figures are per sampler and are an upper bound.
"""
import argparse
import collections
import json
import os
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import analyze_miss  # noqa: E402

SAMPLER = os.path.join(HERE, "sampler", "target", "release", "s8-sampler")


def one(intervals, steps, docker, tag):
    out = os.path.join(HERE, "out")
    os.makedirs(out, exist_ok=True)
    truth = os.path.join(out, "truth-%s.jsonl" % tag)
    cmd = [sys.executable, os.path.join(HERE, "workload", "workload.py"), "--steps", str(steps), "--seed", "7", "--truth", truth,
           "--outside-fetch", ""]
    if docker:
        cmd.append("--docker")
    load = os.getloadavg()[0]
    wl = subprocess.Popen(cmd, stdout=subprocess.PIPE, text=True)
    time.sleep(0.05)
    samplers = {}
    for iv in intervals:
        samplers[iv] = subprocess.Popen([SAMPLER, "--root", str(wl.pid), "--interval-ms", str(iv), "--out", os.path.join(out, "samples-%s-i%d.jsonl" % (tag, iv))],
                                        stdout=subprocess.PIPE, text=True)
    wl.stdout.read()
    wl.wait()
    res = {"load_at_start": load, "intervals": {}}
    for iv, sm in samplers.items():
        _, status, ru = os.wait4(sm.pid, 0)  # each sampler exits when its root is gone
        summary = json.loads(sm.stdout.read().split("SUMMARY", 1)[1])
        stats, tot, ticks = analyze_miss.analyze(truth, os.path.join(out, "samples-%s-i%d.jsonl" % (tag, iv)))
        res["intervals"][iv] = {"summary": summary, "ext_cpu_s": ru.ru_utime + ru.ru_stime, "stats": stats, "total": tot}
    return res


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--intervals", default="1000,500,250,100,50")
    ap.add_argument("--repeats", type=int, default=3)
    ap.add_argument("--steps", type=int, default=7, help="workload steps per run (7 is one pass of every step kind)")
    ap.add_argument("--docker", action="store_true")
    a = ap.parse_args()
    intervals = [int(x) for x in a.intervals.split(",")]
    runs = []
    for r in range(a.repeats):
        res = one(intervals, a.steps, a.docker, "r%d" % r)
        runs.append(res)
        s0 = res["intervals"][intervals[0]]["summary"]
        print("\n## repeat %d: workload ran %.0f s, machine load average at start %.1f" % (r + 1, s0["wall_s"], res["load_at_start"]))
        for iv in intervals:
            x = res["intervals"][iv]
            s = x["summary"]
            print("   %4d ms: %5d ticks %6d rows | sampler CPU %.3f%% of one core (own getrusage), %.3f%% (wait4) | tick work p50 %d us p99 %d us max %d us | seen %.0f%% of %d truth processes" % (
                iv, s["ticks"], s["rows"], s["cpu_pct_of_core"], 100 * x["ext_cpu_s"] / s["wall_s"], s["work_us_p50"], s["work_us_p99"], s["work_us_max"],
                100.0 * x["total"]["seen"] / x["total"]["n"], x["total"]["n"]))
    print("\n## pooled over %d repeats: share of the workload's processes missed, by lifetime (lifetime = spawn to wait4 return)" % a.repeats)
    names = [n for _, n in analyze_miss.BUCKETS]
    for iv in intervals:
        pooled = collections.OrderedDict((n, {"n": 0, "seen": 0, "cpu": 0.0, "cap": 0.0}) for n in names)
        for res in runs:
            for n, b in res["intervals"][iv]["stats"].items():
                for k in b:
                    pooled[n][k] += b[k]
        tot = {k: sum(b[k] for b in pooled.values()) for k in ("n", "seen", "cpu", "cap")}
        ovh = [100 * x["summary"]["cpu_s"] / x["summary"]["wall_s"] for x in (res["intervals"][iv] for res in runs)]
        print("\n== %d ms  (sampler CPU %.3f%% of one core, mean of %d runs; range %.3f-%.3f%%)" % (iv, sum(ovh) / len(ovh), len(ovh), min(ovh), max(ovh)))
        print(analyze_miss.fmt(pooled, tot))
    json.dump(runs, open(os.path.join(HERE, "out", "overhead-miss.json"), "w"), indent=1, default=str)


if __name__ == "__main__":
    main()
