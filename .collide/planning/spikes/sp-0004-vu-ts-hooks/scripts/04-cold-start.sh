#!/bin/sh
# S4 item 4: hook cold start including the SDK import, against a bare `node -e 0`. Median and spread.
# Usage: scripts/04-cold-start.sh [RUNS]   (default 20 timed runs per variant, after 2 warm-up runs)
. "$(dirname "$0")/../lib/common.sh"
fresh_work 04-cold-start
RUNS="${1:-20}"

APP_SDK="$WORK/app/Resources/pack-sdk"
build_sdk "$APP_SDK"
make_pack_repo "$WORK/repo"
for link in symlink copy; do
  "$NODE" "$LIB/pack-cache.mjs" materialize --repo "$REPO" --ref main --path "$PACK_PATH" --lock "$LOCK" \
    --cache "$WORK/cache-$link" --sdk "$APP_SDK" --link "$link" >/dev/null
done
HOOK_SYMLINK="$WORK/cache-symlink/packs/$LOCK/pack/hooks/hello.ts"
HOOK_COPY="$WORK/cache-copy/packs/$LOCK/pack/hooks/hello.ts"
write_input "$WORK/input-2.json" 2
write_input "$WORK/input-1567.json" 1567
mkdir -p "$WORK/workspace" "$WORK/plain" "$WORK/compile-cache"
printf '{ "type": "module" }\n' > "$WORK/plain/package.json"
printf 'const x: number = 1;\nexport { x };\n' > "$WORK/plain/empty.ts"
printf 'const x = 1;\nexport { x };\n' > "$WORK/plain/empty.js"

"$NODE" -e '
  const [work, symlink, copy] = process.argv.slice(1);
  const ws = `${work}/workspace`;
  const plan = [
    { name: "node -e 0", args: ["-e", "0"], cwd: ws, input: null },
    { name: "node empty.js", args: [`${work}/plain/empty.js`], cwd: ws, input: null },
    { name: "node empty.ts", args: [`${work}/plain/empty.ts`], cwd: ws, input: null },
    { name: "hook, SDK symlink", args: [symlink], cwd: ws, input: `${work}/input-2.json`, expectResult: true },
    { name: "hook, SDK copy", args: [copy], cwd: ws, input: `${work}/input-2.json`, expectResult: true },
    { name: "hook, 1567 tasks", args: [symlink], cwd: ws, input: `${work}/input-1567.json`, expectResult: true },
    { name: "hook, NODE_COMPILE_CACHE", args: [symlink], cwd: ws, input: `${work}/input-2.json`,
      env: { NODE_COMPILE_CACHE: `${work}/compile-cache` }, expectResult: true },
  ];
  require("node:fs").writeFileSync(`${work}/plan.json`, JSON.stringify(plan));
' "$WORK" "$HOOK_SYMLINK" "$HOOK_COPY"

printf 'input sizes: 2 tasks %s bytes, 1567 tasks %s bytes\n' \
  "$(wc -c < "$WORK/input-2.json" | tr -d ' ')" "$(wc -c < "$WORK/input-1567.json" | tr -d ' ')"
"$NODE" "$LIB/bench.mjs" --plan "$WORK/plan.json" --runs "$RUNS" --warmup 2 --out "$WORK/runs" | "$NODE" -e '
  let s = ""; process.stdin.on("data", (d) => (s += d)).on("end", () => {
    const [meta, ...rows] = s.trim().split("\n").map((l) => JSON.parse(l));
    console.log(`node ${meta.node}, ${meta.cpus} cpus, load average (1 min) ${meta.loadavg1m.before} -> ${meta.loadavg1m.after}, ` +
      `${meta.runs} timed runs per variant after ${meta.warmup} warm-up, round-robin`);
    console.log("variant".padEnd(28) + ["median", "min", "p10", "p90", "max", "IQR"].map((h) => h.padStart(8)).join("") + "  failures");
    for (const r of rows) {
      console.log(r.name.padEnd(28) + [r.medianMs, r.minMs, r.p10Ms, r.p90Ms, r.maxMs, r.iqrMs]
        .map((v) => String(v).padStart(8)).join("") + `  ${r.failures}`);
    }
  });'
