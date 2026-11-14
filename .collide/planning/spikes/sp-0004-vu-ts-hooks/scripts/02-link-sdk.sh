#!/bin/sh
# S4 item 2: link the bundled SDK into the materialized pack's node_modules. Compare symlink and copy, a compiled
# SDK and a .ts SDK, and show what a realpath under node_modules and an ancestor package.json change.
. "$(dirname "$0")/../lib/common.sh"
fresh_work 02-link-sdk

pc() { "$NODE" "$LIB/pack-cache.mjs" "$@"; }
say() { printf '%s\n' "$*"; }

# The SDK as Collide would bundle it: outside any node_modules, like Collide.app/Contents/Resources/pack-sdk.
APP_SDK="$WORK/app/Resources/pack-sdk"
build_sdk "$APP_SDK"
say "bundled SDK: $(cd "$APP_SDK" && ls | tr '\n' ' ')(runtime deps: $("$NODE" -p 'Object.keys(require(process.argv[1]).dependencies ?? {}).length' "$APP_SDK/package.json"))"

# A .ts SDK (not compiled), to see where Node refuses to strip types.
APP_SDK_TS="$WORK/app/Resources/pack-sdk-ts"
mkdir -p "$APP_SDK_TS"
cp "$SPIKE/sdk/src/index.ts" "$APP_SDK_TS/index.ts"
printf '{ "name": "@collide/pack-sdk", "version": "0.1.0-ts", "type": "module", "exports": { ".": "./index.ts" } }\n' \
  > "$APP_SDK_TS/package.json"

make_pack_repo "$WORK/repo"
write_input "$WORK/input.json" 2
mkdir -p "$WORK/workspace"

# materialize NAME CACHE_ROOT [extra pack-cache flags...] -> prints the pack dir
materialize() {
  name="$1"; cache="$2"; shift 2
  out="$(pc materialize --repo "$REPO" --ref main --path "$PACK_PATH" --lock "$LOCK" --cache "$cache" "$@")"
  dest="$(printf '%s' "$out" | json_field dest)"
  [ -n "$dest" ] || { say "$name: materialize failed: $out"; exit 1; }
  printf '%s' "$dest"
}

# run_variant NAME HOOK_PATH: runs the hook as Collide would and prints one line.
run_variant() {
  summary="$("$NODE" "$LIB/collide-run.mjs" --hook "$2" --cwd "$WORK/workspace" --input "$WORK/input.json" \
    --out "$WORK/runs/$1")"
  printf '%s' "$summary" | "$NODE" -e '
    let s = ""; process.stdin.on("data", (d) => (s += d)).on("end", () => {
      const r = JSON.parse(s); const [name, work] = process.argv.slice(1);
      const sdk = r.result?.outputs?.sdkUrl?.replace("file://" + work, "<work>") ?? "-";
      const version = r.result?.outputs?.sdkVersion ?? "-";
      console.log(`${name.padEnd(30)} verdict=${r.verdict.padEnd(5)} sdk=${version} from ${sdk}`);
      if (r.stderr.codes.length) console.log(`${"".padEnd(30)} stderr codes: ${r.stderr.codes.join(", ")}`);
      if (r.verdict !== "ok") console.log(`${"".padEnd(30)} ${r.problem}; stderr: ${r.stderr.firstLine}`);
    });' "$1" "$WORK"
}

say ""
say "== SDK link method x SDK form (pack at <work>/<variant>/packs/<hash>/pack) =="
D="$(materialize symlink-js "$WORK/symlink-js" --sdk "$APP_SDK" --link symlink)"
run_variant "symlink, compiled SDK" "$D/pack/hooks/hello.ts"
D="$(materialize copy-js "$WORK/copy-js" --sdk "$APP_SDK" --link copy)"
run_variant "copy, compiled SDK" "$D/pack/hooks/hello.ts"
D="$(materialize symlink-ts "$WORK/symlink-ts" --sdk "$APP_SDK_TS" --link symlink)"
run_variant "symlink, .ts SDK" "$D/pack/hooks/hello.ts"
D="$(materialize copy-ts "$WORK/copy-ts" --sdk "$APP_SDK_TS" --link copy)"
run_variant "copy, .ts SDK" "$D/pack/hooks/hello.ts"

say ""
say "== the pack's own .ts and node_modules in its path =="
D="$(materialize under-nm "$WORK/nm/node_modules/collide-cache" --sdk "$APP_SDK" --link symlink)"
run_variant "pack realpath under node_modules" "$D/pack/hooks/hello.ts"
mkdir -p "$WORK/nm-alias/node_modules"
ln -s "$WORK/symlink-js" "$WORK/nm-alias/node_modules/alias"
run_variant "node_modules alias, realpath not" "$WORK/nm-alias/node_modules/alias/packs/$LOCK/pack/hooks/hello.ts"

say ""
say "== module type: who decides that hooks/*.ts are ESM =="
say "(the spike directory's package.json has no \"type\" field and is an ancestor of every cache root here)"
D="$(materialize no-pkg "$WORK/no-pkg" --sdk "$APP_SDK" --link symlink --no-package-json)"
run_variant "no Collide package.json" "$D/pack/hooks/hello.ts"
mkdir -p "$WORK/cjs"
printf '{ "type": "commonjs" }\n' > "$WORK/cjs/package.json"
D="$(materialize cjs-no-pkg "$WORK/cjs/no-pkg" --sdk "$APP_SDK" --link symlink --no-package-json)"
run_variant "ancestor type=commonjs" "$D/pack/hooks/hello.ts"
D="$(materialize cjs-pkg "$WORK/cjs/with-pkg" --sdk "$APP_SDK" --link symlink)"
run_variant "ancestor commonjs + Collide's" "$D/pack/hooks/hello.ts"

say ""
say "== a pack that commits its own node_modules/@collide/pack-sdk =="
SHADOW="$REPO/$PACK_PATH/node_modules/@collide/pack-sdk"
mkdir -p "$SHADOW"
cp "$APP_SDK/index.js" "$APP_SDK/package.json" "$SHADOW/"
sed -i '' 's/SDK_VERSION = "0.1.0"/SDK_VERSION = "9.9.9-shadow"/' "$SHADOW/index.js"
repo_git add -A && repo_git commit -q -m "pack vendors an SDK copy"
LOCK="$(pc tree --repo "$REPO" --ref main --path "$PACK_PATH" | json_field tree)"
D="$(materialize shadow "$WORK/shadow" --sdk "$APP_SDK" --link symlink)"
run_variant "pack-committed SDK copy" "$D/pack/hooks/hello.ts"
