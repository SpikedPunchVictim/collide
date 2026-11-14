"""Stand-in process owner: a tiny workflow engine plus a socket server.

The Engine runs a list of phases in order. It is hosted either by a detached
daemon (option a: `owner.py daemon <state_dir>`) or inside the app process
(option b: app.py --host-engine). All state is in <state_dir>/state.json, so a
new host reattaches to running children by pid.

Spawn modes:
  bare  the host forks the child itself (setsid, stdout/stderr to a log file).
        Exit status and rusage come from wait4 while the host is the parent;
        after a reattach the host is not the parent and only sees the pid vanish.
  shim  the host starts fixtures/shim.py, which parents the child and writes
        exit.json with status and rusage, so they survive a host restart.

Phase kinds:
  proc       argv run as above.
  claude-bg  `claude --bg <prompt>`; the phase ends when `claude agents --json
             --all` reports state "done"; the owner then runs `claude stop`.
"""
import json
import os
import re
import secrets
import select
import signal
import socket
import subprocess
import sys
import time

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "lib"))
import s3  # noqa: E402


class Engine:
    def __init__(self, state_dir, host):
        self.dir = state_dir
        self.path = os.path.join(state_dir, "state.json")
        self.state = s3.read_json(self.path)
        if self.state is None:
            raise SystemExit("no state.json in %s" % state_dir)
        self.host = host
        self.my_children = set()  # pids this host forked (so wait4 works)
        self.event("host_start", host=host, host_pid=os.getpid())
        for ph in self.state["phases"]:
            if ph["state"] == "running":
                self.event("reattach", phase=ph["name"], pid=ph.get("pid"),
                           alive=s3.alive(ph["pid"]) if ph.get("pid") else None,
                           start_time_matches=(s3.start_time(ph["pid"]) == ph.get("pid_start"))
                           if ph.get("pid") else None)

    # ---- persistence -------------------------------------------------
    def save(self):
        s3.write_json(self.path, self.state)

    def event(self, kind, **kw):
        kw.update({"t": round(time.time(), 3), "event": kind})
        with open(os.path.join(self.dir, "events.jsonl"), "a") as f:
            f.write(json.dumps(kw) + "\n")

    def done(self):
        return all(ph["state"] == "ended" for ph in self.state["phases"])

    def env(self):
        env = {k: v for k, v in os.environ.items() if not k.startswith("CLAUDE")}
        env["COLLIDE_RUN"] = self.state["run"]
        return env

    # ---- the loop body -------------------------------------------------
    def tick(self):
        self._reap_strays()
        for ph in self.state["phases"]:
            if ph["state"] == "ended":
                continue
            if ph["state"] == "pending":
                self._start(ph)
            elif ph["state"] == "running":
                self._check(ph)
            return  # phases are sequential

    def _start(self, ph):
        ph["log"] = os.path.join(self.dir, "logs", ph["name"] + ".log")
        os.makedirs(os.path.dirname(ph["log"]), exist_ok=True)
        if ph["kind"] == "proc":
            if self.state["mode"] == "shim":
                exit_file = os.path.join(self.dir, "logs", ph["name"] + ".exit.json")
                argv = [s3.PY, os.path.join(s3.FIXTURES, "shim.py"), "--exit-file", exit_file,
                        "--log", ph["log"], "--"] + ph["argv"]
                shim_pid = self._fork_exec(argv, os.devnull)
                ph["shim_pid"], ph["exit_file"] = shim_pid, exit_file
                deadline = time.time() + 5
                while not os.path.exists(exit_file + ".pid") and time.time() < deadline:
                    time.sleep(0.01)
                with open(exit_file + ".pid") as f:
                    ph["pid"] = int(f.read())
            else:
                ph["pid"] = self._fork_exec(ph["argv"], ph["log"])
            ph["pid_start"] = s3.start_time(ph["pid"])
        elif ph["kind"] == "claude-bg":
            out = s3.run_claude(["--bg", ph["prompt"], "--model", "haiku"] + ph.get("extra", []),
                                env=self.env(), cwd=ph.get("cwd", self.dir))
            m = re.search(r"backgrounded\s+\S+\s+([0-9a-f]{8})", out.stdout)
            ph["launcher"] = {"rc": out.returncode, "stdout": out.stdout[-400:], "stderr": out.stderr[-400:]}
            if not m:
                ph["state"], ph["ended"] = "ended", time.time()
                ph["exit"], ph["exit_source"] = None, "launch failed"
                self.save()
                self.event("phase_end", phase=ph["name"], exit=None, source="launch failed")
                return
            ph["bg_id"] = m.group(1)
            with open(os.path.join(s3.WORK, "bg_sessions.txt"), "a") as f:
                f.write(ph["bg_id"] + "\n")
        ph["state"], ph["started"] = "running", time.time()
        ph["started_by"] = {"host": self.host, "pid": os.getpid()}
        self.save()
        self.event("phase_start", phase=ph["name"], pid=ph.get("pid"), bg_id=ph.get("bg_id"))

    def _fork_exec(self, argv, log):
        pid = os.fork()
        if pid == 0:
            try:
                os.setsid()
                fd = os.open(log, os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o644)
                os.dup2(fd, 1)
                os.dup2(fd, 2)
                nul = os.open(os.devnull, os.O_RDONLY)
                os.dup2(nul, 0)
                os.execve(argv[0], argv, self.env())
            finally:
                os._exit(127)
        self.my_children.add(pid)
        return pid

    def _reap_strays(self):
        # Reap shims we forked; their real result is exit.json.
        for pid in list(self.my_children):
            if any(ph.get("pid") == pid for ph in self.state["phases"] if ph["state"] == "running"):
                continue
            try:
                got, _ = os.waitpid(pid, os.WNOHANG)
                if got:
                    self.my_children.discard(pid)
            except ChildProcessError:
                self.my_children.discard(pid)

    def _check(self, ph):
        if ph["kind"] == "claude-bg":
            return self._check_bg(ph)
        result = None
        if self.state["mode"] == "shim":
            rec = s3.read_json(ph["exit_file"])
            if rec is not None:
                result = (rec, "shim exit.json")
        elif ph["pid"] in self.my_children:
            got, status, ru = os.wait4(ph["pid"], os.WNOHANG)
            if got:
                self.my_children.discard(ph["pid"])
                rec = s3.decode_status(status)
                rec.update(s3.rusage_dict(ru))
                result = (rec, "wait4 (host is parent)")
        elif not s3.alive(ph["pid"]):
            try:
                os.wait4(ph["pid"], os.WNOHANG)
                note = "wait4 unexpectedly worked"
            except ChildProcessError as e:
                note = "wait4 -> %s" % e.strerror
            result = (None, "pid vanished; not our child (%s)" % note)
        if result:
            ph["state"], ph["ended"] = "ended", time.time()
            ph["exit"], ph["exit_source"] = result
            ph["ended_by"] = {"host": self.host, "pid": os.getpid()}
            self.save()
            self.event("phase_end", phase=ph["name"], exit=result[0], source=result[1])

    def _check_bg(self, ph):
        if time.time() - ph.get("last_poll", 0) < 1.0:
            return
        ph["last_poll"] = time.time()
        out = s3.run_claude(["agents", "--json", "--all"])
        try:
            rows = json.loads(out.stdout)
        except json.JSONDecodeError:
            return
        row = next((r for r in rows if r.get("id") == ph["bg_id"]), None)
        if row is None:
            return
        ph.setdefault("seen", [])
        snap = {k: row.get(k) for k in ("state", "status", "pid")}
        if not ph["seen"] or ph["seen"][-1]["snap"] != snap:
            ph["seen"].append({"t": round(time.time(), 2), "snap": snap})
        ph["session_id"] = row.get("sessionId")
        if row.get("pid"):
            ph["pid"] = row["pid"]
        if row.get("state") in ("done", "failed", "error", "stopped"):
            stop = s3.run_claude(["stop", ph["bg_id"]])
            ph["stop"] = {"rc": stop.returncode, "out": (stop.stdout + stop.stderr).strip()[-200:]}
            ph["state"], ph["ended"] = "ended", time.time()
            ph["exit"], ph["exit_source"] = {"bg_state": row.get("state")}, "claude agents --json"
            ph["ended_by"] = {"host": self.host, "pid": os.getpid()}
            self.save()
            self.event("phase_end", phase=ph["name"], exit=ph["exit"], source=ph["exit_source"])
        else:
            self.save()


