#!/bin/sh
# S4 item 3: run hooks as Collide would (plan 0.5): argv only, cwd set to a workspace, JSON on stdin,
# COLLIDE_RESULT and COLLIDE_REPORT set. Check the result file, the verdict and every report line.
. "$(dirname "$0")/../lib/common.sh"
fresh_work 03-run-hook

say() { printf '%s\n' "$*"; }

APP_SDK="$WORK/app/Resources/pack-sdk"
build_sdk "$APP_SDK"
make_pack_repo "$WORK/repo"
DEST="$("$NODE" "$LIB/pack-cache.mjs" materialize --repo "$REPO" --ref main --path "$PACK_PATH" --lock "$LOCK" \
  --cache "$WORK/cache" --sdk "$APP_SDK" --link symlink | json_field dest)"
write_input "$WORK/input.json" 2
mkdir -p "$WORK/workspace"

# run HOOK RUN_NAME [extra collide-run flags...]
run() {
  hook="$1"; name="$2"; shift 2
  "$NODE" "$LIB/collide-run.mjs" --hook "$DEST/pack/hooks/$hook.ts" --cwd "$WORK/workspace" \
    --input "$WORK/input.json" --out "$WORK/runs/$name" "$@"
}

say "== hello.ts: argv [node, <cache>/pack/hooks/hello.ts], cwd <work>/workspace =="
run hello hello >/dev/null
"$NODE" --input-type=module - "$LIB/collide-run.mjs" "$WORK" <<'EOF'
const [lib, work] = process.argv.slice(2);
const { checkReport } = await import(lib);
const { readFileSync, realpathSync } = await import("node:fs");
const out = `${work}/runs/hello`;
const result = JSON.parse(readFileSync(`${out}/result.json`, "utf8"));
const short = (v) => typeof v === "string" ? v.replace(work, "<work>").replace(`file://${work}`, "<work>") : v;
console.log("result file:", JSON.stringify({ ...result, outputs: Object.fromEntries(
  Object.entries(result.outputs).map(([k, v]) => [k, short(v)])) }));
console.log("cwd is the workspace:", result.outputs.cwd === realpathSync(`${work}/workspace`));
const report = checkReport(`${out}/report.jsonl`);
console.log("report lines:");
for (const record of report.records) console.log("  ", JSON.stringify(record));
console.log("report check:", JSON.stringify({ lines: report.lines, kinds: report.kinds, invalid: report.invalid,
  unclosed: report.unclosed }));
EOF

say ""
say "== verdicts (0.5: the result file's status is the verdict; non-zero exit or missing file is error) =="
for hook in hello fail throws exits-early; do
  summary="$(run "$hook" "verdict-$hook")"
  printf '%s' "$summary" | "$NODE" -e '
    let s = ""; process.stdin.on("data", (d) => (s += d)).on("end", () => {
      const r = JSON.parse(s);
      console.log(`${process.argv[1].padEnd(12)} verdict=${r.verdict.padEnd(6)} exit=${r.exitCode} ` +
        `file=${r.result ? JSON.stringify({ status: r.result.status, message: r.result.message }) : "missing"} ` +
        `stderr=${r.stderr.bytes}B${r.problem ? ` problem="${r.problem}"` : ""}`);
    });' "$hook"
done

say ""
say "== nesting: COLLIDE_REPORT_PARENT=4121-1 =="
run hello parent --report-parent 4121-1 >/dev/null
sed -n '1,2p' "$WORK/runs/parent/report.jsonl"
