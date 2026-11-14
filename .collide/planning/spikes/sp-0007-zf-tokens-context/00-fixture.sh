#!/bin/sh
# Scratch repos under work/ (ignored by git). idle/ has ledger1-4.js, q4/ has ledger1-8.js, empty/ has nothing.
# Each ledger file is 120 generated functions (about 17KB, about 11K tokens). No real repo is touched.
set -eu
W="$(cd "$(dirname "$0")" && pwd)/work"
mkdir -p "$W/idle" "$W/q4" "$W/empty"
for d in idle q4 empty; do [ -d "$W/$d/.git" ] || git -C "$W/$d" init -q .; done
node -e '
const fs=require("fs"); const W=process.argv[1];
for (let f=1;f<=8;f++){ let s=`// module ${f}: inventory arithmetic helpers\n`; for(let i=0;i<120;i++){ s+=`export function calc${f}_${i}(a, b) {\n  // step ${i}: combine a and b for ledger ${f}\n  const t = (a * ${i+2}) + (b % ${f+3});\n  return t > ${i*7} ? t - ${i} : t + ${f};\n}\n`;}
  if (f<=4) fs.writeFileSync(`${W}/idle/ledger${f}.js`,s); fs.writeFileSync(`${W}/q4/ledger${f}.js`,s);}
' "$W"
sed "s#S9ROOT#$(cd "$(dirname "$0")" && pwd)#" > "$W/mcp-big.json" <<'EOT'
{"mcpServers":{"s9big":{"command":"node","args":["S9ROOT/mcp-big.mjs"]}}}
EOT
echo ok
