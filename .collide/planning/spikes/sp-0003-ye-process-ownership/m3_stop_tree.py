"""Measurement 3: stop a hostile tree (options a and b share this stop code).

The owner starts fixtures/tree.py as a hook in its own session and process
group, with COLLIDE_RUN=<id>. Then it stops it the way 0.5 describes:
  1. discover: ppid walk from the hook, marker scan through `ps -E`,
     and the hook's process group;
  2. SIGTERM to the group and to every pid found; grace period;
  3. rescan, SIGKILL whatever is left;
  4. run the cleanup hook (fixtures/cleanup.py), which reads lane.json;
  5. check survivors with ps (recorded pids and a fresh marker scan) and
     `docker ps -a --filter label=collide.spike=1`.
Prints which roles each discovery method found and what survived each step.

  python3 m3_stop_tree.py [--grace 2]
"""
import argparse
import json
import os
import signal
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "lib"))
import s3  # noqa: E402


def roles(sd):
    d = os.path.join(sd, "pids")
    return {name: int(open(os.path.join(d, name)).read()) for name in sorted(os.listdir(d))}


def alive_roles(r):
    return sorted(name for name, pid in r.items() if s3.alive(pid) and not s3.is_zombie(pid))


def timed(fn, *a):
    t0 = time.time()
    v = fn(*a)
    return v, round((time.time() - t0) * 1000)


def orphan_suspects(since):
    out = subprocess.run(["ps", "-axo", "pid=,ppid=,uid=,lstart="], capture_output=True, text=True).stdout
    found = set()
    for line in out.splitlines():
        f = line.split(None, 3)
        if len(f) < 4 or f[1] != "1" or int(f[2]) != os.getuid():
            continue
        started = time.mktime(time.strptime(f[3].strip(), "%a %b %d %H:%M:%S %Y"))
        if started >= int(since) and not s3.env_of(int(f[0])) and not has_env(int(f[0])):
            found.add(int(f[0]))
    return found


