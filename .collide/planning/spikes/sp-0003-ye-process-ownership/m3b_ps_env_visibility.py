"""Which same-user processes does the `ps -E` marker scan see?

Starts common binaries with COLLIDE_RUN=<id>, then checks (1) whether the
marker scan finds each and (2) what the kernel's KERN_PROCARGS2 (the sysctl
`ps -E` reads) returns for it. Apple platform binaries return argv only.
Also checks a process that deletes COLLIDE_RUN in-process without exec.

  python3 m3b_ps_env_visibility.py
"""
import ctypes
import os
import shutil
import struct
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "lib"))
import s3  # noqa: E402

libc = ctypes.CDLL(None, use_errno=True)


def procargs2(pid):
    mib = (ctypes.c_int * 3)(1, 49, pid)  # CTL_KERN, KERN_PROCARGS2
    size = ctypes.c_size_t(1 << 20)
    buf = ctypes.create_string_buffer(size.value)
    if libc.sysctl(mib, 3, buf, ctypes.byref(size), None, 0) != 0:
        return "sysctl errno %d" % ctypes.get_errno()
    raw = buf.raw[:size.value]
    argc = struct.unpack("i", raw[:4])[0]
    strings = [p for p in raw[4:].split(b"\0") if p]
    env = [p for p in strings[1 + argc:] if b"=" in p]
    return "argc=%d env_strings=%d" % (argc, len(env))


def platform(path):
    out = subprocess.run(["codesign", "-dv", path], capture_output=True, text=True).stderr
    return "platform" if "Platform identifier" in out else "-"


def main():
    run_id = s3.new_run_id("m3b")
    env = dict(os.environ, COLLIDE_RUN=run_id)
    node = shutil.which("node")
    cands = [
        ["/bin/sleep", "30"], ["/bin/sh", "-c", "sleep 30; :"], ["/bin/bash", "-c", "sleep 30; :"],
        ["/bin/zsh", "-c", "sleep 30; :"], ["/usr/bin/tail", "-f", "/dev/null"], ["/usr/bin/caffeinate", "-t", "30"],
        ["/usr/bin/perl", "-e", "sleep 30"], ["/usr/bin/env", "sleep", "30"],
        [s3.PY, "-c", "import time; time.sleep(30)"],
        [s3.PY, "-c", "import os,time; os.environ.pop('COLLIDE_RUN'); time.sleep(30)"],
    ]
    if node:
        cands.append([os.path.realpath(node), "-e", "setTimeout(()=>{},30000)"])
    procs = [(c, subprocess.Popen(c, env=env)) for c in cands]
    time.sleep(1.0)
    found = s3.marker_scan(run_id)
    s3.say("%-58s %-9s %-6s %s" % ("command", "signing", "found", "KERN_PROCARGS2"))
    for c, p in procs:
        # Sign-check the running image: /usr/bin/python3 is a platform stub that
        # execs the Command Line Tools Python, which is not a platform binary.
        image = subprocess.run(["ps", "-o", "comm=", "-p", str(p.pid)], capture_output=True, text=True).stdout.strip()
        if image and not image.startswith("/"):
            image = shutil.which(image) or image
        s3.say("%-58s %-9s %-6s %s" % (" ".join(c)[:58], platform(image or c[0]), p.pid in found, procargs2(p.pid)))
    for _, p in procs:
        p.kill()
        p.wait()


if __name__ == "__main__":
    main()
