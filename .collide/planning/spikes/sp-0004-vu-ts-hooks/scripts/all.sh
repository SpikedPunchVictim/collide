#!/bin/sh
# Runs every S4 measurement in order. Each script prints its own results.
set -eu
DIR="$(cd "$(dirname "$0")" && pwd)"
for script in 01-materialize 02-link-sdk 03-run-hook 04-cold-start 05-tsc 06-finder-env 07-app-bundle; do
  printf '\n######## %s\n' "$script"
  sh "$DIR/$script.sh"
done
