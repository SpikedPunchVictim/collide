#!/usr/bin/env python3
"""The non-process measurements of measure-attach.sh, read-only: network per process (nettop stream), GPU (ioreg), Docker
stats for containers the owner selects with a filter, and sleep detection. Starts nothing but its own nettop child.

  attach_extras.py --root PID --out DIR [--docker-filter label=KEY=VALUE]
Stops when the root pid is gone or on SIGINT/SIGTERM, then writes DIR/extras.json.
"""
import argparse
import json
import os
import signal
import subprocess
import sys
import threading
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import detached  # noqa: E402
import gpu  # noqa: E402
import libproc  # noqa: E402
import nettop  # noqa: E402
import sleepclock  # noqa: E402

STOP = threading.Event()


def tree_pids(root):
    parents = {}
    for pid in libproc.list_pids():
        bi = libproc.bsd_info(pid)
        if bi:
            parents[pid] = bi[0]
    return libproc.descendants(root, parents)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", type=int, required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--docker-filter", default=None)
    a = ap.parse_args()
    signal.signal(signal.SIGINT, lambda *x: STOP.set())
    signal.signal(signal.SIGTERM, lambda *x: STOP.set())

    net = {}  # pid -> [name, first (in, out), last (in, out)]
    gaps = []

    def net_reader():
        p = subprocess.Popen(["nettop", "-P", "-L", "0", "-s", "1", "-x", "-J", nettop.COLS], stdin=subprocess.PIPE,
                             stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
        try:
            tree = tree_pids(a.root)
            for line in p.stdout:
                if STOP.is_set():
                    break
                if line.startswith(",bytes_in"):
                    tree = tree_pids(a.root)
                    continue
                f = line.split(",")
                if len(f) >= 3 and "." in f[0]:
                    name, pid = f[0].rsplit(".", 1)
                    if pid.isdigit() and int(pid) in tree:
                        v = (int(f[1] or 0), int(f[2] or 0))
                        e = net.setdefault(int(pid), [name, v, v])
                        e[2] = v
        finally:
            p.terminate()
            os.wait4(p.pid, 0)

    threading.Thread(target=net_reader, daemon=True).start()
    gpu_series, docker_series = [], []
    prev = sleepclock.read()
    sock = detached.docker_socket() if a.docker_filter else None
    last_slow = 0.0
    while not STOP.is_set():
        time.sleep(1)
        cur = sleepclock.read()
        gap = (cur[1] - prev[1]) - (cur[0] - prev[0])
        if gap > sleepclock.THRESHOLD_NS:
            gaps.append({"at_wall_ns": cur[2], "slept_s": gap / 1e9})
        prev = cur
        if libproc.bsd_info(a.root) is None and libproc.rusage(a.root) is None:
            break
        if time.time() - last_slow >= 2.0:
            last_slow = time.time()
            tree = tree_pids(a.root)
            clients, *_ = gpu.gpu_clients()
            dev, *_ = gpu.device_stats()
            gpu_series.append({"t": time.time(), "device": dev.get("Device Utilization %"),
                               "tree": {p: v["gpu_ns"] for p, v in clients.items() if p in tree}})
            if sock:
                ids = subprocess.run(["docker", "ps", "-q", "--filter", a.docker_filter], capture_output=True, text=True).stdout.split()
                for cid in ids:
                    try:
                        st, w, _ = detached.docker_stats(sock, cid, True)
                        docker_series.append({"t": time.time(), "id": cid[:12], **detached.flatten(st)})
                    except Exception as e:  # container went away between ps and stats
                        docker_series.append({"t": time.time(), "id": cid[:12], "error": str(e)[:80]})
    json.dump({"sleep_gaps": gaps, "net": {str(p): {"name": v[0], "bytes_in_delta": v[2][0] - v[1][0], "bytes_out_delta": v[2][1] - v[1][1],
                                                  "bytes_in_last": v[2][0], "bytes_out_last": v[2][1]} for p, v in net.items()},
               "gpu": gpu_series, "docker": docker_series}, open(os.path.join(a.out, "extras.json"), "w"))


if __name__ == "__main__":
    main()
