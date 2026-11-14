"""Stand-in for the Collide app.

  app.py client <state_dir>   option (a): talks to the daemon over the socket,
                              mirrors each phase log into app_view/ from saved
                              offsets, so a relaunch continues where it stopped.
  app.py host <state_dir>     option (b): the app hosts the engine itself and
                              owns the children; mirrors logs the same way.

The harness "quits" the app by signalling this process.
"""
import json
import os
import sys
import time

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "lib"))
import s3  # noqa: E402
import owner  # noqa: E402


def mirror(view_dir, offsets, phase, data, next_offset):
    with open(os.path.join(view_dir, phase + ".log"), "ab") as f:
        f.write(data)
    offsets[phase] = next_offset


def log_event(state_dir, kind, **kw):
    kw.update({"t": round(time.time(), 3), "event": kind, "app_pid": os.getpid()})
    with open(os.path.join(state_dir, "app_events.jsonl"), "a") as f:
        f.write(json.dumps(kw) + "\n")


def client(state_dir):
    view = os.path.join(state_dir, "app_view")
    os.makedirs(view, exist_ok=True)
    off_path = os.path.join(view, "offsets.json")
    offsets = s3.read_json(off_path, {})
    log_event(state_dir, "app_start", mode="client", resumed_offsets=offsets)
    unreachable_since = None
    while True:
        try:
            st = owner.rpc(state_dir, "status")["result"]
            if unreachable_since:
                log_event(state_dir, "owner_back", after_s=round(time.time() - unreachable_since, 2))
                unreachable_since = None
            for ph in st["phases"]:
                if not ph.get("log"):
                    continue
                r = owner.rpc(state_dir, "tail", phase=ph["name"], offset=offsets.get(ph["name"], 0))
                if r.get("result"):
                    mirror(view, offsets, ph["name"], r["result"].encode(), r["next"])
            s3.write_json(off_path, offsets)
        except (OSError, ValueError, KeyError):
            if unreachable_since is None:
                unreachable_since = time.time()
                log_event(state_dir, "owner_unreachable")
            st = s3.read_json(os.path.join(state_dir, "state.json"))
        if st and all(p["state"] == "ended" for p in st["phases"]) and unreachable_since:
            log_event(state_dir, "app_exit", reason="run done, owner exited")
            return
        time.sleep(0.2)


def host(state_dir):
    view = os.path.join(state_dir, "app_view")
    os.makedirs(view, exist_ok=True)
    off_path = os.path.join(view, "offsets.json")
    offsets = s3.read_json(off_path, {})
    log_event(state_dir, "app_start", mode="host", resumed_offsets=offsets)
    eng = owner.Engine(state_dir, "app")
    while True:
        eng.tick()
        for ph in eng.state["phases"]:
            if ph.get("log") and os.path.exists(ph["log"]):
                with open(ph["log"], "rb") as f:
                    f.seek(offsets.get(ph["name"], 0))
                    data = f.read()
                if data:
                    mirror(view, offsets, ph["name"], data, offsets.get(ph["name"], 0) + len(data))
        s3.write_json(off_path, offsets)
        if eng.done():
            eng.event("run_done")
            log_event(state_dir, "app_exit", reason="run done")
            return
        time.sleep(0.1)


if __name__ == "__main__":
    mode, state_dir = sys.argv[1], os.path.abspath(sys.argv[2])
    {"client": client, "host": host}[mode](state_dir)
