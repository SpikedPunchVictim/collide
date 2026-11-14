#!/bin/sh
# Attach S8's sampler to a running process tree, beside a real Orchard iteration run or lane test. Read-only: it starts two
# samplers (1 s, the candidate; 50 ms, the reference for what 1 s misses) and one helper (network, GPU, sleep, Docker),
# and nothing else. It never signals a process it did not start. Ctrl-C ends it early; it also ends when the root exits.
#
#   usage: measure-attach.sh ROOT_PID [DOCKER_FILTER]
#     ROOT_PID       the process whose tree to measure (the shell or `iter` process of the run; start the run first,
#                    then `pgrep -f` or `ps` to find its pid)
#     DOCKER_FILTER  optional, passed to `docker ps --filter`, e.g. label=some.label=value, to include lane containers
#   Output: out/attach-<time>/ with samples-1000.jsonl, samples-50.jsonl, extras.json, and report.txt (printed at the end).
#   Cost to expect: the 50 ms sampler costs about 1.5% of one core; the 1 s sampler about 0.1%.
set -eu
[ $# -ge 1 ] || { sed -n '2,11p' "$0"; exit 2; }
root=$1
filter=${2:-}
here=$(cd "$(dirname "$0")" && pwd)
sampler="$here/sampler/target/release/s8-sampler"
if [ ! -x "$sampler" ]; then
  PATH="$HOME/.cargo/bin:$PATH"
  (cd "$here/sampler" && cargo build --release)
fi
kill -0 "$root" 2>/dev/null || { echo "pid $root is not running" >&2; exit 1; }
out="$here/out/attach-$(date +%Y%m%dT%H%M%S)"
mkdir -p "$out"
echo "measuring the tree under pid $root; output in $out; Ctrl-C to stop"
"$sampler" --root "$root" --interval-ms 1000 --out "$out/samples-1000.jsonl" > "$out/sampler-1000.summary" &
p1=$!
"$sampler" --root "$root" --interval-ms 50 --out "$out/samples-50.jsonl" > "$out/sampler-50.summary" &
p2=$!
if [ -n "$filter" ]; then
  python3 "$here/lib/attach_extras.py" --root "$root" --out "$out" --docker-filter "$filter" &
else
  python3 "$here/lib/attach_extras.py" --root "$root" --out "$out" &
fi
p3=$!
trap 'kill -INT $p1 $p2 $p3 2>/dev/null || true' INT TERM
wait $p1 $p2 $p3 || true
python3 "$here/attach_report.py" "$out" | tee "$out/report.txt"
