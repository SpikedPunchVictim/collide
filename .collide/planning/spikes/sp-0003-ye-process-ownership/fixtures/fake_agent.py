"""Fake agent: burns CPU, holds memory, and writes numbered lines to stdout.

stdout is a file the owner opened (never a pipe), so the agent keeps writing
while the owner or the app is gone. Usage:
  fake_agent.py --seconds 6 --burn 0.5 --alloc-mb 100 --exit 3 --tag p1
"""
import argparse
import resource
import sys
import time


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--seconds", type=float, default=5)
    ap.add_argument("--burn", type=float, default=0.0, help="seconds of busy CPU at start")
    ap.add_argument("--alloc-mb", type=int, default=0)
    ap.add_argument("--exit", type=int, default=0)
    ap.add_argument("--tag", default="agent")
    ap.add_argument("--interval", type=float, default=0.1)
    a = ap.parse_args()

    hold = bytearray(a.alloc_mb * 1024 * 1024)
    for i in range(0, len(hold), 4096):
        hold[i] = 1  # touch every page so it counts toward RSS
    end_burn = time.process_time() + a.burn
    x = 0
    while time.process_time() < end_burn:
        x += 1

    deadline = time.time() + a.seconds
    seq = 0
    while time.time() < deadline:
        sys.stdout.write("seq=%d t=%.3f tag=%s\n" % (seq, time.time(), a.tag))
        sys.stdout.flush()
        seq += 1
        time.sleep(a.interval)
    ru = resource.getrusage(resource.RUSAGE_SELF)
    sys.stdout.write("done tag=%s self_cpu=%.3f self_maxrss_mb=%.1f\n"
                     % (a.tag, ru.ru_utime + ru.ru_stime, ru.ru_maxrss / 1048576))
    sys.stdout.flush()
    del hold
    sys.exit(a.exit)


if __name__ == "__main__":
    main()
