#!/usr/bin/env bash
# Q1: Rust -> JSON Schema -> TypeScript, against ts-rs and specta. Run from the spike directory.
set -euo pipefail
cd "$(dirname "$0")/.."
CARGO=${CARGO:-$HOME/.cargo/bin/cargo}
NODE=${NODE:-/opt/homebrew/bin/node}
OUT=work/q1-out
rm -rf "$OUT"; mkdir -p "$OUT"

echo "== Q1 cold builds (fresh target dir each; lib + emit + verdict bins)"
for f in none schemars ts specta; do
  feat=$f; [ "$f" = none ] && feat=""
  T=work/target-q1-$f; rm -rf "$T"
  CARGO_TARGET_DIR=$T $NODE scripts/timeit.mjs "cargo build -p s5-schema --features '$feat'" "$CARGO" build -p s5-schema --features "$feat"
  du -sk "$T" | awk -v f="$f" '{printf "   target dir for %s: %.0f MB\n", f, $1/1024}'
  rm -rf "$T"
done

echo; echo "== Q1 emit (shared target dir)"
"$CARGO" build -q -p s5-schema --features schemars,ts,specta
"$CARGO" run -q -p s5-schema --features schemars,ts,specta --bin emit -- "$OUT" 2> "$OUT/emit-warnings.txt" || { cat "$OUT/emit-warnings.txt"; exit 1; }
"$CARGO" run -q -p s5-schema --features ts-fix --bin emit -- "$OUT" 2>> "$OUT/emit-warnings.txt"
echo "ts-rs warnings printed during the build (count of lines, distinct):"; grep -c . "$OUT/emit-warnings.txt" || true
grep -E "^warning" "$OUT/emit-warnings.txt" | sort | uniq -c || true
"$CARGO" run -q -p s5-schema --bin verdict -- fixtures/q1-cases.json > "$OUT/verdict.json"

echo; echo "== Q1 edit-rebuild loop: touch lib.rs, rebuild and regenerate"
touch q1-schema/src/lib.rs
$NODE scripts/timeit.mjs "schemars: build + emit schema.json"  "$CARGO" run -q -p s5-schema --features schemars --bin emit -- work/q1-tmp
touch q1-schema/src/lib.rs
$NODE scripts/timeit.mjs "ts-rs: build + emit .ts"            "$CARGO" run -q -p s5-schema --features ts --bin emit -- work/q1-tmp
touch q1-schema/src/lib.rs
$NODE scripts/timeit.mjs "specta: build + emit .ts"           "$CARGO" run -q -p s5-schema --features specta --bin emit -- work/q1-tmp
rm -rf work/q1-tmp
# rebuild the full-feature artifacts so the comparison below reads the final state
"$CARGO" run -q -p s5-schema --features schemars,ts,specta --bin emit -- "$OUT" 2>/dev/null
"$CARGO" run -q -p s5-schema --features ts-fix --bin emit -- "$OUT" 2>/dev/null

echo; echo "== Q1 comparison"
$NODE scripts/q1-compare.mjs
