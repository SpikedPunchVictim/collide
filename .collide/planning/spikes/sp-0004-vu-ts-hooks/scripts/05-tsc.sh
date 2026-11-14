#!/bin/sh
# S4 item 5: `tsc --noEmit` over the materialized pack against the SDK's .d.ts, with a Collide-supplied tsconfig.
# A deliberate type error must fail it. TypeScript is installed only in this spike directory (package.json).
. "$(dirname "$0")/../lib/common.sh"
fresh_work 05-tsc

pc() { "$NODE" "$LIB/pack-cache.mjs" "$@"; }
say() { printf '%s\n' "$*"; }

ensure_deps
NATIVE="$("$NODE" -e 'const p=require("node:path");const d=p.dirname(require.resolve("@typescript/typescript-darwin-arm64/package.json",{paths:[require("node:fs").realpathSync(process.argv[1])]}));console.log(p.join(d,"lib","tsc"))' "$SPIKE/node_modules/typescript" 2>/dev/null || true)"
say "tsc: $("$TSC" --version) (package typescript $("$NODE" -p 'require(process.argv[1]).version' "$SPIKE/node_modules/typescript/package.json"))"
if [ -n "$NATIVE" ] && [ -x "$NATIVE" ]; then
  say "native compiler: $(du -h "$NATIVE" | cut -f1) $(file -b "$NATIVE" | cut -d, -f1); bin/tsc is a Node shim that execve()s it"
fi

APP_SDK="$WORK/app/Resources/pack-sdk"
build_sdk "$APP_SDK"
make_pack_repo "$WORK/repo"
GOOD="$(pc materialize --repo "$REPO" --ref main --path "$PACK_PATH" --lock "$LOCK" --cache "$WORK/cache" \
  --sdk "$APP_SDK" --link symlink | json_field dest)"
GOOD_COPY="$(pc materialize --repo "$REPO" --ref main --path "$PACK_PATH" --lock "$LOCK" --cache "$WORK/cache-copy" \
  --sdk "$APP_SDK" --link copy | json_field dest)"

# Commit a deliberate type error, then a commit with two non-erasable or non-type-only constructs.
sed -i '' 's/lanePool: ctx.config.lanePool,/lanePool: ctx.config.lanePool satisfies string,/' "$REPO/$PACK_PATH/hooks/hello.ts"
repo_git commit -q -am "deliberate type error"
BAD_LOCK="$(pc tree --repo "$REPO" --ref main --path "$PACK_PATH" | json_field tree)"
BAD="$(pc materialize --repo "$REPO" --ref main --path "$PACK_PATH" --lock "$BAD_LOCK" --cache "$WORK/cache" \
  --sdk "$APP_SDK" --link symlink | json_field dest)"
repo_git revert --no-edit HEAD >/dev/null
cat > "$REPO/$PACK_PATH/hooks/type-import.ts" <<'EOF'
import { defineHook, HookInput } from "@collide/pack-sdk";

defineHook(async (input: HookInput, ctx) => ctx.ok({ hook: input.hook }));
EOF
cat > "$REPO/$PACK_PATH/hooks/enum.ts" <<'EOF'
import { defineHook } from "@collide/pack-sdk";

enum Lane { Build, Fix }

defineHook(async (_input, ctx) => ctx.ok({ lane: Lane.Fix }));
EOF
repo_git add -A && repo_git commit -q -m "syntax hazards"
SYNTAX_LOCK="$(pc tree --repo "$REPO" --ref main --path "$PACK_PATH" | json_field tree)"
SYNTAX="$(pc materialize --repo "$REPO" --ref main --path "$PACK_PATH" --lock "$SYNTAX_LOCK" --cache "$WORK/cache" \
  --sdk "$APP_SDK" --link symlink | json_field dest)"

# The tsconfig Collide would generate for `collide pack test`, outside the read-only cache.
# write_tsconfig NAME PACK_DIR TYPES_JSON
write_tsconfig() {
  cat > "$WORK/tsconfig.$1.json" <<EOF
{
  "compilerOptions": {
    "noEmit": true,
    "strict": true,
    "target": "es2023",
    "module": "nodenext",
    "moduleResolution": "nodenext",
    "allowImportingTsExtensions": true,
    "erasableSyntaxOnly": true,
    "verbatimModuleSyntax": true,
    "types": $3,
    "typeRoots": ["$SPIKE/node_modules/@types"]
  },
  "include": ["$2/pack/**/*.ts"]
}
EOF
}

