"""Final survivor check for this spike.

Lists any process whose environment carries a COLLIDE_RUN=s3-* marker, any
container labelled collide.spike=1, any still-running claude --bg session
whose id is in work/bg_sessions.txt, and the claude daemon's state.
  python3 check_clean.py
"""
import json
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "lib"))
import s3  # noqa: E402

# The shell that runs this check may carry the pattern in its argv; skip ancestors.
table = s3.ps_table()
ancestors, pid = set(), os.getpid()
while pid in table and pid not in ancestors and pid > 1:
    ancestors.add(pid)
    pid = table[pid]["ppid"]
out = subprocess.run(["ps", "-axE", "-ww", "-o", "pid=,command="], capture_output=True, text=True).stdout
marked = [l.strip()[:120] for l in out.splitlines()
          if re.search(r"(?:^|\s)COLLIDE_RUN=s3-", l) and int(l.split()[0]) not in ancestors]
print("processes with COLLIDE_RUN=s3-*: %d" % len(marked))
for l in marked:
    print("  " + l)
containers = subprocess.run(["docker", "ps", "-a", "--filter", "label=" + s3.SPIKE_LABEL,
                             "--format", "{{.ID}} {{.Status}} {{.Labels}}"], capture_output=True, text=True).stdout
print("containers with label %s: %s" % (s3.SPIKE_LABEL, containers.strip() or "none"))
ids = set()
path = os.path.join(s3.WORK, "bg_sessions.txt")
if os.path.exists(path):
    ids = {l.strip() for l in open(path) if l.strip()}
rows = json.loads(s3.run_claude(["agents", "--json", "--all"]).stdout or "[]")
running = [r["id"] for r in rows if r.get("id") in ids and r.get("pid")]
listed = [r["id"] for r in rows if r.get("id") in ids]
print("claude --bg sessions from this spike: %d known, %d still listed, %d running %s"
      % (len(ids), len(listed), len(running), running))
print("claude daemon: " + " ".join(s3.run_claude(["daemon", "status"]).stdout.split("\n")[:1]))
