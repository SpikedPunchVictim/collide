#!/bin/sh
# Owner step for S8's sleep detection (plan 0.13, "Sleep is not work"). Takes about two minutes.
#   1. Run:   ./sleep-check.sh        (the watcher starts and prints "watching")
#   2. Put the Mac to sleep by hand (Apple menu, Sleep, or close the lid) and note the time.
#   3. Wait about 60 s, then wake it.
#   4. Read the result the watcher prints: "slept N s", and the OS's own record to compare with.
# Nothing here puts the machine to sleep. The watcher gives up after 15 minutes if it sees no sleep.
here=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$here/out"
python3 "$here/lib/sleepclock.py" watch --max-s "${MAX_S:-900}" | tee "$here/out/sleep-check.txt"
