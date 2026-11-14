#!/usr/bin/env python3
"""S8 measurement 7: store growth. Writes sampler recordings into SQLite the way a store would (one transaction per tick,
a process table plus a sample table) and measures bytes per sample row and the file growth per hour.

  07-store-growth.py     (reads out/samples-r1-i1000.jsonl and out/samples-r1-i250.jsonl from 01, then builds synthetic hour-long recordings)
"""
import json
import os
import random
import sqlite3
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))

SCHEMA = """
CREATE TABLE proc (id INTEGER PRIMARY KEY, pid INTEGER NOT NULL, start_ticks INTEGER NOT NULL, ppid INTEGER, comm TEXT,
                   first_ms INTEGER, UNIQUE (pid, start_ticks));
CREATE TABLE sample (proc_id INTEGER NOT NULL, t_ms INTEGER NOT NULL, cpu_us INTEGER, rss INTEGER, footprint INTEGER,
                     disk_r INTEGER, disk_w INTEGER, PRIMARY KEY (proc_id, t_ms)) WITHOUT ROWID;
"""


def replay(path, skip_unchanged, db_path, stop_after=None):
    if os.path.exists(db_path):
        os.unlink(db_path)
    db = sqlite3.connect(db_path)
    db.executescript(SCHEMA)
    ids = {}
    last = {}
    rows = ticks = skipped = 0
    t_insert = 0.0
    for line in open(path):
        if not line.startswith("{"):
            continue
        if stop_after is not None and ticks >= stop_after:
            break
        s = json.loads(line)
        t0 = time.perf_counter()
        t_ms = s["t"] // 1_000_000
        for p in s["p"]:
            pid, ppid, start, comm, user_ns, sys_ns, rss, foot, rd, wr = p[:10]
            key = (pid, start)
            if key not in ids:
                cur = db.execute("INSERT INTO proc (pid, start_ticks, ppid, comm, first_ms) VALUES (?,?,?,?,?)", (pid, start, ppid, comm, t_ms))
                ids[key] = cur.lastrowid
            vals = ((user_ns + sys_ns) // 1000, rss, foot, rd, wr)
            if skip_unchanged and last.get(key) == (vals[0], vals[3], vals[4]):
                skipped += 1
                continue
            last[key] = (vals[0], vals[3], vals[4])
            db.execute("INSERT INTO sample VALUES (?,?,?,?,?,?,?)", (ids[key], t_ms) + vals)
            rows += 1
        db.commit()
        t_insert += time.perf_counter() - t0
        ticks += 1
    db.execute("VACUUM")
    db.close()
    size = os.path.getsize(db_path)
    return {"ticks": ticks, "rows": rows, "skipped": skipped, "procs": len(ids), "bytes": size, "insert_s": t_insert}


def synth(n_procs, ticks, interval_ms, change_p):
    """A synthetic sampler recording: n_procs live processes for ticks ticks. Each tick, a process's CPU, disk counters and
    resident size change with probability change_p (the real recordings show 20-45% of rows unchanged at 250-1000 ms)."""
    rng = random.Random(7)
    state = [[1000 + i, 5000 + i, 1, "node", 0, 0, 40_000_000 + rng.randrange(1 << 24), 30_000_000 + rng.randrange(1 << 24), 0, 0] for i in range(n_procs)]
    t = 1_794_647_000_000_000_000
    for _ in range(ticks):
        t += interval_ms * 1_000_000
        for st in state:
            if rng.random() < change_p:
                st[4] += rng.randrange(1, 40) * interval_ms * 1000   # user ns
                st[5] += rng.randrange(0, 5) * interval_ms * 1000    # sys ns
                st[6] += rng.randrange(-65536, 262144)
                st[7] += rng.randrange(-65536, 262144)
                if rng.random() < 0.2:
                    st[9] += rng.randrange(1, 64) * 4096             # bytes written
        yield {"t": t, "p": [list(st) for st in state]}


def measure(n, iv, skip, ticks=3600, change_p=0.7):
    db_path = os.path.join(HERE, "work", "growth.db")
    with open(db_path + ".jsonl", "w") as f:
        for s in synth(n, ticks, iv, change_p):
            f.write(json.dumps(s) + "\n")
    half = replay(db_path + ".jsonl", skip, db_path, stop_after=ticks // 2)
    full = replay(db_path + ".jsonl", skip, db_path)
    os.unlink(db_path + ".jsonl")
    rows = full["rows"] - half["rows"]
    per_row = (full["bytes"] - half["bytes"]) / max(1, rows)
    return full, per_row


def main():
    # 1. real recordings: how many rows a change-only writer would skip (the rest of the figures use the synthetic runs below)
    for name in ("samples-r1-i1000.jsonl", "samples-r1-i250.jsonl"):
        path = os.path.join(HERE, "out", name)
        if not os.path.exists(path):
            print("missing %s: run 01-overhead-miss.py first [U]" % name)
            continue
        a = replay(path, False, os.path.join(HERE, "work", "growth.db"))
        b = replay(path, True, os.path.join(HERE, "work", "growth.db"))
        print("%s: %d rows, %d (%.0f%%) unchanged since the previous tick (CPU and disk counters equal) [M]" % (
            name, a["rows"], b["skipped"], 100.0 * b["skipped"] / a["rows"]))
    # 2. synthetic one-hour recordings; bytes per row from the growth over the second half (page rounding and the process table cancel)
    print("\nsynthetic recordings, 3600 ticks (1 hour at 1000 ms, 15 minutes at 250 ms), SQLite file after VACUUM; per-row figure = growth over the second half [M]")
    print("%-28s %5s %9s %11s %9s %13s %10s" % ("writer", "procs", "interval", "bytes/row", "rows/h", "MB per hour", "CPU/tick"))
    for iv in (1000, 250):
        for n in (5, 20, 50):
            for skip in (False, True):
                full, per_row = measure(n, iv, skip)
                rows_h = full["rows"] / (3600 * iv / 1000.0 / 3600.0) / (3600 / (3600 * iv / 1000.0)) if False else full["rows"] * (3600000.0 / (3600 * iv))
                print("%-28s %5d %7d ms %11.1f %9.0f %13.2f %7.2f ms" % ("skip unchanged" if skip else "every process every tick", n, iv, per_row,
                                                                      rows_h, rows_h * per_row / 1e6, 1000 * full["insert_s"] / full["ticks"]))
    print("\nrows/h = rows written in the recording scaled to one hour. The synthetic change rate is 70% per tick, so the skip-unchanged rows save about 30%.")


if __name__ == "__main__":
    main()
