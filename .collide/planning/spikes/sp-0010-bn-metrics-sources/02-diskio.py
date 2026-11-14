#!/usr/bin/env python3
"""S8 measurement 3: per-process disk I/O through proc_pid_rusage, and the mach-time CPU units.

A child process does known I/O; the parent reads its rusage while it runs and compares with wait4's ru_oublock/ru_inblock.
"""
import fcntl
import os
import select
import subprocess
import sys
import tempfile
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "lib"))
import libproc  # noqa: E402

CHILD = r'''
import os, sys, time
d, mode, mb = sys.argv[1], sys.argv[2], int(sys.argv[3])
path = os.path.join(d, "f.bin")
chunk = os.urandom(1 << 20)
sys.stdout.write("ready\n"); sys.stdout.flush(); sys.stdin.readline()
if mode == "write_fsync":
    with open(path, "wb") as f:
        for _ in range(mb):
            f.write(chunk)
        f.flush(); os.fsync(f.fileno())
elif mode == "write_nofsync_delete":
    with open(path, "wb") as f:
        for _ in range(mb):
            f.write(chunk)
    os.unlink(path)
elif mode == "read_cached":
    for _ in range(2):
        with open(path, "rb") as f:
            while f.read(1 << 20):
                pass
elif mode == "read_nocache":
    import fcntl
    with open(path, "rb") as f:
        fcntl.fcntl(f.fileno(), 48, 1)  # F_NOCACHE
        while f.read(1 << 20):
            pass
elif mode == "small_files":
    for i in range(mb):  # mb files of 4 KB
        with open(os.path.join(d, "s%d" % i), "wb") as f:
            f.write(chunk[:4096])
sys.stdout.write("done\n"); sys.stdout.flush(); sys.stdin.readline()
'''


def case(d, mode, mb):
    p = subprocess.Popen([sys.executable, "-c", CHILD, d, mode, str(mb)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
    p.stdout.readline()
    before = libproc.rusage(p.pid)
    p.stdin.write("go\n"); p.stdin.flush()
    mid = []
    while True:
        r = libproc.rusage(p.pid)
        mid.append((time.time(), r.ri_diskio_byteswritten - before.ri_diskio_byteswritten, r.ri_diskio_bytesread - before.ri_diskio_bytesread))
        # poll for completion without blocking: the child prints "done" then waits for stdin
        if select.select([p.stdout], [], [], 0.02)[0]:
            p.stdout.readline()
            break
    after = libproc.rusage(p.pid)
    p.stdin.write("bye\n"); p.stdin.flush()
    _, status, ru = os.wait4(p.pid, 0)
    return before, after, mid, ru


def main():
    print("mach_timebase_info: numer/denom = %d/%d (1 tick = %.3f ns)" % (libproc.TIMEBASE + (libproc.TIMEBASE[0] / libproc.TIMEBASE[1],)))
    # CPU units: burn ~0.5 s and compare rusage ticks with wait4 utime+stime
    p = subprocess.Popen([sys.executable, "-c", "import sys,time;sys.stdout.write('r\\n');sys.stdout.flush();sys.stdin.readline();e=time.time()+0.5\nwhile time.time()<e: pass\nsys.stdout.write('d\\n');sys.stdout.flush();sys.stdin.readline()"],
                         stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
    p.stdout.readline(); p.stdin.write("go\n"); p.stdin.flush(); p.stdout.readline()
    ri = libproc.rusage(p.pid)
    p.stdin.write("x\n"); p.stdin.flush()
    _, _, ru = os.wait4(p.pid, 0)
    u, s = libproc.cpu_ns(ri)
    print("CPU units: rusage user+sys = %d + %d ticks -> %.3f s with 125/3; raw ticks would read %.3f s; wait4 says %.3f s" % (
        ri.ri_user_time, ri.ri_system_time, (u + s) / 1e9, (ri.ri_user_time + ri.ri_system_time) / 1e9, ru.ru_utime + ru.ru_stime))
    base = os.path.join(HERE, "work")
    os.makedirs(base, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=base) as d:
        print("\n%-24s %10s %10s %10s %10s %10s %10s %8s" % ("case", "expected", "diskW", "diskR", "logicalW", "ru_oublk", "ru_inblk", "updates"))
        for mode, mb, expected in [("write_fsync", 128, 128 << 20), ("write_nofsync_delete", 64, 64 << 20), ("read_cached", 0, 128 << 20),
                                   ("read_nocache", 0, 128 << 20), ("small_files", 500, 500 * 4096)]:
            if mode.startswith("read"):
                # Written here, outside the measured child. The no-cache case writes with F_NOCACHE so its pages are not
                # left in the buffer cache and the later read has to reach the disk.
                with open(os.path.join(d, "f.bin"), "wb") as f:
                    if mode == "read_nocache":
                        fcntl.fcntl(f.fileno(), 48, 1)
                    for _ in range(128):
                        f.write(os.urandom(1 << 20))
                    f.flush(); os.fsync(f.fileno())
            before, after, mid, ru = case(d, mode, mb)
            dw = after.ri_diskio_byteswritten - before.ri_diskio_byteswritten
            dr = after.ri_diskio_bytesread - before.ri_diskio_bytesread
            lw = after.ri_logical_writes - before.ri_logical_writes
            changes = sum(1 for a, b in zip(mid, mid[1:]) if (a[1], a[2]) != (b[1], b[2]))
            print("%-24s %10d %10d %10d %10d %10d %10d %8d" % (mode, expected, dw, dr, lw, ru.ru_oublock, ru.ru_inblock, changes))
    print("\nupdates = how many of the ~20 ms polls saw the counters move while the child ran (live counters, not exit-only)")


if __name__ == "__main__":
    main()
