# Shared helpers for the S4 scripts. Source it from scripts/*.sh; do not run it.
# Everything a script writes goes under work/<script name>/, which the script recreates.
set -eu

SPIKE="$(cd "$(dirname "$0")/.." && pwd)"
LIB="$SPIKE/lib"
NODE="$(command -v node)"
PACK_PATH=".collide/packs/hello"

# Scratch repos ignore the user's global and system git config and hooks, so runs are reproducible.
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1
GIT_ID="-c user.name=s4-spike -c user.email=s4@example.invalid -c commit.gpgsign=false"

fresh_work() {
  WORK="$SPIKE/work/$1"
  if [ -d "$WORK" ]; then chmod -R u+w "$WORK"; rm -rf "$WORK"; fi
  mkdir -p "$WORK"
}

# TypeScript is installed only inside the spike directory, from its lockfile, into a spike-local store.
ensure_deps() {
  if [ ! -x "$SPIKE/node_modules/.bin/tsc" ]; then
    (cd "$SPIKE" && pnpm install --frozen-lockfile --prefer-offline \
      --store-dir "$SPIKE/.pnpm-store" --cache-dir "$SPIKE/work/pnpm-cache" >&2)
  fi
  TSC="$SPIKE/node_modules/.bin/tsc"
}

# Builds the stand-in @collide/pack-sdk (compiled JS + .d.ts) into $1, as Collide would bundle it.
build_sdk() {
  ensure_deps
  mkdir -p "$1"
  "$TSC" -p "$SPIKE/sdk/tsconfig.json" --outDir "$1"
  cp "$SPIKE/sdk/package.json" "$1/package.json"
}

repo_git() { git -C "$REPO" $GIT_ID "$@"; }

# Creates $1 as a git repo holding the fixture pack at $PACK_PATH, commits it, and records the lock.
# Sets REPO and LOCK.
make_pack_repo() {
  REPO="$1"
  git init -q -b main "$REPO"
  mkdir -p "$REPO/$PACK_PATH"
  cp -R "$SPIKE/fixture/pack/." "$REPO/$PACK_PATH/"
  printf '#!/bin/sh\nexec node --version\n' > "$REPO/$PACK_PATH/hooks/version.sh"
  chmod 755 "$REPO/$PACK_PATH/hooks/version.sh"
  repo_git add -A
  repo_git commit -q -m "add hello pack"
  LOCK="$("$NODE" "$LIB/pack-cache.mjs" tree --repo "$REPO" --ref main --path "$PACK_PATH" | json_field tree)"
  printf 'packs:\n  hello: { path: %s, tree: %s }\n' "$PACK_PATH" "$LOCK" > "$REPO/.collide/packs.lock"
  repo_git add .collide/packs.lock
  repo_git commit -q -m "lock hello pack"
}

# Reads one top-level field from a JSON object on stdin.
json_field() {
  "$NODE" -e 'let s="";process.stdin.on("data",d=>s+=d).on("end",()=>{const v=JSON.parse(s)[process.argv[1]];process.stdout.write(v===undefined?"":typeof v==="string"?v:JSON.stringify(v))})' "$1"
}

# Writes a 0.5-shaped hook input with $2 tasks to $1.
write_input() {
  "$NODE" -e '
    const [file, count] = [process.argv[1], Number(process.argv[2])];
    const tasks = Array.from({ length: count }, (_, i) => ({
      key: `plan:T-00-${String(i + 1).padStart(3, "0")}`, shortId: `T-${i + 1}`,
      path: `arch/tasks/T-${i + 1}.md`, fields: { labelId: `t00-${String(i + 1).padStart(3, "0")}` },
      title: "x".repeat(200),
    }));
    const input = {
      protocol: 0, hook: "hello", pack: "hello", board: { id: "b1" }, line: { ref: "main" }, experiment: null,
      run: { id: "run-s4" }, launch: {}, config: { greeting: "hi", lanePool: 3 }, tasks,
      phase: { name: "build", round: 1 }, workspace: { path: process.cwd() }, history: [],
      state: { run: {}, workspace: {}, point: {} },
    };
    require("node:fs").writeFileSync(file, JSON.stringify(input));
  ' "$1" "$2"
}
