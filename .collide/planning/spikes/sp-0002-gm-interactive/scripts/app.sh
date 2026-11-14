#!/bin/sh
# Helpers for the Tauri app: launch it (activated), quit it with the AppKit quit event, find its pid.
APP="$(cd "$(dirname "$0")/.." && pwd)/target/debug/bundle/macos/S2Spike.app"
case "$1" in
  launch) open -n "$APP"; sleep 1.5; osascript -e 'tell application "S2Spike" to activate' >/dev/null 2>&1 ;;
  pid) pgrep -f 'S2Spike.app/Contents/MacOS/s2-spike-app' | head -1 ;;
  quit) osascript -e 'tell application "S2Spike" to quit' ;;
esac
