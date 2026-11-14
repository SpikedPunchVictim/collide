#!/bin/sh
# S4 item 1: materialize a pack from a git snapshot into a cache keyed by its content hash, check the lock,
# and show that uncommitted edits (working tree, index, untracked, .git/info/attributes) never reach it.
. "$(dirname "$0")/../lib/common.sh"
fresh_work 01-materialize

pc() { "$NODE" "$LIB/pack-cache.mjs" "$@"; }
say() { printf '%s\n' "$*"; }

make_pack_repo "$WORK/repo"
BASE="$(repo_git rev-parse HEAD)"
reset_repo() { repo_git reset -q --hard "$BASE"; repo_git clean -fdq; }
say "lock (git tree id of main:$PACK_PATH): $LOCK"

# Uncommitted edits of three kinds. None may reach the snapshot.
printf '\n// MARK-WORKTREE-EDIT\n' >> "$REPO/$PACK_PATH/hooks/hello.ts"
printf '\n// MARK-STAGED-EDIT\n' >> "$REPO/$PACK_PATH/hooks/lib/greet.ts"
repo_git add "$PACK_PATH/hooks/lib/greet.ts"
printf 'export const x = "MARK-UNTRACKED";\n' > "$REPO/$PACK_PATH/hooks/untracked.ts"

say ""
say "== methods, with uncommitted edits present =="
for method in raw archive-tree archive-commit; do
  out="$(pc materialize --repo "$REPO" --ref main --path "$PACK_PATH" --lock "$LOCK" \
    --cache "$WORK/cache-$method" --method "$method" || true)"
  dest="$(printf '%s' "$out" | json_field dest)"
  marks="none"
  if [ -n "$dest" ] && grep -rq 'MARK-' "$dest/pack"; then marks="$(grep -rho 'MARK-[A-Z-]*' "$dest/pack" | sort -u | tr '\n' ' ')"; fi
  say "$method: $out"
  say "  edits found in snapshot: $marks; executable bit on hooks/version.sh: $( [ -n "$dest" ] && [ -x "$dest/pack/hooks/version.sh" ] && echo yes || echo no)"
done

say ""
say "== cache hit (same tree, second load) =="
pc materialize --repo "$REPO" --ref main --path "$PACK_PATH" --lock "$LOCK" --cache "$WORK/cache-raw" --method raw

say ""
say "== committed change without a lock update =="
repo_git commit -q -m "edit pack (staged greet.ts edit)"
pc materialize --repo "$REPO" --ref main --path "$PACK_PATH" --lock "$LOCK" --cache "$WORK/cache-raw" --method raw || true
reset_repo   # back to the locked commit, without the uncommitted edits

say ""
say "== hazard: .git/info/attributes (local, never committed) =="
printf '*.ts text eol=crlf\npack.yaml export-ignore\n' > "$REPO/.git/info/attributes"
for method in raw archive-tree archive-commit; do
  say "$method: $(pc materialize --repo "$REPO" --ref main --path "$PACK_PATH" --lock "$LOCK" \
    --cache "$WORK/hazard-info-$method" --method "$method" || true)"
done
rm "$REPO/.git/info/attributes"

say ""
say "== hazard: export-subst in .git/info/attributes =="
reset_repo
printf '\n# built from $Format:%%H$\n' >> "$REPO/$PACK_PATH/pack.yaml"
repo_git commit -q -am "pack.yaml with a Format placeholder"
SUBST_LOCK="$(pc tree --repo "$REPO" --ref main --path "$PACK_PATH" | json_field tree)"
printf 'pack.yaml export-subst\n' > "$REPO/.git/info/attributes"
for method in raw archive-tree archive-commit; do
  say "$method: $(pc materialize --repo "$REPO" --ref main --path "$PACK_PATH" --lock "$SUBST_LOCK" \
    --cache "$WORK/hazard-subst-$method" --method "$method" || true)"
done
rm "$REPO/.git/info/attributes"

say ""
say "== hazard: committed .gitattributes inside the pack (export-ignore) =="
reset_repo
printf 'hooks/lib/** export-ignore\n' > "$REPO/$PACK_PATH/.gitattributes"
repo_git add -A && repo_git commit -q -m "pack .gitattributes"
ATTR_LOCK="$(pc tree --repo "$REPO" --ref main --path "$PACK_PATH" | json_field tree)"
for method in raw archive-tree archive-commit; do
  say "$method: $(pc materialize --repo "$REPO" --ref main --path "$PACK_PATH" --lock "$ATTR_LOCK" \
    --cache "$WORK/hazard-attr-$method" --method "$method" || true)"
done

say ""
say "== hazard: committed symlink inside the pack =="
reset_repo
ln -s ../../../../outside.ts "$REPO/$PACK_PATH/hooks/escape.ts"
repo_git add -A && repo_git commit -q -m "pack symlink"
LINK_LOCK="$(pc tree --repo "$REPO" --ref main --path "$PACK_PATH" | json_field tree)"
for method in raw archive-tree; do
  out="$(pc materialize --repo "$REPO" --ref main --path "$PACK_PATH" --lock "$LINK_LOCK" \
    --cache "$WORK/hazard-link-$method" --method "$method" --writable || true)"
  say "$method: $out"
  dest="$(printf '%s' "$out" | json_field dest)"
  if [ -n "$dest" ] && [ -L "$dest/pack/hooks/escape.ts" ]; then
    say "  extracted hooks/escape.ts is a symlink to $(readlink "$dest/pack/hooks/escape.ts")"
  fi
done

say ""
say "== verify cost: hash the extracted pack directory without git (20 runs) =="
pc hash-dir --dir "$WORK/cache-raw/packs/$LOCK/pack" --repeat 20

say ""
say "== concurrent first loads of one tree: 2 processes at once, 5 rounds =="
reset_repo
for round in 1 2 3 4 5; do
  cache="$WORK/race-$round"
  pc materialize --repo "$REPO" --ref main --path "$PACK_PATH" --lock "$LOCK" --cache "$cache" > "$WORK/race-$round-a.json" &
  pa=$!
  pc materialize --repo "$REPO" --ref main --path "$PACK_PATH" --lock "$LOCK" --cache "$cache" > "$WORK/race-$round-b.json" &
  pb=$!
  wait "$pa" || true
  wait "$pb" || true
  leftovers="$(find "$cache/packs" -maxdepth 1 -name '.staging-*' | wc -l | tr -d ' ')"
  say "round $round: a=$(json_field cache < "$WORK/race-$round-a.json") b=$(json_field cache < "$WORK/race-$round-b.json"), staging dirs left: $leftovers"
done
