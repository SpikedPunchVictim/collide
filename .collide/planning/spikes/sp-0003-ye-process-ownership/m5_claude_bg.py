"""Option (c): real `claude --bg` sessions as the agents (costs a few cents).

  python3 m5_claude_bg.py            # parts 1-3
  python3 m5_claude_bg.py 2          # one part
  S3_BG_CWD=/a/trusted/dir python3 m5_claude_bg.py   # default ~/projects

Part 1  The Collide daemon (option a) runs two claude-bg phases with the app
        closed. It is SIGKILLed during phase 1 and restarted 2 s later. Prints
        what `claude agents --json --all` reports over time, the session's
        process chain and environment, the transcript the owner can tail, the
        hand-off delay, and what a non-parent can learn about the exit.
Part 2  A --bg session runs fixtures/tree.py (setsid'd grandchild, double-forked
        orphans, a container) through its Bash tool. Then `claude stop`, then
        Collide's own scans, the cleanup hook, and a survivor check.
Part 3  `--max-budget-usd 0.0001` on a --bg session: is the cap enforced?
Part 4  With the claude daemon already running, a second session is launched
        with a different COLLIDE_RUN: which value do its process and its Bash
        children carry?

Every claude call goes through claude-clean.sh (no CLAUDE* variables) and uses
--model haiku. The script stops and removes every session it started.
"""
import glob
import json
import os
import re
import select
import signal
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "lib"))
sys.path.insert(0, HERE)
import s3  # noqa: E402
import owner  # noqa: E402

STARTED_SESSIONS = []
# `claude --bg` refuses a directory that was never trusted interactively
# ("Workspace not trusted"), and trust was not inherited by subdirectories here.
# Sessions therefore run in an already-trusted directory; they write nothing
# there (the tree fixture writes only under this spike's work/ directory).
BG_CWD = os.environ.get("S3_BG_CWD", os.path.expanduser("~/projects"))
NOTE_EXITSTATUS = 0x04000000


def claude(*args, env=None, cwd=None):
    return s3.run_claude(args, env=env, cwd=cwd)


def agents():
    out = claude("agents", "--json", "--all").stdout
    try:
        return json.loads(out)
    except json.JSONDecodeError:
        return []


def agent_row(bg_id):
    return next((r for r in agents() if r.get("id") == bg_id), None)


def daemon_status():
    out = claude("daemon", "status").stdout
    m = re.search(r"pid:\s+(\d+)", out)
    origin = re.search(r"origin:\s+(.*)", out)
    return {"pid": int(m.group(1)) if m else None, "origin": origin.group(1)[:140] if origin else None}


def launch_bg(prompt, env, cwd, extra=()):
    r = claude("--bg", prompt, "--model", "haiku", *extra, env=env, cwd=cwd)
    m = re.search(r"backgrounded\s+\S+\s+([0-9a-f]{8})", r.stdout)
    if not m:
        raise SystemExit("claude --bg failed: rc=%d %s %s" % (r.returncode, r.stdout, r.stderr))
    STARTED_SESSIONS.append(m.group(1))
    with open(os.path.join(s3.WORK, "bg_sessions.txt"), "a") as f:
        f.write(m.group(1) + "\n")
    return m.group(1)


def transcript(session_id):
    hits = glob.glob(os.path.expanduser("~/.claude/projects/*/%s.jsonl" % session_id))
    return hits[0] if hits else None


