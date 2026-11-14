"""Collect numbers for what a `metrics.processes` hook names: Docker API stats and libproc rusage for detached pids."""
import http.client
import json
import os
import socket
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import libproc  # noqa: E402


class UnixHTTP(http.client.HTTPConnection):
    def __init__(self, path, timeout=30):
        super().__init__("localhost", timeout=timeout)
        self._path = path

    def connect(self):
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.sock.settimeout(self.timeout)
        self.sock.connect(self._path)


def docker_socket():
    out = subprocess.run(["docker", "context", "inspect", "--format", "{{.Endpoints.docker.Host}}"], capture_output=True, text=True).stdout.strip()
    return out[len("unix://"):] if out.startswith("unix://") else out


def docker_stats(sock, cid, one_shot=True):
    """GET /containers/<id>/stats. one_shot returns at once without the previous-CPU fields; otherwise it waits for a second reading."""
    c = UnixHTTP(sock)
    t0 = time.time()
    c.request("GET", "/containers/%s/stats?stream=false&one-shot=%s" % (cid, "true" if one_shot else "false"))
    body = c.getresponse().read()
    wall = time.time() - t0
    c.close()
    return json.loads(body), wall, len(body)


def flatten(st):
    """The numbers a Collide sample keeps from a stats document. A field the API did not give is None, never 0."""
    cpu = (st.get("cpu_stats") or {}).get("cpu_usage") or {}
    mem = st.get("memory_stats") or {}
    io = ((st.get("blkio_stats") or {}).get("io_service_bytes_recursive")) or None
    rd = wr = None
    if io:
        rd = sum(e["value"] for e in io if e.get("op", "").lower() == "read")
        wr = sum(e["value"] for e in io if e.get("op", "").lower() == "write")
    nets = st.get("networks") or None
    return {
        "read": st.get("read"),
        "cpu_total_ns": cpu.get("total_usage"),
        "mem_usage": mem.get("usage"),
        "blk_read": rd, "blk_write": wr,
        "net_rx": sum(n["rx_bytes"] for n in nets.values()) if nets else None,
        "net_tx": sum(n["tx_bytes"] for n in nets.values()) if nets else None,
        "pids": (st.get("pids_stats") or {}).get("current"),
    }


def pid_stats(pid):
    ri = libproc.rusage(pid)
    if ri is None:
        return None
    u, s = libproc.cpu_ns(ri)
    return {"cpu_ns": u + s, "footprint": ri.ri_phys_footprint, "disk_read": ri.ri_diskio_bytesread, "disk_write": ri.ri_diskio_byteswritten}
