"""Fake workspace cleanup hook: removes what tree.py recorded in lane.json.

It reaches what the process scans cannot: containers (in Docker Desktop's VM)
and host processes started with a cleaned environment. It also removes any
container still carrying this run's label.
Usage: cleanup.py --state-dir D --run-id R
"""
import argparse
import json
import os
import signal
import subprocess
import time


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--state-dir", required=True)
    ap.add_argument("--run-id", required=True)
    a = ap.parse_args()
    lane = {}
    try:
        with open(os.path.join(a.state_dir, "lane.json")) as f:
            lane = json.load(f)
    except FileNotFoundError:
        pass
    report = {"killed": [], "containers_removed": []}
    for pid in lane.get("pids", []):
        try:
            os.kill(pid, signal.SIGTERM)
            report["killed"].append(pid)
        except ProcessLookupError:
            pass
    labelled = subprocess.run(["docker", "ps", "-a", "-q", "--filter", "label=collide.spike=1",
                               "--filter", "label=collide.run=" + a.run_id],
                              capture_output=True, text=True).stdout.split()
    ids = sorted(set(lane.get("containers", [])) | set(labelled))
    if ids:
        t0 = time.time()
        out = subprocess.run(["docker", "rm", "-f"] + ids, capture_output=True, text=True)
        report["containers_removed"] = out.stdout.split()
        report["docker_rm_s"] = round(time.time() - t0, 2)
    print(json.dumps(report), flush=True)


if __name__ == "__main__":
    main()
