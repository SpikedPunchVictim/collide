"""nettop -P per-process network bytes (no root). The header decides the columns: the layout depends on option order
(with `-x -J ... -P -L n` a `time` column comes first, with `-P -L n -x -J ...` it does not)."""
import os
import subprocess
import time

COLS = "bytes_in,bytes_out"
ARGS = ["-x", "-J", COLS, "-P"]  # this order gives the `time` column; the parser does not depend on it


def parse(text):
    """Returns {pid: (name, bytes_in, bytes_out)} for the last block in text (a block starts at each header line)."""
    out, idx, name_col = {}, None, None
    for line in text.splitlines():
        f = line.split(",")
        if "bytes_in" in f:
            idx = f.index("bytes_in")
            name_col = idx - 1
            out = {}
            continue
        if idx is None or len(f) <= idx + 1 or "." not in f[name_col]:
            continue
        name, pid = f[name_col].rsplit(".", 1)
        if pid.isdigit():
            out[int(pid)] = (name, int(f[idx] or 0), int(f[idx + 1] or 0))
    return out


def snapshot():
    """One `nettop -L 1` run: (rows, wall_s, cpu_s of nettop, parse_s, bytes of CSV)."""
    t0 = time.time()
    p = subprocess.Popen(["nettop"] + ARGS + ["-L", "1"], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
    text = p.stdout.read()
    _, _, ru = os.wait4(p.pid, 0)
    wall = time.time() - t0
    t1 = time.perf_counter()
    rows = parse(text)
    return rows, wall, ru.ru_utime + ru.ru_stime, time.perf_counter() - t1, len(text)
