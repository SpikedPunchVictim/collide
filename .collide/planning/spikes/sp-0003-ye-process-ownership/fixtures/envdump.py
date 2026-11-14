"""Write this process's COLLIDE_RUN and CLAUDE* names to a file: envdump.py <file>."""
import json
import os
import sys

with open(sys.argv[1], "w") as f:
    json.dump({"COLLIDE_RUN": os.environ.get("COLLIDE_RUN"), "pid": os.getpid(), "ppid": os.getppid(),
               "claude_vars": sorted(k for k in os.environ if k.startswith("CLAUDE"))}, f)
