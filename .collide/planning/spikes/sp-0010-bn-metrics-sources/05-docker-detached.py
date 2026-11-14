#!/usr/bin/env python3
"""S8 measurement 6: Docker API stats for a container this script starts, detached host processes, and the
`metrics.processes` hook's output shape. Starts one container (unique name, removed at the end) and one detached server."""
import json
import os
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "lib"))
sys.path.insert(0, os.path.join(HERE, "workload"))
import detached  # noqa: E402
import libproc  # noqa: E402
import workload  # noqa: E402


def main():
    sock = detached.docker_socket()
    print("docker endpoint: unix socket %s" % os.path.basename(sock))
    before = {p: libproc.bsd_info(p) for p in libproc.list_pids()}
    wd = os.path.join(HERE, "work")
    os.makedirs(wd, exist_ok=True)
    lane = workload.lane_up(wd, True, 0)
    try:
        json.dump(lane, open(os.path.join(HERE, "out", "lane.json"), "w"))
        if not lane.get("container"):
            print("container did not start: %s [U]" % lane.get("dockerError"))
            return
        time.sleep(3)
        # the hook
        t0 = time.time()
        hook = subprocess.run([sys.executable, os.path.join(HERE, "hooks", "metrics-processes.py"), os.path.join(HERE, "out", "lane.json")],
                              capture_output=True, text=True)
        print("hook output (%.0f ms): %s" % (1000 * (time.time() - t0), hook.stdout.strip()))
        spec = json.loads(hook.stdout)
        cid = spec["containers"][0]["id"]

        # Docker API stats: latency and fields
        _, w_one, _ = detached.docker_stats(sock, cid, True)
        st2, w_two, size = detached.docker_stats(sock, cid, False)
        st1, w_one, size1 = detached.docker_stats(sock, cid, True)
        print("\nDocker API stats: one-shot %.0f ms (%d bytes), stream=false %.0f ms (%d bytes) [M]" % (1000 * w_one, size1, 1000 * w_two, size))
        print("  top-level keys: %s" % sorted(st2.keys()))
        print("  flattened (one-shot): %s" % json.dumps(detached.flatten(st1)))
        print("  blkio_stats keys: %s; networks: %s" % (sorted((st2.get("blkio_stats") or {}).keys()), sorted((st2.get("networks") or {}).keys())))
        # the same through the CLI, for cost comparison
        t0 = time.time()
        p = subprocess.Popen(["docker", "stats", "--no-stream", "--format", "{{json .}}", cid], stdout=subprocess.PIPE, text=True)
        out = p.stdout.read()
        _, _, ru = os.wait4(p.pid, 0)
        print("  `docker stats --no-stream`: %.0f ms wall, %.0f ms CPU; output: %s" % (1000 * (time.time() - t0), 1000 * (ru.ru_utime + ru.ru_stime), out.strip()[:160]))

        # rates over 10 s of polling
        samples = []
        t_start = time.time()
        cpu_cost = time.process_time()
        for _ in range(10):
            st, w, _ = detached.docker_stats(sock, cid, True)
            samples.append((time.time(), detached.flatten(st), w))
            time.sleep(1)
        cpu_cost = time.process_time() - cpu_cost
        a, b = samples[0], samples[-1]
        dt = b[0] - a[0]
        print("\nover %.1f s of one-shot polling (client CPU %.1f ms for 10 calls) [M]:" % (dt, 1000 * cpu_cost))
        print("  container CPU %.1f%% of one core (the workload burns ~150 ms of each second, so ~15%% plus node idle)" % (100 * (b[1]["cpu_total_ns"] - a[1]["cpu_total_ns"]) / 1e9 / dt))
        print("  memory %.1f MiB; net rx +%s tx +%s bytes; block read +%s write +%s; pids %s" % (
            b[1]["mem_usage"] / 2**20, (b[1]["net_rx"] - a[1]["net_rx"]) if b[1]["net_rx"] is not None else None,
            (b[1]["net_tx"] - a[1]["net_tx"]) if b[1]["net_tx"] is not None else None, b[1]["blk_read"], b[1]["blk_write"], b[1]["pids"]))
        print("  stats `read` timestamps advance by %.2f s per call (the API's own sampling step)" % (
            (time.mktime(time.strptime(b[1]["read"][:19], "%Y-%m-%dT%H:%M:%S")) - time.mktime(time.strptime(a[1]["read"][:19], "%Y-%m-%dT%H:%M:%S"))) / (len(samples) - 1)))

        # host view: the container's processes are not in the host process list
        after = {p: libproc.bsd_info(p) for p in libproc.list_pids()}
        new = {p: v for p, v in after.items() if p not in before and v}
        print("\nhost processes new since before the container started (name:count): %s" % dict(
            sorted(((n, sum(1 for v in new.values() if v[2] == n)) for n in {v[2] for v in new.values()}), key=lambda kv: -kv[1])[:8]))
        print("  none of them is the container's `node` unless it is this lane's host server (pid %s) [M: compare names above]" % lane["serverPid"])

        # detached host process
        sp = lane["serverPid"]
        bi = libproc.bsd_info(sp)
        print("\ndetached host server pid %d: ppid %d (1 = launchd, so a tree walk from the launcher misses it) [M]" % (sp, bi[0]))
        a = detached.pid_stats(sp)
        urls = "http://127.0.0.1:%d/work?ms=100" % lane["port"]
        import urllib.request
        for _ in range(5):
            urllib.request.urlopen(urls).read()
        b = detached.pid_stats(sp)
        print("  rusage by pid through the hook's list: cpu +%.0f ms after five 100 ms requests; footprint %.1f MiB [M]" % (
            (b["cpu_ns"] - a["cpu_ns"]) / 1e6, b["footprint"] / 2**20))
    finally:
        workload.lane_down(lane)
        time.sleep(1.5)
        left = subprocess.run(["docker", "ps", "-a", "--filter", "label=collide-s8=1", "--format", "{{.Names}}"], capture_output=True, text=True).stdout.strip()
        print("\ncleanup: containers with our label left: %r; server pid %s alive: %s" % (left, lane.get("serverPid"), libproc.bsd_info(lane["serverPid"]) is not None if lane.get("serverPid") else None))


if __name__ == "__main__":
    main()
