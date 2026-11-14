#!/bin/sh
# Copies xterm.js and its addons from the spike-local node_modules into app/dist (no bundler).
set -eu
cd "$(dirname "$0")/.."
cp app/index.html app/app.js app/dist/
cp node_modules/@xterm/xterm/lib/xterm.js node_modules/@xterm/xterm/css/xterm.css app/dist/
cp node_modules/@xterm/addon-fit/lib/addon-fit.js node_modules/@xterm/addon-webgl/lib/addon-webgl.js app/dist/
