#!/bin/sh
# S7 live run: drive `opencode acp` with one local Ollama model. Needs a running Ollama server and qwen2.5:7b pulled.
# State lives under work/ only. Output: out/live-run.jsonl and out/live-run.txt.
set -eu
cd "$(dirname "$0")"
mkdir -p work/config/opencode work/data work/state work/cache work/home work/scratch out
cp opencode.json work/config/opencode/opencode.json
rm -rf work/data/opencode work/state/opencode work/cache/opencode work/state.json
rm -rf work/scratch && mkdir -p work/scratch
node driver.mjs "$@" 2>&1 | tee out/live-run.txt
