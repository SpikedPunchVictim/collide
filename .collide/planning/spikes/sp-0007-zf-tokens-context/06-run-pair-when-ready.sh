#!/bin/bash
# Waits until 61 minutes (plus a margin) have passed since 06a-idle-start.sh finished, then runs 06b.
cd "$(dirname "$0")"
ended=$(cat out/06a-idle-start/ended-at)
until [ $(( $(date +%s) - ended )) -ge 3700 ]; do sleep 30; done
./06b-idle-resume.sh
