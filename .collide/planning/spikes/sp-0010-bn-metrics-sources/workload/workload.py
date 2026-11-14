#!/usr/bin/env python3
"""Synthetic workload with the process shape of an Orchard iter run and a lane test (see record.md, "Orchard census").

Launcher:  workload.py --duration 60 --seed 1 --truth out/truth.jsonl [--docker] [--ollama] [--no-lane]
Every process this workload starts is reaped with os.wait4, and the exit totals go to the truth file: that is the
ground truth the sampler's misses are compared with. Python stdlib only; node and docker are used when present.
"""
import json
import os
import random
import shutil
import subprocess
import sys
import threading
import time
import urllib.request
import uuid

HERE = os.path.dirname(os.path.abspath(__file__))
NODE = shutil.which("node") or "node"
TRUTH = os.environ.get("S8_TRUTH")
_lock = threading.Lock()


def _log_truth(row):
    if not TRUTH:
        return
    line = (json.dumps(row) + "\n").encode()
    fd = os.open(TRUTH, os.O_WRONLY | os.O_APPEND | os.O_CREAT, 0o644)
    try:
        os.write(fd, line)  # one write per line, below PIPE_BUF: appends from many processes do not interleave
    finally:
        os.close(fd)


def run(argv, **popen):
    """Start argv, reap it with wait4, record its totals. Returns the exit status."""
    t0 = time.time_ns()
    p = subprocess.Popen(argv, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, **popen)
    pid, status, ru = os.wait4(p.pid, 0)
    t1 = time.time_ns()
    p.returncode = 0  # reaped here, not by Popen
    _log_truth({"pid": pid, "ppid": os.getpid(), "argv0": os.path.basename(argv[0]), "t_spawn": t0, "t_exit": t1,
                "utime": ru.ru_utime, "stime": ru.ru_stime, "maxrss": ru.ru_maxrss,
                "inblock": ru.ru_inblock, "oublock": ru.ru_oublock, "status": status})
    return status


def run_parallel(argvs, width=4):
    pending = list(argvs)
    threads = []

    def worker():
        while True:
            with _lock:
                if not pending:
                    return
                argv = pending.pop()
            run(argv)

    for _ in range(width):
        t = threading.Thread(target=worker)
        t.start()
        threads.append(t)
    for t in threads:
        t.join()


def node_burn(ms):
    return [NODE, "-e", "const e=Date.now()+%d;let n=0;while(Date.now()<e)n++" % ms]


def tiny(rng):
    """One process of the many that live well under 100 ms."""
    return rng.choice([
        ["/usr/bin/true"], ["/bin/sh", "-c", ":"], ["/usr/bin/git", "--version"], ["/bin/echo", "x"],
        [NODE, "-e", "0"], ["/bin/ls", HERE], ["/usr/bin/env", "true"], ["/usr/bin/uname"],
    ])


# ---- steps: each one runs as its own python process, so the tree is launcher -> sh -> step -> children ----

def step_build(rng):
    run([NODE, "-e", "0"])
    run(node_burn(rng.randint(600, 1500)))
    run_parallel([node_burn(rng.randint(150, 700)) for _ in range(6)], width=3)


def step_tsc(rng):
    for _ in range(rng.randint(3, 5)):
        run(node_burn(rng.randint(900, 2200)))
        run(tiny(rng))


def step_preflight(rng):
    for _ in range(rng.randint(40, 80)):
        run(tiny(rng))
    for _ in range(rng.randint(2, 4)):
        run(["/bin/sleep", "%.2f" % rng.uniform(0.2, 0.9)])


def step_suite(rng, port):
    """A test-runner step: workers in parallel, file I/O, localhost requests."""
    work = os.path.join(HERE, "..", "work", "suite-%d" % os.getpid())
    os.makedirs(work, exist_ok=True)
    for i in range(rng.randint(300, 600)):  # many small files, like a dependency tree
        with open(os.path.join(work, "f%d.js" % i), "wb") as f:
            f.write(os.urandom(2048))
    with open(os.path.join(work, "big.bin"), "wb") as f:  # a larger write that reaches the disk
        for _ in range(rng.randint(16, 48)):
            f.write(os.urandom(1 << 20))
        f.flush()
        os.fsync(f.fileno())
    run_parallel([node_burn(rng.randint(200, 2500)) for _ in range(rng.randint(4, 8))], width=4)
    for _ in range(rng.randint(40, 100)):
        urllib.request.urlopen("http://127.0.0.1:%d/blob?n=%d" % (port, rng.choice([1024, 65536, 1 << 20]))).read()
    req = urllib.request.Request("http://127.0.0.1:%d/sink" % port, data=os.urandom(1 << 20), method="POST")
    urllib.request.urlopen(req).read()
    shutil.rmtree(work, ignore_errors=True)
    for _ in range(rng.randint(5, 15)):
        run(tiny(rng))


def step_main():
    kind, seed, port = sys.argv[2], int(sys.argv[3]), int(sys.argv[4])
    rng = random.Random(seed)
    {"build": lambda: step_build(rng), "tsc": lambda: step_tsc(rng), "preflight": lambda: step_preflight(rng),
     "suite": lambda: step_suite(rng, port)}[kind]()


# ---- lane: detached server and a container, as Orchard's dev CLI starts them ----

