#!/bin/sh
# S4 item 6: an app launched from Finder or launchd gets a minimal PATH. Simulate that launch with env -i, then
# capture the user's login-shell environment (-l -c, and -l -i -c) and look for node, claude, pnpm, gh.
# Prints variable names and PATH only. Usage: scripts/06-finder-env.sh [RUNS]   (default 5 per mode)
. "$(dirname "$0")/../lib/common.sh"
fresh_work 06-finder-env
RUNS="${1:-5}"

say() { printf '%s\n' "$*"; }

say "== simulated Finder launch: env -i HOME=\$HOME PATH=/usr/bin:/bin:/usr/sbin:/sbin =="
env -i HOME="$HOME" PATH=/usr/bin:/bin:/usr/sbin:/sbin "$NODE" "$LIB/env-capture.mjs" --runs "$RUNS" \
  | sed "s#$HOME#~#g"

# Delimiting check: synthetic rc files that print to stdout and stderr, including a decoy marker.
# ZDOTDIR points zsh at them, so the user's real rc files are not read in this part.
mkdir -p "$WORK/zdotdir"
cat > "$WORK/zdotdir/.zprofile" <<'EOF'
echo "zprofile: hello on stdout"
printf '__COLLIDE_ENV_BEGIN_decoy__ PATH=/decoy\0'
EOF
cat > "$WORK/zdotdir/.zshrc" <<'EOF'
echo "zshrc: hello on stdout"
echo "zshrc: warning on stderr" >&2
export S4_MULTILINE="line one
line two"
export PATH="/opt/s4-synthetic/bin:$PATH"
EOF
say ""
say "== delimiting with noisy synthetic rc files (ZDOTDIR=work/06-finder-env/zdotdir) =="
env -i HOME="$HOME" PATH=/usr/bin:/bin:/usr/sbin:/sbin SHELL=/bin/zsh ZDOTDIR="$WORK/zdotdir" \
  "$NODE" "$LIB/env-capture.mjs" --runs 1 --json | "$NODE" -e '
  let s = ""; process.stdin.on("data", (d) => (s += d)).on("end", () => {
    const r = JSON.parse(s);
    for (const m of r.modes) {
      console.log(`${m.name}: parsed ${m.parsed}/${m.runs}; multi-line value intact: ${m.multilineIntact}; pollution ${JSON.stringify(m.pollution)}`);
      console.log(`  PATH starts with the rc entry: ${String(m.PATH).startsWith("/opt/s4-synthetic/bin")}; PATH=${m.PATH}`);
    }
  });'

# The user's own startup files minus ~/.zshenv, without copying or changing them: a ZDOTDIR shim that sources
# ~/.zprofile and ~/.zshrc. Shows which tools depend on ~/.zshenv (zsh reads it for every shell, even `zsh -c`).
mkdir -p "$WORK/no-zshenv"
printf '[ -f "$HOME/.zprofile" ] && . "$HOME/.zprofile"\n' > "$WORK/no-zshenv/.zprofile"
printf '[ -f "$HOME/.zshrc" ] && . "$HOME/.zshrc"\n' > "$WORK/no-zshenv/.zshrc"
say ""
say "== the user's .zprofile and .zshrc, without ~/.zshenv (ZDOTDIR shim) =="
env -i HOME="$HOME" PATH=/usr/bin:/bin:/usr/sbin:/sbin ZDOTDIR="$WORK/no-zshenv" \
  "$NODE" "$LIB/env-capture.mjs" --runs 1 --json | "$NODE" -e '
  let s = ""; process.stdin.on("data", (d) => (s += d)).on("end", () => {
    const r = JSON.parse(s);
    for (const m of r.modes) {
      const found = Object.entries(m.tools ?? {}).map(([t, v]) => `${t}=${v.path ? "yes" : "NO"}`).join(" ");
      console.log(`${m.name}: parsed ${m.parsed}/${m.runs}; ${found}`);
    }
  });'
