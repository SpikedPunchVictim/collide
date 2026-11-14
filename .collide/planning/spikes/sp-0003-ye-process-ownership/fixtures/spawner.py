"""Parent with one worker grandchild, for rusage tests.

  spawner.py --mode waited --burn 1 --alloc-mb 300 --truth F
      starts the worker, waits for it, exits.
  spawner.py --mode orphan --burn 1 --alloc-mb 300 --truth F
      double-forks the worker (launchd adopts it) and exits at once.
The worker writes its own getrusage(RUSAGE_SELF) to F at exit: ground truth
to compare with what an owner can see.
"""
import argparse
import json
import os
import resource
import time


def worker(a):
    hold = bytearray(a.alloc_mb * 1024 * 1024)
    for i in range(0, len(hold), 4096):
        hold[i] = 1
    end = time.process_time() + a.burn
    while time.process_time() < end:
        pass
    time.sleep(a.linger)
    ru = resource.getrusage(resource.RUSAGE_SELF)
    with open(a.truth, "w") as f:
        json.dump({"pid": os.getpid(), "cpu_s": round(ru.ru_utime + ru.ru_stime, 3),
                   "maxrss_mb": round(ru.ru_maxrss / 1048576, 1)}, f)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--mode", choices=["waited", "orphan"], required=True)
    ap.add_argument("--burn", type=float, default=1.0)
    ap.add_argument("--alloc-mb", type=int, default=300)
    ap.add_argument("--linger", type=float, default=0.5)
    ap.add_argument("--truth", required=True)
    a = ap.parse_args()
    if a.mode == "waited":
        pid = os.fork()
        if pid == 0:
            worker(a)
            os._exit(0)
        os.waitpid(pid, 0)
    else:
        pid = os.fork()
        if pid == 0:
            os.setsid()
            if os.fork() == 0:
                worker(a)
            os._exit(0)
        os.waitpid(pid, 0)


if __name__ == "__main__":
    main()