def read_transcript(path):
    recs, texts, usage = [], [], {"input": 0, "cache_write": 0, "cache_read": 0, "output": 0}
    models = set()
    with open(path) as f:
        for line in f:
            try:
                d = json.loads(line)
            except json.JSONDecodeError:
                continue
            recs.append(d.get("type"))
            if d.get("type") == "cost-state":
                usage["total_cost_usd"] = d.get("totalCostUSD")
            msg = d.get("message") if isinstance(d.get("message"), dict) else {}
            if d.get("type") == "assistant":
                models.add(msg.get("model"))
                u = msg.get("usage") or {}
                usage["input"] += u.get("input_tokens", 0)
                usage["cache_write"] += u.get("cache_creation_input_tokens", 0)
                usage["cache_read"] += u.get("cache_read_input_tokens", 0)
                usage["output"] += u.get("output_tokens", 0)
                for c in msg.get("content", []):
                    if c.get("type") == "text":
                        texts.append(c["text"])
    return {"records": len(recs), "types": sorted(set(recs)), "assistant_texts": texts,
            "usage": usage, "models": sorted(m for m in models if m)}


def chain(pid):
    table = s3.ps_table()
    out = []
    while pid and pid in table and len(out) < 8:
        out.append("%d %s" % (pid, table[pid]["command"][:70]))
        if pid == 1:
            break
        pid = table[pid]["ppid"]
    return out


def watch_exit(pid):
    kq = select.kqueue()
    kq.control([select.kevent(pid, filter=select.KQ_FILTER_PROC, flags=select.KQ_EV_ADD | select.KQ_EV_ONESHOT,
                              fflags=select.KQ_NOTE_EXIT | NOTE_EXITSTATUS)], 0, 0)
    return kq


def wait_until(pred, timeout, step=0.5):
    deadline = time.time() + timeout
    while time.time() < deadline:
        v = pred()
        if v:
            return v
        time.sleep(step)
    return None


def stop_and_remove(bg_id):
    a = claude("stop", bg_id)
    b = claude("rm", bg_id)
    return (a.stdout + a.stderr).strip(), (b.stdout + b.stderr).strip()


# ---------------------------------------------------------------- part 1

def part1():
    s3.say("== part 1: two claude --bg phases under the Collide daemon, app closed, daemon restarted")
    run_id = s3.new_run_id("c1")
    sd = s3.run_dir(run_id)
    ws = BG_CWD
    phases = [{"name": "p1", "kind": "claude-bg", "cwd": ws,
               "prompt": "Reply with exactly PHASE-ONE-OK and nothing else."},
              {"name": "p2", "kind": "claude-bg", "cwd": ws,
               "prompt": "Reply with exactly PHASE-TWO-OK and nothing else."}]
    owner.new_state(sd, run_id, "bare", phases)
    s3.say("claude daemon before: %s" % daemon_status())
    d1 = owner.start_daemon(sd)
    st = lambda: s3.read_json(os.path.join(sd, "state.json"))  # noqa: E731
    p1 = wait_until(lambda: st()["phases"][0].get("bg_id") and st()["phases"][0], 30, 0.1)
    bg1 = p1["bg_id"]
    os.kill(d1, signal.SIGKILL)
    killed_at = time.time()
    s3.say("p1 launched as %s; Collide daemon %d SIGKILLed" % (bg1, d1))
    row = wait_until(lambda: (agent_row(bg1) or {}).get("pid") and agent_row(bg1), 30)
    spid = row["pid"]
    try:
        kq = watch_exit(spid)
    except OSError as e:
        kq = None
        s3.say("kqueue on session pid failed: %s" % e)
    s3.say("p1 session pid %d chain: %s" % (spid, chain(spid)))
    s3.say("p1 session env COLLIDE_RUN per ps -E: %s (launcher had %s)" % (s3.env_of(spid) or "none", run_id))
    s3.say("claude daemon now: %s" % daemon_status())
    tpath = wait_until(lambda: transcript(row["sessionId"]), 20)
    sizes = []
    for _ in range(6):
        sizes.append(os.path.getsize(tpath) if tpath else None)
        time.sleep(0.5)
    s3.say("p1 transcript %s readable while running; sizes every 0.5 s: %s" % (tpath, sizes))
    time.sleep(max(0, 2.0 - (time.time() - killed_at)))
    d2 = owner.start_daemon(sd)
    s3.say("Collide daemon restarted as %d after %.1f s" % (d2, time.time() - killed_at))
    wait_until(lambda: all(p["state"] == "ended" for p in st()["phases"]), 240, 1)
    got = kq.control(None, 1, 10) if kq else None
    final = st()
    for ph in final["phases"]:
        tp = transcript(ph.get("session_id", "")) if ph.get("session_id") else None
        tr = read_transcript(tp) if tp else {}
        s3.say("%s id=%s states seen=%s stop=%s exit_source=%s" % (
            ph["name"], ph.get("bg_id"), [x["snap"] for x in ph.get("seen", [])], ph.get("stop"),
            ph.get("exit_source")))
        s3.say("   transcript: records=%s types=%s replies=%s usage=%s models=%s" % (
            tr.get("records"), tr.get("types"), tr.get("assistant_texts"), tr.get("usage"), tr.get("models")))
        s3.say("   started_by=%s ended_by=%s" % (ph.get("started_by"), ph.get("ended_by")))
    h = final["phases"][1]["started"] - final["phases"][0]["ended"]
    s3.say("hand-off p1 end -> p2 start: %.2f s (app never open; daemon restarted mid-p1)" % h)
    s3.say("p1 session exit seen by kqueue (non-parent): %s" % (
        s3.decode_status(got[0].data) if got else "no event"))
    logs = claude("logs", bg1)
    s3.say("`claude logs %s` after stop: rc=%d, %d bytes, starts: %r" % (
        bg1, logs.returncode, len(logs.stdout), re.sub(r"\s+", " ", logs.stdout)[:160]))
    s3.say("agents --json row after stop: %s" % json.dumps(agent_row(bg1)))
    for ph in final["phases"]:
        if ph.get("bg_id"):
            s3.say("rm %s: %s" % (ph["bg_id"], claude("rm", ph["bg_id"]).stdout.strip()))
    return {"run": run_id, "daemon_marker": s3.env_of(spid)}


