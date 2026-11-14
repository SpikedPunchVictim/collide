#!/usr/bin/env python3
"""Prototype of a pack's `metrics.processes` hook (plan 0.13, "Containers and detached processes").

Collide calls it with the workspace's lane record and reads one JSON object from stdout:
  {"v": 1,
   "containers": [{"id": "<container id>", "role": "database"}],
   "processes":  [{"pid": 123, "start_usec": <optional start time, to survive pid reuse>, "role": "server"}]}
A hook never reports numbers, only identities; Collide samples them itself (container stats through the Docker API,
processes through libproc). An empty list means "none now", a missing key means "this hook does not know".
Usage: metrics-processes.py <lane.json>
"""
import json
import os
import sys


def main():
    lane = json.load(open(sys.argv[1]))
    out = {"v": 1, "containers": [], "processes": []}
    if lane.get("container"):
        out["containers"].append({"id": lane["container"], "role": "database-or-service"})
    if lane.get("serverPid"):
        out["processes"].append({"pid": lane["serverPid"], "role": "server"})
    if lane.get("uiPid"):
        out["processes"].append({"pid": lane["uiPid"], "role": "ui"})
    json.dump(out, sys.stdout)
    sys.stdout.write("\n")


if __name__ == "__main__":
    main()
