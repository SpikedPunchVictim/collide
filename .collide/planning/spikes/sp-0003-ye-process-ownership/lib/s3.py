"""Shared helpers for the S3 process-ownership spike.

Everything here is scoped to this spike's own processes: discovery is by a
COLLIDE_RUN value this spike generated, by pids it recorded, or by the
collide.spike=1 container label. Nothing signals a process by name.
"""
import errno
import json
import os
import re
import signal
import subprocess
import time

SPIKE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FIXTURES = os.path.join(SPIKE, "fixtures")
WORK = os.path.join(SPIKE, "work")
PY = "/usr/bin/python3"
CLAUDE = os.path.join(SPIKE, "claude-clean.sh")
SPIKE_LABEL = "collide.spike=1"


def new_run_id(tag):
    return "s3-%s-%d-%d" % (tag, os.getpid(), int(time.time()))


def run_dir(run_id):
    d = os.path.join(WORK, run_id)
    os.makedirs(d, exist_ok=True)
    return d


def write_json(path, obj):
    tmp = path + ".tmp"
    with open(tmp, "w") as f:
        json.dump(obj, f, indent=1, sort_keys=True)
    os.replace(tmp, path)


def read_json(path, default=None):
    try:
        with open(path) as f:
            return json.load(f)
    except (FileNotFoundError, json.JSONDecodeError):
        return default


def alive(pid):
    """True while pid exists (a zombie counts as existing)."""
    try:
        os.kill(pid, 0)
        return True
    except ProcessLookupError:
        return False
    except PermissionError:
        return True


def ps_table():
    """pid -> dict(ppid, pgid, stat, command) for every process of every user."""
    out = subprocess.run(["ps", "-axww", "-o", "pid=,ppid=,pgid=,stat=,command="],
                         capture_output=True, text=True, check=True).stdout
    table = {}
    for line in out.splitlines():
        parts = line.split(None, 4)
        if len(parts) < 4:
            continue
        table[int(parts[0])] = {"ppid": int(parts[1]), "pgid": int(parts[2]),
                                "stat": parts[3], "command": parts[4] if len(parts) > 4 else ""}
    return table


def ppid_walk(roots, table=None):
    """Every live descendant of roots (roots included), following ppid links."""
    table = table if table is not None else ps_table()
    children = {}
    for pid, row in table.items():
        children.setdefault(row["ppid"], []).append(pid)
    found, stack = set(), [r for r in roots if r in table]
    while stack:
        pid = stack.pop()
        if pid in found:
            continue
        found.add(pid)
        stack.extend(children.get(pid, []))
    return found


def marker_scan(run_id):
    """Pids whose exec-time environment (as `ps -E` prints it) has COLLIDE_RUN=<run_id>.

    `ps -E` appends the environment after the arguments, so a process whose
    argv contains the same text would also match; the scan excludes this
    process and its `ps` child, and the spike never puts the marker in argv.
    """
    out = subprocess.run(["ps", "-axE", "-ww", "-o", "pid=,command="],
                         capture_output=True, text=True, check=True).stdout
    pat = re.compile(r"(?:^|\s)COLLIDE_RUN=%s(?:\s|$)" % re.escape(run_id))
    me = os.getpid()
    found = set()
    for line in out.splitlines():
        parts = line.strip().split(None, 1)
        if len(parts) == 2 and pat.search(parts[1]):
            pid = int(parts[0])
            if pid != me:
                found.add(pid)
    return found


def env_of(pid):
    """COLLIDE_RUN value(s) `ps -E` shows for one pid, or None if ps shows no environment."""
    out = subprocess.run(["ps", "-E", "-ww", "-o", "command=", "-p", str(pid)],
                         capture_output=True, text=True).stdout
    vals = re.findall(r"(?:^|\s)COLLIDE_RUN=(\S+)", out)
    return vals