# ---------------------------------------------------------------- part 2

def part2():
    s3.say("== part 2: stop a hostile tree started by a --bg session's Bash tool")
    run_id = s3.new_run_id("c2")
    sd = s3.run_dir(run_id)
    ws = BG_CWD
    tree_dir = os.path.join(sd, "tree")
    env = {k: v for k, v in os.environ.items() if not k.startswith("CLAUDE")}
    env["COLLIDE_RUN"] = run_id
    cmd = "/usr/bin/python3 %s --state-dir %s --docker" % (os.path.join(s3.FIXTURES, "tree.py"), tree_dir)
    prompt = ("Use the Bash tool with run_in_background set to true to run exactly this command, "
              "and do not wait for it or check on it: %s\nThen reply with exactly STARTED." % cmd)
    s3.say("claude daemon before: %s" % daemon_status())
    bg = launch_bg(prompt, env, ws, ("--allowedTools", "Bash(/usr/bin/python3 *)"))
    ready = wait_until(lambda: os.path.exists(os.path.join(tree_dir, "ready")), 120, 0.5)
    row = agent_row(bg)
    if not ready:
        s3.say("tree never became ready; agents row: %s" % json.dumps(row))
        s3.say("stop/rm: %s" % (stop_and_remove(bg),))
        return
    spid = row.get("pid")
    hook_env = s3.read_json(os.path.join(tree_dir, "hook_env.json"))
    r = {n: int(open(os.path.join(tree_dir, "pids", n)).read()) for n in os.listdir(os.path.join(tree_dir, "pids"))}
    seen_marker = hook_env.get("COLLIDE_RUN")
    s3.say("session %s pid %s; launcher COLLIDE_RUN=%s; hook saw COLLIDE_RUN=%s; CLAUDE* in hook env: %s" % (
        bg, spid, run_id, seen_marker, sorted(k for k in hook_env if k.startswith("CLAUDE"))))
    s3.say("hook chain: %s" % chain(r["hook"]))
    table = s3.ps_table()
    walk = s3.ppid_walk([spid], table) if spid else set()
    marked_launcher = s3.marker_scan(run_id)
    marked_seen = s3.marker_scan(seen_marker) if seen_marker else set()
    s3.say("%-18s %7s %6s %16s %14s" % ("role", "pid", "walk", "marker(launcher)", "marker(hook's)"))
    for n, pid in sorted(r.items()):
        s3.say("%-18s %7d %6s %16s %14s" % (n, pid, pid in walk, pid in marked_launcher, pid in marked_seen))
    s3.say("marker(hook's) also matches %d processes outside this tree" % len(marked_seen - set(r.values())))
    wait_until(lambda: (agent_row(bg) or {}).get("state") == "done", 60, 1)
    s3.say("agents row before stop: %s" % json.dumps({k: (agent_row(bg) or {}).get(k) for k in ("state", "status", "pid")}))
    t0 = time.time()
    out = claude("stop", bg)
    s3.say("`claude stop` rc=%d in %.2f s: %s" % (out.returncode, time.time() - t0, out.stdout.strip()))
    time.sleep(2)
    alive = sorted(n for n, pid in r.items() if s3.alive(pid) and not s3.is_zombie(pid))
    s3.say("2 s after `claude stop`, alive roles: %s; session pid alive: %s; containers: %s" % (
        alive, s3.alive(spid) if spid else None, [c[:12] for c in s3.docker_ids(seen_marker or run_id)]))
    # Collide's own stop over what it found before `claude stop`.
    targets = {p for p in (walk | marked_launcher | s3.marker_scan(seen_marker or run_id))
               if p in r.values() and s3.alive(p)}
    foreign = (walk | marked_launcher) - set(r.values()) - {spid}
    s3.signal_pids(targets, signal.SIGTERM)
    time.sleep(1)
    s3.signal_pids({p for p in targets if s3.alive(p)}, signal.SIGKILL)
    s3.wait_gone(targets, 3)
    alive2 = sorted(n for n, pid in r.items() if s3.alive(pid) and not s3.is_zombie(pid))
    s3.say("after Collide's scans (own tree pids only; %d other found pids left alone), alive: %s"
           % (len(foreign), alive2))
    out = subprocess.run([s3.PY, os.path.join(s3.FIXTURES, "cleanup.py"), "--state-dir", tree_dir,
                          "--run-id", seen_marker or run_id], capture_output=True, text=True)
    s3.say("cleanup hook: %s" % out.stdout.strip())
    time.sleep(0.5)
    alive3 = sorted(n for n, pid in r.items() if s3.alive(pid) and not s3.is_zombie(pid))
    s3.say("after cleanup hook, alive: %s; containers: %s" % (alive3, s3.docker_ids(seen_marker or run_id)))
    if alive3:
        s3.signal_pids({r[n] for n in alive3}, signal.SIGKILL)
        s3.wait_gone({r[n] for n in alive3}, 3)
        s3.say("safety net SIGKILLed: %s" % alive3)
    s3.say("stop/rm: %s" % (stop_and_remove(bg),))
    tp = transcript(row.get("sessionId", ""))
    if tp:
        tr = read_transcript(tp)
        s3.say("transcript: replies=%s usage=%s" % (tr["assistant_texts"][-2:], tr["usage"]))