def new_state(state_dir, run_id, mode, phases):
    os.makedirs(state_dir, exist_ok=True)
    for ph in phases:
        ph.setdefault("kind", "proc")
        ph["state"] = "pending"
    s3.write_json(os.path.join(state_dir, "state.json"),
                  {"run": run_id, "mode": mode, "phases": phases})


# ---- option (a): the daemon -------------------------------------------------

def serve(state_dir):
    """Run the engine and a JSON-lines socket server until the run is done."""
    os.chdir(state_dir)  # bind a relative path: macOS caps sun_path at 104 bytes
    token = secrets.token_hex(16)
    fd = os.open("token", os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    os.write(fd, token.encode())
    os.close(fd)
    try:
        os.unlink("owner.sock")
    except FileNotFoundError:
        pass
    srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    srv.bind("owner.sock")
    srv.listen(8)
    eng = Engine(state_dir, "daemon")
    stopping = []
    signal.signal(signal.SIGTERM, lambda *_: stopping.append(1))
    with open("owner.pid", "w") as f:
        f.write(str(os.getpid()))
    linger_until = None
    while not stopping:
        eng.tick()
        if eng.done() and linger_until is None:
            eng.event("run_done")
            linger_until = time.time() + 2.0  # let a connected app read the last lines
        if linger_until and time.time() > linger_until:
            break
        r, _, _ = select.select([srv], [], [], 0.1)
        if r:
            conn, _ = srv.accept()
            handle(conn, token, eng)
    eng.event("host_exit", reason="sigterm" if stopping else "run done")


def handle(conn, token, eng):
    try:
        conn.settimeout(2)
        req = json.loads(conn.makefile().readline())
        if req.get("token") != token:
            resp = {"error": "bad token"}
        elif req["method"] == "status":
            resp = {"result": eng.state, "owner_pid": os.getpid()}
        elif req["method"] == "tail":
            ph = next(p for p in eng.state["phases"] if p["name"] == req["params"]["phase"])
            data = b""
            if ph.get("log") and os.path.exists(ph["log"]):
                with open(ph["log"], "rb") as f:
                    f.seek(req["params"]["offset"])
                    data = f.read(1 << 20)
            resp = {"result": data.decode(errors="replace"), "next": req["params"]["offset"] + len(data)}
        else:
            resp = {"error": "unknown method"}
        conn.sendall((json.dumps(resp) + "\n").encode())
    except Exception as e:  # a bad client must not kill the owner
        try:
            conn.sendall((json.dumps({"error": repr(e)}) + "\n").encode())
        except OSError:
            pass
    finally:
        conn.close()


def daemonize(state_dir):
    """Double fork + setsid so the daemon is not the caller's child."""
    pid = os.fork()
    if pid:
        os.waitpid(pid, 0)
        return
    os.setsid()
    if os.fork():
        os._exit(0)
    log = os.open(os.path.join(state_dir, "daemon.log"), os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o644)
    os.dup2(log, 1)
    os.dup2(log, 2)
    nul = os.open(os.devnull, os.O_RDONLY)
    os.dup2(nul, 0)
    try:
        serve(state_dir)
    finally:
        os._exit(0)


def start_daemon(state_dir, timeout=5):
    """Start a detached daemon for state_dir and return its pid once it serves."""
    try:
        os.unlink(os.path.join(state_dir, "owner.pid"))
    except FileNotFoundError:
        pass
    subprocess.run([s3.PY, os.path.abspath(__file__), "daemon", state_dir], check=True)
    deadline = time.time() + timeout
    while time.time() < deadline:
        pid_file = os.path.join(state_dir, "owner.pid")
        if os.path.exists(pid_file):
            with open(pid_file) as f:
                txt = f.read().strip()
            if txt:
                return int(txt)
        time.sleep(0.02)
    raise RuntimeError("daemon did not start in %s" % state_dir)


def rpc(state_dir, method, **params):
    with open(os.path.join(state_dir, "token")) as f:
        token = f.read().strip()
    c = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    cwd = os.getcwd()
    os.chdir(state_dir)
    try:
        c.connect("owner.sock")
    finally:
        os.chdir(cwd)
    c.sendall((json.dumps({"token": token, "method": method, "params": params}) + "\n").encode())
    resp = json.loads(c.makefile().readline())
    c.close()
    return resp


if __name__ == "__main__":
    if len(sys.argv) == 3 and sys.argv[1] == "daemon":
        daemonize(os.path.abspath(sys.argv[2]))
    else:
        raise SystemExit("usage: owner.py daemon <state_dir>")
