#!/bin/bash
# Runs the S9 scripts from a clean state. 06a must come first: 06b needs 61 minutes of idle time after it,
# and 06-run-pair-when-ready.sh waits for that, so the whole run takes a little over an hour.
set -u
cd "$(dirname "$0")"
mkdir -p work out
[ -d work/idle ] || ./00-fixture.sh
./06a-idle-start.sh > out/06a-idle-start.txt 2>&1
./06c-big-idle-start.sh > out/06c-big-idle-start.txt 2>&1
for s in 01-usage-compact 03-start-weight 03b-context-usage 04-env 04b-window-clamp 05-hook-block; do ./$s.sh > out/$s.txt 2>&1; done
./cache-tier.sh > out/cache-tier.txt 2>&1
node usage-aggregate.mjs > out/usage-aggregate.txt 2>&1
node result-usage-check.mjs > out/result-usage-check.txt 2>&1
./06-run-pair-when-ready.sh > out/06-pair.txt 2>&1
./06-run-big-pair-when-ready.sh > out/06-big-pair.txt 2>&1
./spend.sh > out/spend.txt