# ---------------------------------------------------------------- part 3

def part3():
    s3.say("== part 3: --max-budget-usd 0.0001 with --bg, on a turn that needs several API calls")
    run_id = s3.new_run_id("c3")
    sd = s3.run_dir(run_id)
    env = {k: v for k, v in os.environ.items() if not k.startswith("CLAUDE")}
    env["COLLIDE_RUN"] = run_id
    dump = os.path.join(sd, "n.json")
    cmd = "/usr/bin/python3 %s %s" % (os.path.join(s3.FIXTURES, "envdump.py"), dump)
    prompt = ("Run this exact command with the Bash tool three times, one call at a time, waiting for each "
              "to finish before the next: %s\nThen reply with exactly DONE." % cmd)
    bg = launch_bg(prompt, env, BG_CWD, ("--max-budget-usd", "0.0001", "--allowedTools", "Bash(/usr/bin/python3 *)"))
    row = wait_until(lambda: (agent_row(bg) or {}).get("state") in ("done", "blocked", "failed", "stopped")
                     and agent_row(bg), 120, 1)
    time.sleep(2)
    tp = transcript((row or {}).get("sessionId", ""))
    tr = read_transcript(tp) if tp else {}
    calls = 0
    if tp:
        with open(tp) as f:
            for line in f:
                d = json.loads(line)
                if d.get("type") == "assistant":
                    calls += sum(1 for c in d["message"].get("content", []) if c.get("type") == "tool_use")
    s3.say("state=%s tool calls=%d replies=%s usage=%s" % ((row or {}).get("state"), calls,
                                                           tr.get("assistant_texts"), tr.get("usage")))
    s3.say("budget cap 0.0001 enforced: %s" % ("no: the session made %d tool calls and finished, spending $%s"
                                                % (calls, tr.get("usage", {}).get("total_cost_usd"))
                                                if calls >= 2 else "possibly (stopped early)"))
    s3.say("stop/rm: %s" % (stop_and_remove(bg),))


