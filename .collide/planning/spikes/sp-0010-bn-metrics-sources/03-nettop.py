#!/usr/bin/env python3
"""S8 measurement 4: `nettop -P` per process: cost, parse cost, exactness, granularity, what it misses. No root needed."""
import os
import pty
import select
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "lib"))
from nettop import ARGS, parse, snapshot  # noqa: E402

NODE = subprocess.run(["/usr/bin/which", "node"], capture_output=True, text=True).stdout.strip() or "node"


def stream_cost(extra, secs=10, stdin=subprocess.PIPE):
    """CPU of a streaming nettop writing to a file, as a share of one core. nettop reads commands from stdin: with an open
    pipe (never written to) it idles, with /dev/null (end of file) it spins at over 100% of a core."""
    path = os.path.join(HERE, "work", "nettop-stream.csv")
    with open(path, "w") as f:
        t0 = time.time()
        p = subprocess.Popen(["nettop"] + ARGS + ["-L", "0"] + extra, stdin=stdin, stdout=f, stderr=subprocess.DEVNULL)
        time.sleep(secs)
        p.terminate()  # our own child
        _, _, ru = os.wait4(p.pid, 0)
        wall = time.time() - t0
    blocks = sum(1 for l in open(path) if l.startswith("time,"))
    return (ru.ru_utime + ru.ru_stime) / wall, blocks, os.path.getsize(path)


def delivery(use_pty, secs=8):
    """When do blocks reach a reader? Returns the arrival times of each header line, relative to the start."""
    t0 = time.time()
    if use_pty:
        master, slave = pty.openpty()
        p = subprocess.Popen(["nettop"] + ARGS + ["-L", "0", "-s", "1"], stdin=subprocess.PIPE, stdout=slave, stderr=subprocess.DEVNULL)
        os.close(slave)
        rd = lambda: os.read(master, 65536)
    else:
        p = subprocess.Popen(["nettop"] + ARGS + ["-L", "0", "-s", "1"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        rd = lambda: os.read(p.stdout.fileno(), 65536)
    arrivals = []
    fd = master if use_pty else p.stdout.fileno()
    while time.time() - t0 < secs:
        if select.select([fd], [], [], 0.2)[0]:
            data = rd()
            arrivals += [time.time() - t0] * data.count(b"time,")
    p.terminate()
    os.wait4(p.pid, 0)
    return arrivals


def main():
    walls, cpus, parses = [], [], []
    for _ in range(5):
        rows, wall, cpu, ptime, nbytes = snapshot()
        walls.append(wall); cpus.append(cpu); parses.append(ptime)
    print("one `nettop -P -L 1` run: %d processes listed, %d bytes of CSV; wall %.3f s, nettop CPU %.3f s per run; parse in Python %.2f ms [M]" % (
        len(rows), nbytes, sum(walls) / 5, sum(cpus) / 5, 1000 * sum(parses) / 5))
    for label, extra in (("-s 1", ["-s", "1"]), ("-s 5", ["-s", "5"])):
        runs = [stream_cost(extra, 8) for _ in range(3)]
        print("streaming `nettop -P -L 0 %s` to a file, stdin an open pipe, 3 runs of 8 s: nettop CPU %s of one core; %d blocks, %d bytes in the last run [M]" % (
            label, ", ".join("%.1f%%" % (100 * r[0]) for r in runs), runs[-1][1], runs[-1][2]))
    cpu, blocks, size = stream_cost(["-s", "1"], stdin=subprocess.DEVNULL)
    print("the same with stdin = /dev/null: nettop CPU %.0f%% of one core (it spins on the end of file) [M]" % (100 * cpu))
    a = delivery(False)
    b = delivery(True)
    print("delivery of 1 s blocks to a reader over a pipe: %d header lines seen in 8 s, first at %.1f s, arrivals %s [M]" % (
        len(a), a[0] if a else -1, sorted({round(x, 1) for x in a})[:6]))
    print("delivery over a pty: %d header lines, first at %.1f s, arrivals %s [M]" % (len(b), b[0] if b else -1, sorted({round(x, 1) for x in b})[:8]))

    port_file = os.path.join(HERE, "work", "nettop.port")
    if os.path.exists(port_file):
        os.unlink(port_file)
    srv = subprocess.Popen([NODE, os.path.join(HERE, "workload", "server.js"), port_file])
    try:
        for _ in range(100):
            if os.path.exists(port_file) and open(port_file).read().strip():
                break
            time.sleep(0.05)
        port = int(open(port_file).read())
        keep = ("import sys,http.client,time;c=http.client.HTTPConnection('127.0.0.1',int(sys.argv[1]));"
                "c.request('GET','/blob?n=%s'%sys.argv[2]);d=c.getresponse().read();print(len(d),flush=True);time.sleep(float(sys.argv[3]))")
        c = subprocess.Popen([sys.executable, "-c", keep, str(port), str(64 << 20), "6"], stdout=subprocess.PIPE, text=True)
        c.stdout.readline()
        rows, *_ = snapshot()
        print("\n64 MiB (67108864 bytes) over loopback, connection still open: client %s, server %s [M]" % (rows.get(c.pid), rows.get(srv.pid)))
        c.wait()
        rows, *_ = snapshot()
        print("after the client exited: client listed: %s; server %s [M]  (counters are the sum over open sockets: a closed connection's bytes vanish)" % (
            c.pid in rows, rows.get(srv.pid)))

        # short connections: 20 clients, 1 MiB each, connection open about 0.1 s, a 1 s nettop stream watching through a pty
        master, slave = pty.openpty()
        s = subprocess.Popen(["nettop"] + ARGS + ["-L", "0", "-s", "1"], stdin=subprocess.PIPE, stdout=slave, stderr=subprocess.DEVNULL)
        os.close(slave)
        pids = set()
        seen = {}
        buf = b""

        def drain(t):
            nonlocal buf
            end = time.time() + t
            while time.time() < end:
                if select.select([master], [], [], 0.05)[0]:
                    buf += os.read(master, 65536)
                    *lines, buf = buf.split(b"\n")
                    block = parse("time,,bytes_in,bytes_out,\n" + "\n".join(l.decode("utf8", "replace").rstrip("\r") for l in lines))
                    for pid, v in block.items():
                        if pid in pids:
                            seen[pid] = max(seen.get(pid, 0), v[1])

        drain(1.5)
        for _ in range(20):
            q = subprocess.Popen([sys.executable, "-c", keep, str(port), str(1 << 20), "0.1"], stdout=subprocess.PIPE, text=True)
            pids.add(q.pid)
            q.wait()
            drain(0.3)
        drain(2.5)
        s.terminate()
        os.wait4(s.pid, 0)
        got = sum(seen.values())
        print("\n20 short clients (1 MiB each, connection open ~0.1 s) watched by a 1 s nettop stream: %d of 20 ever listed; %d of %d bytes seen (%.0f%%) [M]" % (
            len(seen), got, 20 << 20, 100.0 * got / (20 << 20)))
    finally:
        srv.terminate()  # our own child
        srv.wait()
    rows, *_ = snapshot()
    print("\nthe kernel's process name is truncated to 16 characters (longest listed: %s); a row is 'name.pid'; with -P there are no per-socket rows [M]" % (
        sorted({v[0] for v in rows.values()}, key=len)[-1]))


if __name__ == "__main__":
    main()
