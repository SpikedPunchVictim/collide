"""Fake hook that builds a hard-to-stop process tree, then holds.

Roles (each writes <state_dir>/pids/<role>):
  hook               this process
  child              plain child, same process group as the hook
  setsid_parent      child that stays alive as parent of ...
  setsid_grandchild  ... a grandchild in its own session (escapes a group kill)
  orphan             double-forked, adopted by launchd, ignores SIGTERM
  unsetenv_orphan    double-forked, then deletes COLLIDE_RUN in-process (no exec)
  platform_orphan    double-forked, exec /bin/sleep (an Apple platform binary)
  clean_env_child    child exec'd with only PATH and HOME
  clean_env_orphan   double-forked, exec'd with only PATH and HOME, recorded in
                     lane.json the way Orchard records its servers
and, with --docker, a container labelled collide.spike=1 and collide.run=<id>,
also recorded in lane.json. A cleanup hook (cleanup.py) reads lane.json.

Usage: tree.py --state-dir D [--docker] [--hold 600] [--detach]
  --detach  return to the caller once the tree is ready (the hook keeps holding
            in the background). For running under an agent's Bash tool.
"""
import argparse
import json
import os
import signal
import subprocess
import sys
import time

LOOP = "import time\nwhile True: time.sleep(0.5)"
PY = "/usr/bin/python3"


def record(state_dir, role, pid=None):
    d = os.path.join(state_dir, "pids")
    os.makedirs(d, exist_ok=True)
    with open(os.path.join(d, role), "w") as f:
        f.write(str(pid or os.getpid()))


def clean_env():
    return {"PATH": os.environ.get("PATH", "/usr/bin:/bin"), "HOME": os.environ.get("HOME", "/")}


def double_fork(fn):
    pid = os.fork()
    if pid == 0:
        os.setsid()  # detach from the hook's group the way daemons do
        if os.fork() == 0:
            try:
                fn()
            finally:
                os._exit(0)
        os._exit(0)
    os.waitpid(pid, 0)


def build(a):
    sd = a.state_dir
    lane = {"pids": [], "containers": []}
    record(sd, "hook")
    with open(os.path.join(sd, "hook_env.json"), "w") as f:
        json.dump({k: v for k, v in os.environ.items() if k.startswith(("COLLIDE", "CLAUDE"))}, f)

    # plain child
    p = subprocess.Popen([PY, "-c", LOOP])
    record(sd, "child", p.pid)

    # setsid grandchild under a living parent
    pid = os.fork()
    if pid == 0:
        record(sd, "setsid_parent")
        g = os.fork()
        if g == 0:
            os.setsid()
            record(sd, "setsid_grandchild")
            os.execv(PY, [PY, "-c", LOOP])
        while True:
            time.sleep(0.5)

    # double-forked orphan that ignores SIGTERM
    def orphan():
        record(sd, "orphan")
        signal.signal(signal.SIGTERM, signal.SIG_IGN)
        while True:
            time.sleep(0.5)
    double_fork(orphan)

    def unsetenv_orphan():
        record(sd, "unsetenv_orphan")
        os.environ.pop("COLLIDE_RUN", None)  # calls unsetenv(3); no exec
        while True:
            time.sleep(0.5)
    double_fork(unsetenv_orphan)

    def platform_orphan():
        record(sd, "platform_orphan")
        os.execv("/bin/sleep", ["/bin/sleep", "600"])
    double_fork(platform_orphan)

    # cleaned environment, as a direct child and as a recorded detached server
    p = subprocess.Popen([PY, "-c", LOOP], env=clean_env())
    record(sd, "clean_env_child", p.pid)

    r, w = os.pipe()

    def clean_orphan():
        os.close(r)
        os.write(w, str(os.getpid()).encode())
        os.close(w)
        os.execve(PY, [PY, "-c", LOOP], clean_env())
    double_fork(clean_orphan)
    os.close(w)
    server_pid = int(os.read(r, 32))
    os.close(r)
    record(sd, "clean_env_orphan", server_pid)
    lane["pids"].append(server_pid)

    if a.docker:
        run_id = os.environ.get("COLLIDE_RUN", "none")
        out = subprocess.run(["docker", "run", "-d", "--label", "collide.spike=1",
                              "--label", "collide.run=" + run_id, "alpine", "sleep", "600"],
                             capture_output=True, text=True)
        cid = out.stdout.strip()
        if cid:
            lane["containers"].append(cid)
        else:
            lane["docker_error"] = out.stderr.strip()

    with open(os.path.join(sd, "lane.json.tmp"), "w") as f:
        json.dump(lane, f)
    os.replace(os.path.join(sd, "lane.json.tmp"), os.path.join(sd, "lane.json"))
    with open(os.path.join(sd, "ready"), "w") as f:
        f.write(str(time.time()))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--state-dir", required=True)
    ap.add_argument("--docker", action="store_true")
    ap.add_argument("--hold", type=float, default=600)
    ap.add_argument("--detach", action="store_true")
    a = ap.parse_args()
    os.makedirs(a.state_dir, exist_ok=True)
    if a.detach:
        pid = os.fork()
        if pid:
            ready = os.path.join(a.state_dir, "ready")
            deadline = time.time() + 60
            while not os.path.exists(ready) and time.time() < deadline:
                time.sleep(0.1)
            print("tree ready" if os.path.exists(ready) else "tree not ready", flush=True)
            os._exit(0)
        # The background hook keeps its stdio off the caller's pipe.
        nul = os.open(os.devnull, os.O_RDWR)
        for fd in (0, 1, 2):
            os.dup2(nul, fd)
    build(a)
    time.sleep(a.hold)


if __name__ == "__main__":
    main()