def lane_up(port_file_dir, docker, host_port):
    """Start a detached server (own session, parent exits) and optionally one container. Returns the lane record."""
    lane = {"serverPid": None, "container": None, "containerName": None}
    # A launcher process starts the server detached and exits, so the server is reparented to launchd, as in Orchard.
    pf = os.path.join(port_file_dir, "lane-%d.port" % os.getpid())  # unique, so a stale file is never read
    if os.path.exists(pf):
        os.unlink(pf)
    lane["portFile"] = pf
    code = ("import subprocess,sys;p=subprocess.Popen([sys.argv[1],sys.argv[2],sys.argv[3]],start_new_session=True,"
            "stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL);print(p.pid)")
    out = subprocess.run([sys.executable, "-c", code, NODE, os.path.join(HERE, "server.js"), pf], capture_output=True, text=True)
    lane["serverPid"] = int(out.stdout.strip())
    for _ in range(100):
        if os.path.exists(pf) and open(pf).read().strip():
            break
        time.sleep(0.05)
    lane["port"] = int(open(pf).read())
    if docker:
        name = "collide-s8-" + uuid.uuid4().hex[:8]
        js = ("const c=require('crypto'),fs=require('fs'),http=require('http');let n=0;setInterval(()=>{const t=Date.now();"
              "while(Date.now()-t<150)c.createHash('sha256').update(String(n++)).digest();"
              "fs.writeFileSync('/tmp/x'+(n%%5),Buffer.alloc(4<<20,1));"
              "http.get('http://host.docker.internal:%d/blob?n=262144',r=>r.resume()).on('error',()=>{});},1000)" % lane["port"])
        r = subprocess.run(["docker", "run", "-d", "--rm", "--name", name, "--label", "collide-s8=1", "node:24-alpine", "node", "-e", js],
                           capture_output=True, text=True)
        if r.returncode == 0:
            lane["container"] = r.stdout.strip()
            lane["containerName"] = name
        else:
            lane["dockerError"] = r.stderr.strip()[:200]
    return lane


def lane_down(lane):
    if lane.get("portFile") and os.path.exists(lane["portFile"]):
        os.unlink(lane["portFile"])
    if lane.get("containerName"):
        subprocess.run(["docker", "rm", "-f", lane["containerName"]], capture_output=True)
    if lane.get("serverPid"):
        try:
            os.kill(lane["serverPid"], 15)  # a process this workload started
        except ProcessLookupError:
            pass


def ollama_generate(n):
    body = json.dumps({"model": "llama3.1:8b", "prompt": "Write a short paragraph about build systems.", "stream": False,
                       "options": {"num_predict": n}}).encode()
    req = urllib.request.Request("http://127.0.0.1:11434/api/generate", data=body, method="POST")
    return json.loads(urllib.request.urlopen(req, timeout=300).read())


def main():
    import argparse
    ap = argparse.ArgumentParser()
    ap.add_argument("--duration", type=float, default=60)
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--truth", default=None)
    ap.add_argument("--steps", type=int, default=0, help="run exactly this many steps instead of running for --duration")
    ap.add_argument("--docker", action="store_true")
    ap.add_argument("--ollama", action="store_true")
    ap.add_argument("--no-lane", action="store_true")
    ap.add_argument("--lane-out", default=None, help="write the lane record here (for the metrics.processes hook)")
    ap.add_argument("--outside-fetch", default="https://example.com/")
    a = ap.parse_args()
    global TRUTH
    if a.truth:
        TRUTH = os.path.abspath(a.truth)
        open(TRUTH, "w").close()
        os.environ["S8_TRUTH"] = TRUTH
    rng = random.Random(a.seed)
    wd = os.path.join(HERE, "..", "work")
    os.makedirs(wd, exist_ok=True)
    lane = {}
    if not a.no_lane:
        lane = lane_up(wd, a.docker, 0)
        if a.lane_out:
            json.dump(lane, open(a.lane_out, "w"))
    port = lane.get("port", 0)
    events = []
    try:
        if a.outside_fetch:
            t = time.time()
            try:
                n = len(urllib.request.urlopen(a.outside_fetch, timeout=10).read())
                events.append(("outside_fetch_bytes", n, round(time.time() - t, 3)))
            except Exception as e:  # offline is fine; the record says so
                events.append(("outside_fetch_failed", str(e)[:80], 0))
        ollama_thread = None
        if a.ollama:
            ollama_thread = threading.Thread(target=lambda: ollama_generate(300))
            ollama_thread.start()
        end = time.time() + a.duration
        order = ["build", "tsc", "preflight", "suite", "preflight", "suite", "tsc"]
        i = 0
        while (i < a.steps) if a.steps else (time.time() < end):
            kind = order[i % len(order)]
            i += 1
            # launcher -> sh -> python step -> children: three levels, as npm -> node -> workers
            run(["/bin/sh", "-c", 'exec "$0" "$@"', sys.executable, os.path.abspath(__file__), "step", kind,
                 str(rng.randint(1, 10**9)), str(port)])
        if ollama_thread:
            ollama_thread.join()
    finally:
        lane_down(lane)
    print(json.dumps({"steps": i, "events": events, "lane": {k: v for k, v in lane.items() if k != "container"}}))


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "step":
        step_main()
    else:
        main()
