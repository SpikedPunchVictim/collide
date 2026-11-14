#!/bin/sh
# Builds work/: a scratch git repo (main checkout), a worktree with a builder's edits, and a
# "snapshot" of the repo's committed Claude config, as Collide would materialize it.
#
#   work/repo      branch main. Committed config is tagged BASE. Untracked main-only files
#                  (settings.local.json, CLAUDE.local.md) are tagged MAIN-LOCAL.
#   work/wt        worktree on branch builder. Uncommitted builder edits are tagged WT:
#                  settings hooks, .mcp.json server s1wt, a new skill, CLAUDE.md canary,
#                  and the repo's agent definition. Untracked settings.local.json is WT-LOCAL.
#   work/snapshot  built from `git show main:...`, retagged SNAP: settings.json, mcp.json,
#                  agents.json (from fixture/collide-test-builder.md) and a plugin with a skill.
#   work/plain     an empty non-git directory for questions that need no repo.
set -eu
S1="$(cd "$(dirname "$0")" && pwd)"
WORK="$S1/work"
rm -rf "$WORK"
mkdir -p "$WORK/plain"
R="$WORK/repo"
mkdir -p "$R/.claude/skills/s1-skill-base" "$R/.claude/agents"

mcp() { printf '{ "mcpServers": { "%s": { "command": "node", "args": ["%s", "%s"] } } }\n' "$1" "$S1/mcp-canary.mjs" "$1"; }
skill() {
  mkdir -p "$1"
  printf -- '---\nname: %s\ndescription: S1 canary skill (%s). Never invoke it.\n---\nReply with SKILL-%s.\n' "$2" "$3" "$3" >"$1/SKILL.md"
}

# --- committed on main (BASE) ---
cd "$R"
git init -q -b main
git config user.email s1@example.invalid
git config user.name "S1 spike"
printf '# S1 fixture repo\n\nThe S1 canary word is BASE-CANARY-OTTER.\n' >CLAUDE.md
node "$S1/make-settings.mjs" BASE >.claude/settings.json
mcp s1base >.mcp.json
skill .claude/skills/s1-skill-base s1-skill-base BASE
cat >.claude/agents/collide-test-builder.md <<'EOF'
---
name: collide-test-builder
description: Test builder from the repository (S1 clash fixture). Use for S1 tests only.
model: haiku
---
You are the repository's test builder for spike S1.
Begin every reply with the exact word REPOAGENT, then a space, then your answer.
EOF
printf 'alpha beta gamma\n' >notes.txt
git add -A
git commit -qm "S1 fixture base"

# --- untracked, main checkout only (MAIN-LOCAL) ---
node "$S1/make-settings.mjs" MAIN-LOCAL >.claude/settings.local.json
printf 'The S1 main-local canary word is MAINLOCAL-CANARY-LYNX.\n' >CLAUDE.local.md

# --- worktree with the builder's uncommitted edits (WT) ---
W="$WORK/wt"
git worktree add -q -b builder "$W"
cd "$W"
printf '# S1 fixture repo\n\nThe S1 canary word is WT-CANARY-HERON.\n' >CLAUDE.md
node "$S1/make-settings.mjs" WT >.claude/settings.json
mcp s1wt >.mcp.json
skill .claude/skills/s1-skill-wt s1-skill-wt WT
sed -i '' 's/REPOAGENT/WTAGENT/; s/the repository.s test builder/the worktree test builder/' .claude/agents/collide-test-builder.md
node "$S1/make-settings.mjs" WT-LOCAL >.claude/settings.local.json

# --- snapshot from main's commit (SNAP) ---
SNAP="$WORK/snapshot"
mkdir -p "$SNAP/plugin/s1snap/.claude-plugin" "$SNAP/plugin/s1snap/skills"
git -C "$R" show main:.claude/settings.json | sed 's/tag=BASE/tag=SNAP/g' >"$SNAP/settings.json"
mcp s1snap >"$SNAP/mcp.json"
node "$S1/build-agents.mjs" "$SNAP/agents.json" "$S1/fixture/collide-test-builder.md"
printf '{ "name": "s1snap", "version": "0.0.1", "description": "S1 snapshot plugin" }\n' >"$SNAP/plugin/s1snap/.claude-plugin/plugin.json"
skill "$SNAP/plugin/s1snap/skills/s1-skill-snap" s1-skill-snap SNAP

touch "$WORK/.fixture-ok"
echo "fixture ready: $WORK"
git -C "$R" worktree list
echo "worktree edits:"; git -C "$W" status --short
