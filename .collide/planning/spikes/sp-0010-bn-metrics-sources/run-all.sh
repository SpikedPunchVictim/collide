#!/bin/sh
# Runs every S8 measurement from a clean checkout of this folder and prints the results (also saved in out/).
# Needs: python3 (stdlib only), cargo (for the sampler; found in ~/.cargo/bin), node, docker (running), and for two parts
# an Ollama server with llama3.1:8b and a logged-in `claude`. A part whose tool is missing says so and is skipped.
#   S8_SKIP_TELEMETRY=1   skip the three small `claude -p` runs (they cost a few cents)
#   S8_SKIP_GPU=1         skip the Ollama generation
# Takes about 12 minutes, most of it the workload runs. It starts one container (unique name, removed again) and a few
# servers, all stopped at the end. It does not touch ~/.claude, the Orchard repo, or system settings.
here=$(cd "$(dirname "$0")" && pwd)
cd "$here"
mkdir -p out work
PATH="$HOME/.cargo/bin:$PATH"
run() { # name, command...
  name=$1; shift
  echo; echo "################ $name"
  "$@" 2>&1 | tee "out/$name.txt"
}
(cd sampler && cargo build --release 2>&1 | tail -2)
run 01-overhead-miss python3 -u 01-overhead-miss.py
run 02-diskio python3 -u 02-diskio.py
run 03-nettop python3 -u 03-nettop.py
if [ -z "${S8_SKIP_GPU:-}" ]; then run 04-gpu python3 -u 04-gpu.py; fi
run 05-docker-detached python3 -u 05-docker-detached.py
run 06-sleep python3 -u lib/sleepclock.py selftest
run 07-store-growth python3 -u 07-store-growth.py
run 08-report-tail python3 -u 08-report-tail.py
if [ -z "${S8_SKIP_TELEMETRY:-}" ]; then
  run 09-telemetry-run python3 -u telemetry/run_sessions.py
fi
if [ -d out/telemetry ]; then run 09-telemetry-analysis python3 -u telemetry/analyze.py; fi
echo; echo "done; results in out/"
