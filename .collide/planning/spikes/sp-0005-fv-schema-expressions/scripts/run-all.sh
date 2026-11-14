#!/usr/bin/env bash
# One command that re-runs every S5 measurement and prints the numbers. Run from anywhere.
#   scripts/run-all.sh                 about 20-25 minutes on an Apple-silicon laptop
#   S5_FULL=1 scripts/run-all.sh       also runs the 4-character exhaustive id sweeps in Q3 (about 4 more minutes)
#   S5_SKIP_NET=1 scripts/run-all.sh   skips the crates.io metadata fetch
# Each section is also saved under work/results/. Everything stays inside this directory.
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="/opt/homebrew/bin:$PATH"
CARGO=${CARGO:-$HOME/.cargo/bin/cargo}
NODE=${NODE:-/opt/homebrew/bin/node}
R=work/results
mkdir -p "$R" work/q3 work/q4
section() { echo; echo "######## $1"; }

section "environment"
{ "$CARGO" --version; rustc --version 2>/dev/null || "$HOME/.cargo/bin/rustc" --version; "$NODE" --version; pnpm --version; uname -sm; uptime; } | tee "$R/env.txt"

section "npm dependencies (spike-local store, frozen lockfile)"
pnpm install --frozen-lockfile 2>&1 | tail -3

if [ -z "${S5_SKIP_NET:-}" ]; then
  section "crate maintenance (crates.io API; data only)"
  $NODE scripts/crate-status.mjs serde_yaml serde_yaml_ng serde_norway serde_yml serde-saphyr saphyr granit-parser yaml-rust2 unsafe-libyaml \
    schemars ts-rs specta specta-typescript cel cel-interpreter cel-core cellang rscel ferricel cel-cxx 2>&1 | tee "$R/crate-status.txt"
fi

section "Q1: Rust -> JSON Schema -> TypeScript (schemars, ts-rs, specta)"
scripts/q1.sh 2>&1 | tee "$R/q1.txt"

section "Q2: Rust YAML crates"
"$CARGO" build --release -q -p s5-yaml
work/target/release/s5-yaml all 2>&1 | tee "$R/q2.txt"

section "Q3: YAML in TypeScript (yaml, js-yaml, gray-matter) and the id alphabet"
$NODE scripts/q3-yaml-ts.mjs 2>&1 | tee "$R/q3.txt"

section "Q4: CEL (cel 0.15.0), stock and with the step-counter patch"
scripts/q4-vendor.sh
"$CARGO" build --release -q -p s5-cel
(cd q4-cel-patched && "$CARGO" build --release -q)
for flavor in cel-bench cel-bench-patched; do
  for sub in ordinary "estimate 200" limits scaling hostile threads parse; do
    case "$flavor/$sub" in
      cel-bench-patched/scaling|cel-bench-patched/threads|cel-bench-patched/parse) continue ;;  # same numbers as stock apart from counter overhead
    esac
    section "Q4 $flavor $sub"
    work/target/release/$flavor $sub 2>&1 | tee "$R/q4-$flavor-${sub%% *}.txt"
  done
done
section "done; saved under $R/"