def signal_pids(pids, sig):
    sent = []
    for pid in sorted(pids):
        if pid <= 1 or pid == os.getpid():
            continue
        try:
            os.kill(pid, sig)
            sent.append(pid)
        except ProcessLookupError:
            pass
    return sent


def signal_group(pgid, sig):
    try:
        os.killpg(pgid, sig)
        return True
    except ProcessLookupError:
        return False


def wait_gone(pids, timeout):
    """Wait until every pid is gone (zombies of our own children are reaped)."""
    deadline = time.time() + timeout
    while time.time() < deadline:
        reap_any()
        left = {p for p in pids if alive(p) and not is_zombie(p)}
        if not left:
            return set()
        time.sleep(0.05)
    return {p for p in pids if alive(p) and not is_zombie(p)}


def is_zombie(pid):
    out = subprocess.run(["ps", "-o", "stat=", "-p", str(pid)], capture_output=True, text=True).stdout
    return out.strip().startswith("Z")


def reap_any():
    while True:
        try:
            pid, _ = os.waitpid(-1, os.WNOHANG)
        except ChildProcessError:
            return
        if pid == 0:
            return


def docker_ids(run_id=None):
    args = ["docker", "ps", "-a", "-q", "--filter", "label=" + SPIKE_LABEL]
    if run_id:
        args += ["--filter", "label=collide.run=" + run_id]
    out = subprocess.run(args, capture_output=True, text=True).stdout
    return [x for x in out.split() if x]


def start_time(pid):
    """Process start time as ps prints it; used to guard against pid reuse on reattach."""
    out = subprocess.run(["ps", "-o", "lstart=", "-p", str(pid)], capture_output=True, text=True).stdout
    return out.strip() or None


def rusage_dict(ru):
    return {"utime_s": round(ru.ru_utime, 3), "stime_s": round(ru.ru_stime, 3),
            "cpu_s": round(ru.ru_utime + ru.ru_stime, 3),
            # macOS reports ru_maxrss in bytes (Linux: KiB).
            "maxrss_mb": round(ru.ru_maxrss / (1024 * 1024), 1)}


def decode_status(status):
    if os.WIFEXITED(status):
        return {"exit_code": os.WEXITSTATUS(status)}
    if os.WIFSIGNALED(status):
        return {"signal": os.WTERMSIG(status)}
    return {"raw": status}


def check_log_continuity(path):
    """A fake agent writes `seq=N t=T` lines. Returns count, gaps, and time span."""
    seqs, times = [], []
    try:
        with open(path) as f:
            for line in f:
                m = re.match(r"seq=(\d+) t=([\d.]+)", line)
                if m:
                    seqs.append(int(m.group(1)))
                    times.append(float(m.group(2)))
    except FileNotFoundError:
        return {"lines": 0, "gaps": None}
    gaps = [(a, b) for a, b in zip(seqs, seqs[1:]) if b != a + 1]
    return {"lines": len(seqs), "gaps": gaps, "first_t": times[0] if times else None,
            "last_t": times[-1] if times else None}


def lines_written_between(path, t0, t1):
    n = 0
    with open(path) as f:
        for line in f:
            m = re.match(r"seq=(\d+) t=([\d.]+)", line)
            if m and t0 <= float(m.group(2)) <= t1:
                n += 1
    return n


def run_claude(args, env=None, cwd=None):
    """Run claude-clean.sh with stdout/stderr on temp files, not pipes.

    `claude --bg` may start the claude daemon, which could inherit a pipe and
    keep a pipe reader waiting until the daemon exits.
    """
    import tempfile
    with tempfile.TemporaryFile() as out, tempfile.TemporaryFile() as err:
        rc = subprocess.run([CLAUDE] + list(args), stdout=out, stderr=err, stdin=subprocess.DEVNULL,
                            env=env, cwd=cwd).returncode
        out.seek(0)
        err.seek(0)
        return subprocess.CompletedProcess(args, rc, out.read().decode(errors="replace"),
                                           err.read().decode(errors="replace"))


def say(*parts):
    print(*parts, flush=True)
