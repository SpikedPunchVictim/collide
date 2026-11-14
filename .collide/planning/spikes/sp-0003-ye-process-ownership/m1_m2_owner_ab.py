"""Measurements 1 and 2 for options (a) and (b), with fake agents.

  python3 m1_m2_owner_ab.py            # runs a/bare, a/shim, b/bare, b/shim
  python3 m1_m2_owner_ab.py a shim     # one combination

Option (a): a detached daemon owns the children; the app is a socket client.
  - the app quits during phase 1 and relaunches during phase 2;
  - phase 1 ends while the app is closed (does phase 2 start?);
  - the daemon is SIGKILLed during phase 2 and restarted 2 s later.
Option (b): the app hosts the engine and owns the children.
  - the app quits during phase 1 and relaunches 4 s after phase 1 ends;
    for (b) quitting the app is also the owner restart.

Prints, per phase: exit status and where it came from, CPU and max RSS,
log lines and gaps, whether the app's mirrored view equals the log, and the
delay between one phase ending and the next starting.
"""
import json
import os
import signal
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "lib"))
sys.path.insert(0, HERE)
import s3  # noqa: E402
import owner  # noqa: E402

AGENT = os.path.join(s3.FIXTURES, "fake_agent.py")


def phases():
    return [
        {"name": "p1", "argv": [s3.PY, AGENT, "--seconds", "6", "--burn", "0.5", "--alloc-mb", "100",
                                "--exit", "3", "--tag", "p1"]},
        {"name": "p2", "argv": [s3.PY, AGENT, "--seconds", "6", "--burn", "0.3", "--alloc-mb", "50",
                                "--exit", "0", "--tag", "p2"]},
        {"name": "p3", "argv": [s3.PY, AGENT, "--seconds", "2", "--exit", "0", "--tag", "p3"]},
    ]


def state(d):
    return s3.read_json(os.path.join(d, "state.json"))


def wait_for(pred, timeout, step=0.05):
    deadline = time.time() + timeout
    while time.time() < deadline:
        v = pred()
        if v:
            return v
        time.sleep(step)
    return None


def phase(d, name):
    return next(p for p in state(d)["phases"] if p["name"] == name)


def launch_app(d, mode):
    return subprocess.Popen([s3.PY, os.path.join(HERE, "app.py"), mode, d],
                            stdout=open(os.path.join(d, "app.out"), "ab"), stderr=subprocess.STDOUT,
                            start_new_session=True)


def quit_app(proc):
    proc.send_signal(signal.SIGKILL)  # an abrupt quit; a clean quit leaves the same children
    proc.wait()
    return time.time()


def run_a(mode):
    run_id = s3.new_run_id("a-" + mode)
    d = s3.run_dir(run_id)
    owner.new_state(d, run_id, mode, phases())
    marks = {}
    d1 = owner.start_daemon(d)
    app = launch_app(d, "client")
    wait_for(lambda: phase(d, "p1")["state"] == "running", 5)
    time.sleep(1.5)
    marks["app_closed"] = quit_app(app)
    p2_started = wait_for(lambda: phase(d, "p2").get("started"), 15)
    marks["p2_started_seen"] = time.time()
    time.sleep(1.0)
    app = launch_app(d, "client")
    marks["app_reopened"] = time.time()
    time.sleep(1.0)
    os.kill(d1, signal.SIGKILL)
    marks["owner_killed"] = time.time()
    time.sleep(2.0)
    d2 = owner.start_daemon(d)
    marks["owner_restarted"] = time.time()
    wait_for(lambda: state(d) and all(p["state"] == "ended" for p in state(d)["phases"]), 30)
    app.wait(timeout=15)
    time.sleep(0.3)
    survivors = [p for p in (d1, d2, app.pid) if s3.alive(p) and not s3.is_zombie(p)]
    return report("a", mode, d, run_id, marks, survivors,
                  [("app_closed", "app_closed", "app_reopened"), ("owner_down", "owner_killed", "owner_restarted")])


