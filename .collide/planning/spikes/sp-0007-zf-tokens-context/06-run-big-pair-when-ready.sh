#!/bin/bash
# Waits until 61 minutes (plus a margin) have passed since 06c-big-idle-start.sh finished, then runs 06d.
cd "$(dirname "$0")"
ended=$(cat out/06c-big-idle-start/ended-at)
until [ $(( $(date +%s) - ended )) -ge 3700 ]; do sleep 30; done
./06d-big-pair.sh
