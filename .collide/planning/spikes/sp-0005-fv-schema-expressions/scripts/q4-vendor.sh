#!/usr/bin/env bash
# Copies the registry source of cel 0.15.0 to work/vendor/cel-patched and applies q4-cel/patches/cel-step-limit.patch.
set -euo pipefail
cd "$(dirname "$0")/.."
CARGO=${CARGO:-$HOME/.cargo/bin/cargo}
"$CARGO" fetch -q
SRC=$(ls -d "$HOME"/.cargo/registry/src/*/cel-0.15.0 | head -1)
rm -rf work/vendor; mkdir -p work/vendor
cp -R "$SRC" work/vendor/cel-orig
cp -R "$SRC" work/vendor/cel-patched
patch -s -p1 -d work/vendor/cel-patched < q4-cel/patches/cel-step-limit.patch
echo "patched copy at work/vendor/cel-patched"