def run_b(mode):
    run_id = s3.new_run_id("b-" + mode)
    d = s3.run_dir(run_id)
    owner.new_state(d, run_id, mode, phases())
    marks = {}
    app = launch_app(d, "host")
    wait_for(lambda: state(d)["phases"][0]["state"] == "running", 5)
    time.sleep(1.5)
    marks["app_closed"] = quit_app(app)
    # Phase 1 ends with nobody watching; keep the app closed 4 s past that.
    wait_for(lambda: not s3.alive(phase(d, "p1")["pid"]), 15)
    marks["p1_pid_gone"] = time.time()
    time.sleep(4.0)
    app = launch_app(d, "host")
    marks["app_reopened"] = time.time()
    app.wait(timeout=40)
    survivors = [p for p in (app.pid,) if s3.alive(p) and not s3.is_zombie(p)]
    return report("b", mode, d, run_id, marks, survivors,
                  [("app_and_owner_down", "app_closed", "app_reopened")])


def report(option, mode, d, run_id, marks, survivors, windows):
    st = state(d)
    out = {"option": option, "mode": mode, "run": run_id, "phases": [], "handoffs": [],
           "survivors": survivors, "leftover_marked": sorted(s3.marker_scan(run_id))}
    for ph in st["phases"]:
        cont = s3.check_log_continuity(ph["log"])
        view = os.path.join(d, "app_view", ph["name"] + ".log")
        same = os.path.exists(view) and open(view, "rb").read() == open(ph["log"], "rb").read()
        row = {"phase": ph["name"], "exit": ph["exit"], "exit_source": ph["exit_source"],
               "lines": cont["lines"], "gaps": cont["gaps"], "app_view_equals_log": same,
               "started_by": ph["started_by"]["host"] + ":" + str(ph["started_by"]["pid"]),
               "ended_by": ph["ended_by"]["host"] + ":" + str(ph["ended_by"]["pid"])}
        for label, a, b in windows:
            if cont["first_t"] and cont["first_t"] < marks[b] and cont["last_t"] > marks[a]:
                row["lines_written_while_" + label] = s3.lines_written_between(ph["log"], marks[a], marks[b])
        out["phases"].append(row)
    for prev, nxt in zip(st["phases"], st["phases"][1:]):
        last_line_t = s3.check_log_continuity(prev["log"])["last_t"]
        out["handoffs"].append({
            "from": prev["name"], "to": nxt["name"],
            "next_start_minus_prev_last_output_s": round(nxt["started"] - last_line_t, 2),
            "app_closed_when_prev_ended": marks["app_closed"] < last_line_t < marks["app_reopened"],
        })
    with open(os.path.join(d, "events.jsonl")) as f:
        out["reattach_events"] = [json.loads(l) for l in f if '"reattach"' in l]
    return out


def sun_path_limit():
    """macOS caps an AF_UNIX path at 104 bytes; the owner binds a relative path."""
    import socket
    longest = None
    for n in (100, 103, 104, 105, 120):
        path = os.path.join(s3.WORK, "x" * max(1, n - len(s3.WORK) - 1))
        sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        try:
            sock.bind(path)
            longest = len(path.encode())
            os.unlink(path)
        except OSError:
            pass
        finally:
            sock.close()
    return longest


def main():
    os.makedirs(s3.WORK, exist_ok=True)
    s3.say("longest AF_UNIX socket path that binds: %s bytes" % sun_path_limit())
    combos = [(a, m) for a in ("a", "b") for m in ("bare", "shim")]
    if len(sys.argv) == 3:
        combos = [(sys.argv[1], sys.argv[2])]
    results = []
    for opt, mode in combos:
        s3.say("== option %s, spawn mode %s" % (opt, mode))
        r = run_a(mode) if opt == "a" else run_b(mode)
        results.append(r)
        for ph in r["phases"]:
            s3.say("  " + json.dumps(ph))
        for h in r["handoffs"]:
            s3.say("  handoff " + json.dumps(h))
        for e in r["reattach_events"]:
            s3.say("  reattach " + json.dumps(e))
        s3.say("  survivors (owner/app pids still alive): %s; marked processes left: %s"
               % (r["survivors"], r["leftover_marked"]))
    s3.write_json(os.path.join(s3.WORK, "m1_m2_owner_ab.json"), results)


if __name__ == "__main__":
    main()
