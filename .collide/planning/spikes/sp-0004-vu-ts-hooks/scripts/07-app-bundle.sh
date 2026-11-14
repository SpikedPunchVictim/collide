#!/bin/sh
# S4 item 6, the real thing: a tiny .app whose executable is a shell script, launched through LaunchServices,
# records its real launch environment (variable names and PATH only), then runs the same login-shell capture
# as script 06. Three launches:
#   1. `open` from this shell;
#   2. `open` under `env -i` (what `open` forwards);
#   3. `open` run by a transient launchd job in the user's gui domain: the launchd environment a Finder or Dock
#      launch gets. The job's plist stays in work/; the job is booted out as soon as the probe finishes.
. "$(dirname "$0")/../lib/common.sh"
fresh_work 07-app-bundle

say() { printf '%s\n' "$*"; }

APP="$WORK/S4EnvProbe.app"
mkdir -p "$APP/Contents/MacOS"
cat > "$APP/Contents/Info.plist" <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key><string>s4probe</string>
  <key>CFBundleIdentifier</key><string>dev.collide.spike.s4-env-probe</string>
  <key>CFBundleName</key><string>S4EnvProbe</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>LSUIElement</key><true/>
</dict>
</plist>
EOF
# The output directory arrives as the first argument (open --args DIR).
cat > "$APP/Contents/MacOS/s4probe" <<EOF
#!/bin/sh
OUT="\$1"
mkdir -p "\$OUT"
echo "\$PPID" > "\$OUT/ppid"
'$NODE' '$LIB/env-capture.mjs' --runs 3 --json > "\$OUT/capture.json" 2> "\$OUT/capture.err"
echo "\$?" > "\$OUT/done"
EOF
chmod 755 "$APP/Contents/MacOS/s4probe"

wait_done() {
  i=0
  while [ ! -f "$1/done" ] && [ "$i" -lt 120 ]; do sleep 0.5; i=$((i + 1)); done
  [ -f "$1/done" ] || { say "  the probe did not finish within 60 s"; return 1; }
}

summarize() {
  say "  probe exit $(cat "$1/done"); the app script's parent pid: $(cat "$1/ppid") (1 = launchd)"
  "$NODE" -e '
    const r = JSON.parse(require("node:fs").readFileSync(process.argv[1], "utf8"));
    const t = (s) => String(s).split(process.env.HOME).join("~");
    const line = (tools) => Object.entries(tools).map(([k, v]) => `${k}=${v.path ? "yes" : "NO"}`).join(" ");
    const markers = ["CLAUDECODE", "TERM_PROGRAM", "GIT_CONFIG_GLOBAL", "SHELL", "USER", "TMPDIR", "XPC_SERVICE_NAME", "__CFBundleIdentifier"]
      .map((n) => `${n}=${r.launch.names.includes(n) ? "set" : "unset"}`).join(" ");
    console.log(`  launch env: ${r.launch.names.length} variables; ${markers}`);
    console.log(`  launch PATH: ${t(r.launch.PATH)}`);
    console.log(`  login shell: ${r.launch.shell} (from ${r.launch.shellSource}); tools on the launch PATH: ${line(r.launch.tools)}`);
    for (const m of r.modes) {
      console.log(`  ${m.name}: median ${m.medianMs} ms (min ${m.minMs}, max ${m.maxMs}), parsed ${m.parsed}/${m.runs}, ` +
        `pollution ${m.pollution.stdoutBefore}/${m.pollution.stdoutAfter}/${m.pollution.stderrBytes} B; ${line(m.tools ?? {})}`);
    }
  ' "$1/capture.json"
}

say "== 1. open -n S4EnvProbe.app, from this shell =="
open -n "$APP" --args "$WORK/out-open"
wait_done "$WORK/out-open" && summarize "$WORK/out-open"

say ""
say "== 2. env -i HOME=\$HOME PATH=/usr/bin:/bin:/usr/sbin:/sbin open -n S4EnvProbe.app =="
env -i HOME="$HOME" PATH=/usr/bin:/bin:/usr/sbin:/sbin /usr/bin/open -n "$APP" --args "$WORK/out-open-env-i"
wait_done "$WORK/out-open-env-i" && summarize "$WORK/out-open-env-i"

say ""
say "== 3. a launchd job (gui/$(id -u)) runs open -n S4EnvProbe.app =="
say "  launchctl getenv PATH: '$(launchctl getenv PATH)' (empty = no user-domain override)"
LABEL="dev.collide.spike.s4-env-probe.$$"
cat > "$WORK/$LABEL.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>$LABEL</string>
  <key>ProgramArguments</key>
  <array><string>/usr/bin/open</string><string>-n</string><string>$APP</string><string>--args</string><string>$WORK/out-launchd</string></array>
  <key>RunAtLoad</key><true/>
  <key>StandardErrorPath</key><string>$WORK/launchd-open.err</string>
</dict>
</plist>
EOF
if launchctl bootstrap "gui/$(id -u)" "$WORK/$LABEL.plist"; then
  wait_done "$WORK/out-launchd" && summarize "$WORK/out-launchd" || true
  launchctl bootout "gui/$(id -u)/$LABEL" 2>/dev/null || true
  say "  job booted out: $(launchctl print "gui/$(id -u)/$LABEL" >/dev/null 2>&1 && echo NO || echo yes)"
else
  say "  launchctl bootstrap failed; this launch is unmeasured"
fi

LSREGISTER=/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister
[ -x "$LSREGISTER" ] && "$LSREGISTER" -u "$APP" 2>/dev/null || true
