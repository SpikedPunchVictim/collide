#!/usr/bin/env python3
"""S8 measurement 5: GPU. Per-process GPU time (AGXDeviceUserClient) and device utilization (IOAccelerator) via ioreg,
read once a second while a host-run Ollama generates. No root. Python stdlib only."""
import json
import os
import sys
import threading
import time
import urllib.request

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "lib"))
from gpu import device_stats, gpu_clients, ioreg  # noqa: E402

OLLAMA = "http://127.0.0.1:11434"
# The model default is a 131072-token context, whose KV cache made the loaded model 23 GB on this 16 GB machine and the
# first attempt hang for 15 minutes. A small context keeps the load to about 5 GB; GPU time does not depend on it.
NUM_CTX = 2048


def preload():
    """Load the model (no prompt) and wait for it: under memory pressure a cold load of the 8B model can take minutes."""
    t0 = time.time()
    body = json.dumps({"model": "llama3.1:8b", "keep_alive": "10m", "options": {"num_ctx": NUM_CTX}}).encode()
    urllib.request.urlopen(urllib.request.Request(OLLAMA + "/api/generate", data=body, method="POST"), timeout=900).read()
    return time.time() - t0


def generate(result):
    body = json.dumps({"model": "llama3.1:8b", "prompt": "Explain in four sentences how incremental builds work.", "stream": False,
                       "options": {"num_predict": 350, "temperature": 0, "num_ctx": NUM_CTX}}).encode()
    t0 = time.time()
    r = json.loads(urllib.request.urlopen(urllib.request.Request(OLLAMA + "/api/generate", data=body, method="POST"), timeout=600).read())
    result.update({k: r.get(k) for k in ("total_duration", "load_duration", "prompt_eval_duration", "eval_duration", "eval_count")})
    result["wall_s"] = time.time() - t0


def main():
    try:
        urllib.request.urlopen(OLLAMA + "/api/ps", timeout=3).read()
    except Exception as e:
        print("Ollama is not reachable at %s (%s): GPU run skipped [U]" % (OLLAMA, e))
        return
    print("preloading llama3.1:8b (a cold load can take minutes on a busy machine)...", flush=True)
    print("model loaded in %.1f s [M]" % preload(), flush=True)
    idle, w, c, pt, size = gpu_clients()
    print("ioreg -r -c AGXDeviceUserClient -a: %d bytes, %d processes with a GPU client; wall %.0f ms, ioreg CPU %.0f ms, plist parse %.0f ms [M]" % (
        size, len(idle), 1000 * w, 1000 * c, 1000 * pt))
    dev, w2, c2, pt2, size2 = device_stats()
    print("ioreg -r -d 1 -c IOAccelerator -a: %d bytes; wall %.0f ms, CPU %.0f ms, parse %.0f ms; fields %s [M]" % (size2, 1000 * w2, 1000 * c2, 1000 * pt2, dev))

    result = {}
    th = threading.Thread(target=generate, args=(result,))
    prev, prev_t = idle, time.time()
    rows = []
    print("\nsecond  device%%  top processes by GPU time this second (pid name: share of one second)")
    for i in range(6):  # idle baseline
        time.sleep(1)
        cur, *_ = gpu_clients()
        d, *_ = device_stats()
        t = time.time()
        rows.append((t, d.get("Device Utilization %"), {p: (cur[p]["gpu_ns"] - prev.get(p, {"gpu_ns": cur[p]["gpu_ns"]})["gpu_ns"]) / 1e9 / (t - prev_t) for p in cur}, cur))
        prev, prev_t = cur, t
        top = sorted(rows[-1][2].items(), key=lambda kv: -kv[1])[:2]
        print("idle %2d  %5s  %s" % (i, d.get("Device Utilization %"), ", ".join("%d %s: %.2f" % (p, cur[p]["name"], v) for p, v in top)))
    th.start()
    gen_rows = []
    while th.is_alive():
        time.sleep(1)
        cur, *_ = gpu_clients()
        d, *_ = device_stats()
        t = time.time()
        shares = {p: (cur[p]["gpu_ns"] - prev.get(p, {"gpu_ns": cur[p]["gpu_ns"]})["gpu_ns"]) / 1e9 / (t - prev_t) for p in cur}
        gen_rows.append((d.get("Device Utilization %"), shares, cur))
        prev, prev_t = cur, t
        top = sorted(shares.items(), key=lambda kv: -kv[1])[:2]
        print("gen %3d  %5s  %s" % (len(gen_rows), d.get("Device Utilization %"), ", ".join("%d %s: %.2f" % (p, cur[p]["name"], v) for p, v in top)))
    th.join()
    # the process that used the most GPU time during the request (the model runner, not the Ollama app that started it)
    first = rows[-1][3]
    deltas = {p: v["gpu_ns"] - first.get(p, {"gpu_ns": v["gpu_ns"]})["gpu_ns"] for p, v in gen_rows[-1][2].items()}
    runner = max(deltas, key=deltas.get) if deltas else None
    print("\nOllama: eval %s tokens in %.2f s (%.1f tok/s), load %.2f s, total %.2f s" % (
        result.get("eval_count"), (result.get("eval_duration") or 0) / 1e9,
        (result.get("eval_count") or 0) / max(1e-9, (result.get("eval_duration") or 1) / 1e9), (result.get("load_duration") or 0) / 1e9,
        (result.get("total_duration") or 0) / 1e9))
    if runner is not None:
        before = rows[-1][3].get(runner, {"gpu_ns": 0})["gpu_ns"]
        after = gen_rows[-1][2][runner]["gpu_ns"]
        busy = [r[1].get(runner, 0) for r in gen_rows]
        print("Ollama runner pid %d (%s): GPU time before generation %.2f s, after %.2f s; delta %.2f s over a %.2f s request [M]" % (
            runner, gen_rows[-1][2][runner]["name"], before / 1e9, after / 1e9, (after - before) / 1e9, result["wall_s"]))
        print("   per-second GPU share while generating: %s (sum %.2f s)" % (", ".join("%.2f" % b for b in busy), sum(busy)))
        print("   device utilization %% while generating: %s; ollama's eval_duration was %.2f s" % ([r[0] for r in gen_rows], (result.get("eval_duration") or 0) / 1e9))
    else:
        print("no GPU client gained GPU time during the request [M]")
    print("note: the counter is cumulative GPU busy time in ns per client; a process that exits takes its counter with it, so a delta"
          " needs two samples while it lives.")


if __name__ == "__main__":
    main()