def has_env(pid):
    out = subprocess.run(["ps", "-E", "-ww", "-o", "command=", "-p", str(pid)], capture_output=True, text=True).stdout
    return " PATH=" in out or " HOME=" in out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--grace", type=float, default=2.0)
    a = ap.parse_args()
    run_id = s3.new_run_id("m3")
    sd = s3.run_dir(run_id)
    env = {k: v for k, v in os.environ.items() if not k.startswith("CLAUDE")}
    env["COLLIDE_RUN"] = run_id

    run_start = time.time()
    hook = os.fork()
    if hook == 0:
        os.setsid()
        fd = os.open(os.path.join(sd, "hook.log"), os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o644)
        os.dup2(fd, 1)
        os.dup2(fd, 2)
        os.execve(s3.PY, [s3.PY, os.path.join(s3.FIXTURES, "tree.py"), "--state-dir", sd, "--docker"], env)
    deadline = time.time() + 60
    while not os.path.exists(os.path.join(sd, "ready")) and time.time() < deadline:
        time.sleep(0.1)
    if not os.path.exists(os.path.join(sd, "ready")):
        raise SystemExit("tree did not become ready; see %s/hook.log" % sd)

    r = roles(sd)
    lane = s3.read_json(os.path.join(sd, "lane.json"))
    table, table_ms = timed(s3.ps_table)
    pgid = os.getpgid(hook)
    walk, walk_ms = timed(s3.ppid_walk, [hook], table)
    marked, marker_ms = timed(s3.marker_scan, run_id)
    group = {pid for pid, row in table.items() if row["pgid"] == pgid}
    s3.say("run %s  hook pid %d  pgid %d  processes on machine: %d" % (run_id, hook, pgid, len(table)))
    s3.say("scan cost: ps table %d ms + ppid walk %d ms; marker scan %d ms (ps -E)" % (table_ms, walk_ms, marker_ms))
    s3.say("%-18s %7s %5s %7s %6s %6s  %s" % ("role", "pid", "ppid", "walk", "marker", "group", "ps -E shows COLLIDE_RUN"))
    for name, pid in r.items():
        row = table.get(pid, {})
        s3.say("%-18s %7d %5s %7s %6s %6s  %s" % (
            name, pid, row.get("ppid"), pid in walk, pid in marked, pid in group, s3.env_of(pid) or "-"))
    s3.say("container(s) from lane.json: %s; docker ps by label: %s"
           % ([c[:12] for c in lane["containers"]], [c[:12] for c in s3.docker_ids(run_id)]))
    if lane.get("docker_error"):
        s3.say("docker error: " + lane["docker_error"])

    # Stop: SIGTERM, grace, rescan, SIGKILL.
    t0 = time.time()
    s3.signal_group(pgid, signal.SIGTERM)
    s3.signal_pids(walk | marked, signal.SIGTERM)
    time.sleep(a.grace)
    s3.reap_any()
    rescan = s3.ppid_walk([p for p in walk if s3.alive(p)]) | s3.marker_scan(run_id)
    left = {p for p in (walk | marked | rescan) if s3.alive(p) and not s3.is_zombie(p)}
    after_term = alive_roles(r)
    s3.signal_pids(left, signal.SIGKILL)
    s3.wait_gone(left, 5)
    after_kill = alive_roles(r)
    stop_s = round(time.time() - t0, 2)
    s3.say("after SIGTERM + %.1fs grace, alive: %s" % (a.grace, after_term))
    s3.say("after SIGKILL of rescanned pids (stop took %.2fs), alive: %s; containers: %s"
           % (stop_s, after_kill, [c[:12] for c in s3.docker_ids(run_id)]))

    # Cleanup hook.
    out = subprocess.run([s3.PY, os.path.join(s3.FIXTURES, "cleanup.py"), "--state-dir", sd, "--run-id", run_id],
                         capture_output=True, text=True, env=env)
    s3.say("cleanup hook: " + out.stdout.strip())
    time.sleep(0.5)
    s3.reap_any()
    final_roles = alive_roles(r)
    final_marked = sorted(s3.marker_scan(run_id))
    final_containers = s3.docker_ids(run_id)
    s3.say("FINAL alive roles: %s; marker scan: %s; docker ps -a label=collide.run=%s: %s"
           % (final_roles, final_marked, run_id, final_containers))

    # What a "report survivors" pass could see: same-user orphans (ppid 1) started
    # since the run began whose environment ps cannot read. Report only, never kill:
    # other tools on this machine start such processes too.
    suspects = orphan_suspects(run_start)
    s3.say("suspect orphans (ppid 1, started since run start, env unreadable): %d, of which ours: %s"
           % (len(suspects), sorted(n for n, p in r.items() if p in suspects)))
    escaped = {n: p for n, p in r.items() if n in final_roles}
    if escaped:
        s3.signal_pids(set(escaped.values()), signal.SIGKILL)  # spike safety net, not a Collide mechanism
        s3.wait_gone(set(escaped.values()), 3)
        s3.say("safety net: SIGKILLed escaped roles %s by recorded pid" % sorted(escaped))
    result = {"run": run_id, "escaped_everything": sorted(escaped), "suspects": len(suspects), "roles": r, "walk": sorted(walk), "marked": sorted(marked), "group": sorted(group),
              "after_term": after_term, "after_kill": after_kill, "final_roles": final_roles,
              "final_marked": final_marked, "final_containers": final_containers,
              "walk_ms": walk_ms, "marker_ms": marker_ms, "stop_s": stop_s}
    s3.write_json(os.path.join(sd, "result.json"), result)
    s3.say("zero survivors after scans + cleanup hook" if not (final_roles or final_marked or final_containers)
           else "SURVIVED scans + cleanup hook: %s" % sorted(escaped))
    s3.say("after safety net, alive roles: %s" % alive_roles(r))


if __name__ == "__main__":
    main()