# ---------------------------------------------------------------- part 4

def part4():
    s3.say("== part 4: whose environment does a --bg session get when the claude daemon is already running?")
    run_id = s3.new_run_id("c4")
    sd = s3.run_dir(run_id)
    base = {k: v for k, v in os.environ.items() if not k.startswith("CLAUDE")}
    env_a = dict(base, COLLIDE_RUN=run_id + "-A")
    env_b = dict(base, COLLIDE_RUN=run_id + "-B")
    s3.say("claude daemon before: %s" % daemon_status())
    a = launch_bg("Reply with exactly A-OK and nothing else.", env_a, BG_CWD)
    wait_until(lambda: (agent_row(a) or {}).get("state") == "done", 60, 1)
    dpid = daemon_status()["pid"]
    s3.say("session A %s done; claude daemon pid %s env COLLIDE_RUN=%s" % (a, dpid, s3.env_of(dpid) if dpid else None))
    dump = os.path.join(sd, "b_env.json")
    b = launch_bg("Use the Bash tool to run exactly this command: /usr/bin/python3 %s %s\nThen reply with exactly DONE."
                  % (os.path.join(s3.FIXTURES, "envdump.py"), dump), env_b, BG_CWD,
                  ("--allowedTools", "Bash(/usr/bin/python3 *)"))
    row = wait_until(lambda: (agent_row(b) or {}).get("pid") and agent_row(b), 60, 0.5)
    s3.say("session B %s pid %s: ps -E COLLIDE_RUN=%s; process: %s" % (
        b, row.get("pid"), s3.env_of(row["pid"]), chain(row["pid"])[0]))
    got = wait_until(lambda: s3.read_json(dump), 90, 1)
    s3.say("B's Bash child env: %s (launcher B had %s, daemon/A had %s)" % (got, env_b["COLLIDE_RUN"], env_a["COLLIDE_RUN"]))
    for x in (a, b):
        s3.say("stop/rm %s: %s" % (x, stop_and_remove(x)))


def main():
    parts = sys.argv[1:] or ["1", "2", "3", "4"]
    started_daemon_before = daemon_status()
    try:
        for p in parts:
            {"1": part1, "2": part2, "3": part3, "4": part4}[p]()
    finally:
        for bg in STARTED_SESSIONS:
            row = agent_row(bg)
            if row and row.get("pid"):
                s3.say("cleanup: stopping leftover session %s: %s" % (bg, stop_and_remove(bg)))
        s3.say("claude daemon before this script: %s; now: %s" % (started_daemon_before, daemon_status()))


if __name__ == "__main__":
    main()
