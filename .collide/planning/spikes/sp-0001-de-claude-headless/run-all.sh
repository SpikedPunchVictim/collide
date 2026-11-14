#!/bin/sh
# Runs every S1 script in order from a clean state and writes each one's output to
# out/<script>.txt, then prints the spend per script. Scripts run one at a time: 00-fixture.sh
# deletes work/, which the others use.
set -u
S1="$(cd "$(dirname "$0")" && pwd)"
cd "$S1"
rm -rf out work
mkdir -p out
for s in 0*.sh; do
  name=$(basename "$s" .sh)
  start=$(date +%s)
  ./"$s" >"out/$name.txt" 2>&1
  rc=$?
  echo "$name exit=$rc seconds=$(( $(date +%s) - start ))"
done

echo
echo "## spend per script"
./spend.sh | tee out/spend.txt
