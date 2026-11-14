#!/usr/bin/env python3
"""Sleep detection: the uptime clock (mach_absolute_time) stops while the Mac sleeps; the continuous clock
(mach_continuous_time) does not. For two readings, slept = (continuous delta) - (uptime delta).

  sleepclock.py selftest              checks the math without sleeping
  sleepclock.py watch [--max-s N]     ticks once a second and reports each gap (used by sleep-check.sh)
"""
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import libproc  # noqa: E402

THRESHOLD_NS = 2_000_000_000  # a gap above this between two one-second ticks counts as sleep; scheduling jitter is far below it


def read():
    """(uptime_ns, continuous_ns, wall_ns), read as close together as possible."""
    a = libproc.mach_absolute_ns()
    c = libproc.mach_continuous_ns()
    w = time.time_ns()
    return a, c, w


def slept_ns(prev, cur):
    return max(0, (cur[1] - prev[1]) - (cur[0] - prev[0]))


def sysctl_timeval(name):
    out = subprocess.run(["sysctl", "-n", name], capture_output=True, text=True).stdout
    # "{ sec = 1794646000, usec = 123 } Sat Nov 14 ..."
    try:
        sec = int(out.split("sec =")[1].split(",")[0])
        usec = int(out.split("usec =")[1].split("}")[0])
        return sec + usec / 1e6
    except Exception:
        return None


def selftest():
    print("[M] two clocks over 5 s with no sleep:")
    r0 = read()
    time.sleep(5)
    r1 = read()
    print("    uptime +%.6f s, continuous +%.6f s, wall +%.6f s, slept by the formula %.6f s (expected ~0)" % (
        (r1[0] - r0[0]) / 1e9, (r1[1] - r0[1]) / 1e9, (r1[2] - r0[2]) / 1e9, slept_ns(r0, r1) / 1e9))
    n = 20000
    t = time.perf_counter()
    for _ in range(n):
        read()
    print("[M] one reading of both clocks plus wall time costs %.2f us" % (1e6 * (time.perf_counter() - t) / n))

    print("[M, math only] injected readings: a 60 s sleep makes the continuous clock jump while the uptime clock does not:")
    base = (10_000_000_000, 50_000_000_000, 0)
    after = (base[0] + 1_000_000_000, base[1] + 1_000_000_000 + 60_000_000_000, 0)
    print("    uptime +1.0 s, continuous +61.0 s -> slept %.1f s" % (slept_ns(base, after) / 1e9))
    jitter = (base[0] + 1_040_000_000, base[1] + 1_040_000_000, 0)
    print("    a 40 ms late tick (both clocks +1.04 s) -> slept %.3f s; below the %.0f s threshold, so not sleep" % (
        slept_ns(base, jitter) / 1e9, THRESHOLD_NS / 1e9))

    a, c, w = r1
    boot = sysctl_timeval("kern.boottime")
    total_sleep = (c - a) / 1e9
    print("[M] since boot: continuous %.1f s, uptime %.1f s, so the Mac has slept %.1f s (%.2f h) in total" % (c / 1e9, a / 1e9, total_sleep, total_sleep / 3600))
    if boot:
        print("[M] check: wall now minus kern.boottime = %.1f s; continuous clock = %.1f s; difference %.2f s" % (w / 1e9 - boot, c / 1e9, w / 1e9 - boot - c / 1e9))
    st, wt = sysctl_timeval("kern.sleeptime"), sysctl_timeval("kern.waketime")
    if st and wt and wt > st:
        last = wt - st
        print("[M] kern.sleeptime to kern.waketime: the last sleep lasted %.1f s (%.2f h), within the %.1f s total: %s" % (
            last, last / 3600, total_sleep, last <= total_sleep))
        # The gap method applied to that real sleep: the continuous clock at wake minus the uptime clock at wake cannot be
        # read after the fact, so only the total and the wall-clock identity above are checked, not this one sleep alone.
    print("[I] a dark wake (maintenance, lid closed) runs the CPU, so the uptime clock advances through it and it is not counted as sleep.")


def watch(max_s):
    prev = read()
    t_end = time.time() + max_s
    found = None
    print("watching; sleep the Mac now (Apple menu, Sleep), wait about a minute, wake it", flush=True)
    while time.time() < t_end:
        time.sleep(1)
        cur = read()
        gap = (cur[1] - prev[1]) - (cur[0] - prev[0])
        if gap > THRESHOLD_NS:
            found = {"slept_s": gap / 1e9, "uptime_advanced_s": (cur[0] - prev[0]) / 1e9, "continuous_advanced_s": (cur[1] - prev[1]) / 1e9,
                     "wall_advanced_s": (cur[2] - prev[2]) / 1e9}
            print("gap detected: slept %.2f s by (continuous - uptime); uptime advanced %.2f s, continuous %.2f s, wall %.2f s" % (
                found["slept_s"], found["uptime_advanced_s"], found["continuous_advanced_s"], found["wall_advanced_s"]), flush=True)
            st, wt = sysctl_timeval("kern.sleeptime"), sysctl_timeval("kern.waketime")
            if st and wt:
                print("kern.sleeptime to kern.waketime says %.2f s (the OS's own record of the last sleep)" % (wt - st), flush=True)
            print("compare with your own stopwatch; the gap method and the OS record should agree within about 1-2 s", flush=True)
            time.sleep(3)
            break
        prev = cur
    if not found:
        print("no sleep seen in %d s" % max_s)
    return found


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "watch":
        mx = float(sys.argv[sys.argv.index("--max-s") + 1]) if "--max-s" in sys.argv else 900
        watch(mx)
    else:
        selftest()
