"""Do children survive when a LaunchServices-launched app exits?

A Tauri app started from Finder runs under launchd as an app. This launches a
minimal .app bundle (fixtures/S3Quit.app, a shell script) with `open`, lets it
start a plain background child, a setsid'd child and a double-forked child,
and exit. Prints which children are still alive 2 s later, then kills them
by recorded pid.

  python3 m6_app_quit.py
"""
import os
import shutil
import signal
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "lib"))
import s3  # noqa: E402


def main():
    run_id = s3.new_run_id("m6")
    d = s3.run_dir(run_id)
    app = os.path.join(d, "S3Quit.app")
    shutil.copytree(os.path.join(s3.FIXTURES, "S3Quit.app"), app)
    with open(os.path.join(d, "S3Quit.target"), "w") as f:
        f.write(d)
    t0 = time.time()
    r = subprocess.run(["open", "-n", "-W", app], capture_output=True, text=True, timeout=60)
    s3.say("open -n -W rc=%d in %.1f s %s" % (r.returncode, time.time() - t0, r.stderr.strip()))
    if not os.path.exists(os.path.join(d, "app.exited")):
        raise SystemExit("app did not run; see %s" % d)
    app_pid, app_ppid, app_pgid = open(os.path.join(d, "app.pid")).read().split()
    s3.say("app pid %s ppid %s pgid %s (ppid 1: launched by launchd)" % (app_pid, app_ppid, app_pgid))
    time.sleep(2)
    pids = {}
    for role in ("plain_child", "setsid_child", "double_fork"):
        pids[role] = int(open(os.path.join(d, role + ".pid")).read())
    table = s3.ps_table()
    for role, pid in pids.items():
        row = table.get(pid)
        s3.say("%-13s pid %6d alive after app exit: %-5s ppid=%s pgid=%s marker=%s" % (
            role, pid, row is not None, row and row["ppid"], row and row["pgid"], s3.env_of(pid) or "-"))
    left = {p for p in pids.values() if s3.alive(p)} | s3.marker_scan(run_id)
    s3.signal_pids(left, signal.SIGKILL)
    s3.wait_gone(left, 3)
    s3.say("cleanup: killed %s; alive now: %s" % (sorted(left), [p for p in pids.values() if s3.alive(p)]))


if __name__ == "__main__":
    main()
