"""GPU readings through ioreg (no root): per-process GPU time and device utilization."""
import os
import plistlib
import re
import subprocess
import time


def ioreg(args):
    t0 = time.time()
    p = subprocess.Popen(["ioreg"] + args + ["-a"], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    data = p.stdout.read()
    _, _, ru = os.wait4(p.pid, 0)
    t1 = time.perf_counter()
    items = plistlib.loads(data) if data else []
    return items, time.time() - t0, ru.ru_utime + ru.ru_stime, time.perf_counter() - t1, len(data)


def gpu_clients():
    """{pid: {name, gpu_ns, clients}} summed over each process's user clients (one per Metal device connection)."""
    items, wall, cpu, parse, size = ioreg(["-r", "-c", "AGXDeviceUserClient"])
    out = {}
    for it in items:
        m = re.match(r"pid (\d+), (.*)", it.get("IOUserClientCreator", ""))
        if not m:
            continue
        pid = int(m.group(1))
        ns = sum(u.get("accumulatedGPUTime", 0) for u in it.get("AppUsage", []))
        e = out.setdefault(pid, {"name": m.group(2), "gpu_ns": 0, "clients": 0})
        e["gpu_ns"] += ns
        e["clients"] += 1
    return out, wall, cpu, parse, size


def device_stats():
    items, wall, cpu, parse, size = ioreg(["-r", "-d", "1", "-c", "IOAccelerator"])
    for it in items:
        ps = it.get("PerformanceStatistics")
        if ps:
            return {k: ps[k] for k in ("Device Utilization %", "Renderer Utilization %", "Tiler Utilization %", "In use system memory",
                                       "Alloc system memory") if k in ps}, wall, cpu, parse, size
    return {}, wall, cpu, parse, size
