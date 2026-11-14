"""Per-child shim: the parent that outlives an owner restart.

The shim starts in its own session, forks the real child with stdout and
stderr on a log file, reaps it with wait4, and writes exit.json (status plus
rusage) atomically. An owner that restarts reads exit.json instead of needing
to be the parent. Usage:
  shim.py --exit-file X --log L -- argv...
"""
import json
import os
import signal
import sys
import time


def main():
    args = sys.argv[1:]
    sep = args.index("--")
    opts, argv = args[:sep], args[sep + 1:]
    exit_file = opts[opts.index("--exit-file") + 1]
    log = opts[opts.index("--log") + 1]

    try:
        os.setsid()
    except PermissionError:
        pass  # already a session leader
    signal.signal(signal.SIGHUP, signal.SIG_IGN)

    started = time.time()
    pid = os.fork()
    if pid == 0:
        signal.signal(signal.SIGHUP, signal.SIG_DFL)
        fd = os.open(log, os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o644)
        os.dup2(fd, 1)
        os.dup2(fd, 2)
        nul = os.open(os.devnull, os.O_RDONLY)
        os.dup2(nul, 0)
        os.execv(argv[0], argv)

    with open(exit_file + ".pid", "w") as f:
        f.write(str(pid))

    # Forward a stop to the child; the shim itself stays to write exit.json.
    def forward(sig, _frame):
        try:
            os.kill(pid, sig)
        except ProcessLookupError:
            pass

    signal.signal(signal.SIGTERM, forward)
    signal.signal(signal.SIGINT, forward)

    while True:
        try:
            _, status, ru = os.wait4(pid, 0)
            break
        except InterruptedError:
            continue

    rec = {"pid": pid, "started": started, "ended": time.time(),
           "cpu_s": round(ru.ru_utime + ru.ru_stime, 3),
           "maxrss_mb": round(ru.ru_maxrss / 1048576, 1)}
    if os.WIFEXITED(status):
        rec["exit_code"] = os.WEXITSTATUS(status)
    elif os.WIFSIGNALED(status):
        rec["signal"] = os.WTERMSIG(status)
    tmp = exit_file + ".tmp"
    with open(tmp, "w") as f:
        json.dump(rec, f)
    os.replace(tmp, exit_file)


if __name__ == "__main__":
    main()
