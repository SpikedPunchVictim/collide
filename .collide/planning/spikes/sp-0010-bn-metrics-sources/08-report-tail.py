#!/usr/bin/env python3
"""S8 measurement 8: report files (plan 0.5, COLLIDE_REPORT). Tail lag under a chatty writer, and what caps follow.

A writer process appends small JSON lines (about 130 bytes, one os.write each) at a fixed rate. A tailer in this process
reads them back by polling every 50 ms or 250 ms, or by waking on kqueue vnode events. Lag = tailer read time minus the
time stamp inside the line. The tailer parses every line with json.loads, so its CPU is a Python upper bound for Collide's.
"""
import json
import os
import select
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))

WRITER = r'''
import os, sys, time, json
path, rate, secs = sys.argv[1], int(sys.argv[2]), float(sys.argv[3])
fd = os.open(path, os.O_WRONLY | os.O_APPEND | os.O_CREAT)
pad = "x" * 60
seq = 0
start = time.perf_counter()
end = start + secs
step = 0.001
n_per = max(1, rate // 1000) if rate >= 1000 else 1
interval = step if rate >= 1000 else 1.0 / rate
nxt = start
while True:
    now = time.perf_counter()
    if now >= end:
        break
    if now < nxt:
        time.sleep(min(nxt - now, 0.001))
        continue
    for _ in range(n_per):
        seq += 1
        os.write(fd, ('{"seq":%d,"t":%d,"kind":"metric","name":"files.checked","v":%d,"note":"%s"}\n' % (seq, time.time_ns(), seq % 97, pad)).encode())
    nxt += interval
print(seq)
'''


def tail(path, mode, secs, proc):
    fd = os.open(path, os.O_RDONLY)
    kq = None
    if mode == "kqueue":
        kq = select.kqueue()
        ev = select.kevent(fd, filter=select.KQ_FILTER_VNODE, flags=select.KQ_EV_ADD | select.KQ_EV_CLEAR,
                           fflags=select.KQ_NOTE_WRITE | select.KQ_NOTE_EXTEND)
    interval = {"poll50": 0.05, "poll250": 0.25}.get(mode)
    buf = b""
    lags, seqs = [], []
    reads = 0
    cpu0 = time.process_time()
    t_start = time.time()
    while True:
        if kq:
            kq.control([ev], 1, 0.05)
        else:
            time.sleep(interval)
        while True:
            data = os.read(fd, 1 << 20)
            if not data:
                break
            reads += 1
            now = time.time_ns()
            buf += data
            *lines, buf = buf.split(b"\n")
            for ln in lines:
                row = json.loads(ln)
                lags.append((now - row["t"]) / 1e6)
                seqs.append(row["seq"])
        if proc.poll() is not None and time.time() - t_start > secs + 1:
            break
    cpu = time.process_time() - cpu0
    os.close(fd)
    return lags, seqs, cpu, time.time() - t_start, reads


def pct(v, p):
    v = sorted(v)
    return v[min(len(v) - 1, int(p * len(v)))]


def main():
    secs = 5
    print("%-9s %8s %9s %9s %9s %9s %9s %8s %s" % ("tailer", "lines/s", "lines", "lag_p50", "lag_p99", "lag_max", "tail_cpu", "reads", "lost/torn"))
    for rate in (100, 1000, 10000, 50000):
        for mode in ("poll250", "poll50", "kqueue"):
            path = os.path.join(HERE, "work", "report-%s-%d.jsonl" % (mode, rate))
            os.makedirs(os.path.dirname(path), exist_ok=True)
            open(path, "w").close()
            w = subprocess.Popen([sys.executable, "-c", WRITER, path, str(rate), str(secs)], stdout=subprocess.PIPE, text=True)
            lags, seqs, cpu, wall, reads = tail(path, mode, secs, w)
            written = int(w.stdout.read().strip())
            w.wait()
            ok = seqs == list(range(1, len(seqs) + 1)) and len(seqs) == written
            print("%-9s %8d %9d %7.0f ms %7.0f ms %7.0f ms %7.2f%% %8d %s" % (
                mode, rate, len(lags), pct(lags, .5), pct(lags, .99), max(lags), 100 * cpu / wall, reads,
                "none (%d of %d)" % (len(seqs), written) if ok else "MISMATCH %d of %d" % (len(seqs), written)))
            os.unlink(path)
    # a line written in two pieces must not be parsed half
    path = os.path.join(HERE, "work", "report-torn.jsonl")
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_TRUNC)
    rfd = os.open(path, os.O_RDONLY)
    os.write(fd, b'{"seq":1,"t":1,"x":"ab')
    d1 = os.read(rfd, 4096)
    os.write(fd, b'cd"}\n')
    d2 = os.read(rfd, 4096)
    print("\n[M] a line written in two pieces: first read %r ends without a newline, second read completes it; a tailer must hold the partial tail (%s does)" % (
        d1[-8:], "this one" if (d1 + d2).endswith(b"\n") else "?"))
    os.close(fd); os.close(rfd); os.unlink(path)
    print("[M] bytes per line: %d" % len(('{"seq":%d,"t":%d,"kind":"metric","name":"files.checked","v":%d,"note":"%s"}\n' % (1, time.time_ns(), 5, "x" * 60))))


if __name__ == "__main__":
    main()