# check ID LABEL PACK_DIR TYPES_JSON: runs tsc --noEmit once and prints exit code, time and error codes.
check() {
  id="$1"; label="$2"; shift 2
  set -- "$id" "$1" "$2"
  write_tsconfig "$id" "$2" "$3"
  start="$("$NODE" -p 'performance.timeOrigin + performance.now()')"
  set +e
  "$TSC" --noEmit -p "$WORK/tsconfig.$1.json" --pretty false > "$WORK/tsc.$1.log" 2>&1
  code=$?
  set -e
  end="$("$NODE" -p 'performance.timeOrigin + performance.now()')"
  ms="$("$NODE" -p "Math.round($end - $start)")"
  codes="$(grep -o 'error TS[0-9]*' "$WORK/tsc.$1.log" | sort | uniq -c | awk '{printf "%s x%s; ", $3, $1}')"
  say "$(printf '%-36s' "$label") exit=$code ${ms}ms ${codes:-no errors}"
  sed -E "s#$WORK/##g" "$WORK/tsc.$1.log" | grep 'error TS' | head -4 | sed 's/^/    /'
}

say ""
say "== tsc --noEmit over the materialized pack (wall time includes the Node shim) =="
check good "good, SDK symlink, @types/node" "$GOOD" '["node"]'
check good-copy "good, SDK copy, @types/node" "$GOOD_COPY" '["node"]'
check no-node-types "good, SDK symlink, no @types/node" "$GOOD" '[]'
check type-error "deliberate type error" "$BAD" '["node"]'
check syntax "type import without 'type', enum" "$SYNTAX" '["node"]'

say ""
say "== which SDK declaration file tsc read (--listFiles) =="
"$TSC" --noEmit -p "$WORK/tsconfig.good.json" --listFiles 2>&1 | grep -i 'pack-sdk' | sed -E "s#$WORK#<work>#g" || say "(no pack-sdk entry in --listFiles)"

say ""
say "== the same two hazards at run time (type stripping only, no tsc) =="
write_input "$WORK/input.json" 1
mkdir -p "$WORK/workspace"
for hook in type-import enum; do
  "$NODE" "$LIB/collide-run.mjs" --hook "$SYNTAX/pack/hooks/$hook.ts" --cwd "$WORK/workspace" \
    --input "$WORK/input.json" --out "$WORK/runs/$hook" | "$NODE" -e '
    let s = ""; process.stdin.on("data", (d) => (s += d)).on("end", () => {
      const r = JSON.parse(s);
      console.log(`${process.argv[1].padEnd(12)} verdict=${r.verdict} ${r.stderr.codes.join(",")}`);
    });' "$hook"
  grep -m1 -E 'SyntaxError|Error \[' "$WORK/runs/$hook/stderr.log" | sed -E "s#$WORK#<work>#g; s/^/    /" | cut -c1-200
done

say ""
say "== timing: the good check, 7 runs (a loaded machine inflates these; see the record) =="
"$NODE" -e '
  const { spawnSync } = require("node:child_process");
  const [tsc, config] = process.argv.slice(1);
  const times = [];
  for (let i = 0; i < 7; i += 1) {
    const t = performance.now();
    const r = spawnSync(tsc, ["--noEmit", "-p", config]);
    if (r.status !== 0) throw new Error(`tsc exited ${r.status}`);
    times.push(performance.now() - t);
  }
  times.sort((a, b) => a - b);
  console.log(`median ${Math.round(times[3])} ms, min ${Math.round(times[0])} ms, max ${Math.round(times[6])} ms`);
' "$TSC" "$WORK/tsconfig.good.json"
"$NODE" -e '
  const { spawnSync } = require("node:child_process");
  const [tsc, config] = process.argv.slice(1);
  const r = spawnSync(tsc, ["--noEmit", "-p", config, "--extendedDiagnostics"], { encoding: "utf8" });
  const keep = r.stdout.split("\n").filter((l) => /^(Files|Lines of Library|Lines of Definitions|Lines of TypeScript|Parse time|Bind time|Check time|Total time|Memory used)/.test(l));
  console.log(keep.length ? keep.map((l) => "  " + l.replace(/\s+/g, " ")).join("\n") : "  (--extendedDiagnostics printed nothing recognisable)");
' "$TSC" "$WORK/tsconfig.good.json"
