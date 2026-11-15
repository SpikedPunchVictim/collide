//! The base environment's capture (ts-0056-ag) over fake login shells. Each shell is a script the test writes into its
//! own folder under `/tmp`, so the user's real shell and profile are never run. The capture calls a shell as `$SHELL -l
//! -i -c <script>`, so a fake sees the script as its fourth argument and runs it with `eval "$4"`.

mod common;

use collide_core::adapter::EnvStrip;
use collide_host::base_env::{
    BaseEnvironment, CAPTURE_CAP_BYTES, CaptureOptions, EnvSource, FALLBACK_LANG,
};
use collide_host::clock::SystemClocks;
use common::TempRoot;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

/// A capture with a short timeout and a small fallback environment, so a test never waits and never depends on the
/// environment of the process that runs it.
fn options(timeout: Duration) -> CaptureOptions {
    CaptureOptions {
        timeout,
        cap_bytes: CAPTURE_CAP_BYTES,
        fallback: BTreeMap::from([
            ("PATH".to_string(), "/usr/bin:/bin".to_string()),
            ("USER".to_string(), "tester".to_string()),
        ]),
    }
}

fn capture(shell: &Path, options: &CaptureOptions) -> BaseEnvironment {
    BaseEnvironment::capture_with(shell.to_str().unwrap(), &SystemClocks, options)
}

fn reason_of(env: &BaseEnvironment) -> &str {
    match &env.source {
        EnvSource::Minimal { reason, .. } => reason,
        other => panic!("expected the minimal environment, got {other:?}"),
    }
}

fn alive(pid: i32) -> bool {
    // SAFETY: `kill` with signal 0 sends nothing; it only asks whether the pid exists.
    unsafe { libc::kill(pid, 0) == 0 }
}

/// Waits for a process this test's fake shell started to be gone.
fn gone(pid: i32) -> bool {
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if !alive(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

fn pid_in(path: &Path) -> i32 {
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if let Ok(text) = fs::read_to_string(path)
            && let Ok(pid) = text.trim().parse()
        {
            return pid;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("no pid in {}", path.display());
}

#[test]
fn a_login_shell_that_prints_its_environment_gives_it_back() {
    let dir = TempRoot::new();
    let shell = dir.script(
        "fake-shell",
        "#!/bin/sh\nexport COLLIDE_FAKE=from-the-profile\neval \"$4\"\n",
    );
    let env = capture(&shell, &options(Duration::from_secs(10)));
    assert_eq!(
        env.vars.get("COLLIDE_FAKE").map(String::as_str),
        Some("from-the-profile")
    );
    assert_eq!(
        env.source,
        EnvSource::LoginShell {
            shell: shell.to_str().unwrap().to_string()
        }
    );
}

#[test]
fn the_shell_is_called_with_l_i_c_and_a_script_that_prints_env_0() {
    let dir = TempRoot::new();
    let args = dir.join("args");
    let shell = dir.script(
        "fake-shell",
        &format!(
            "#!/bin/sh\nprintf '%s\\n' \"$1 $2 $3\" > {}\nprintf '%s' \"$4\" > {}.script\neval \"$4\"\n",
            args.display(),
            args.display()
        ),
    );
    let env = capture(&shell, &options(Duration::from_secs(10)));
    assert!(matches!(env.source, EnvSource::LoginShell { .. }));
    assert_eq!(fs::read_to_string(&args).unwrap().trim(), "-l -i -c");
    let script = fs::read_to_string(dir.join("args.script")).unwrap();
    assert!(script.contains("env -0"), "{script}");
}

#[test]
fn a_banner_before_the_marker_and_a_value_with_a_newline_are_read_correctly() {
    let dir = TempRoot::new();
    let shell = dir.script(
        "fake-shell",
        "#!/bin/sh\necho 'welcome to the profile'\nexport MULTI='line one\nline two'\neval \"$4\"\necho 'goodbye'\n",
    );
    let env = capture(&shell, &options(Duration::from_secs(10)));
    assert_eq!(
        env.vars.get("MULTI").map(String::as_str),
        Some("line one\nline two")
    );
    assert!(env.vars.keys().all(|k| !k.contains("welcome")));
}

#[test]
fn the_markers_are_new_for_each_capture() {
    let dir = TempRoot::new();
    let log = dir.join("scripts");
    let shell = dir.script(
        "fake-shell",
        &format!(
            "#!/bin/sh\nprintf '%s\\n' \"$4\" >> {}\neval \"$4\"\n",
            log.display()
        ),
    );
    capture(&shell, &options(Duration::from_secs(10)));
    capture(&shell, &options(Duration::from_secs(10)));
    let text = fs::read_to_string(&log).unwrap();
    let scripts: Vec<&str> = text.lines().collect();
    assert_eq!(scripts.len(), 2);
    assert_ne!(scripts[0], scripts[1], "the nonce is the same twice");
}

#[test]
fn stdin_is_dev_null_so_a_shell_that_reads_it_does_not_wait() {
    let dir = TempRoot::new();
    let shell = dir.script("fake-shell", "#!/bin/sh\ncat > /dev/null\neval \"$4\"\n");
    let start = Instant::now();
    let env = capture(&shell, &options(Duration::from_secs(10)));
    assert!(matches!(env.source, EnvSource::LoginShell { .. }));
    assert!(start.elapsed() < Duration::from_secs(5));
}

#[test]
fn a_hung_shell_falls_back_to_the_minimal_environment_and_is_killed() {
    let dir = TempRoot::new();
    let pid_file = dir.join("pid");
    let child_file = dir.join("child");
    let shell = dir.script(
        "fake-shell",
        &format!(
            "#!/bin/sh\necho $$ > {}\nsleep 60 &\necho $! > {}\nsleep 60\n",
            pid_file.display(),
            child_file.display()
        ),
    );
    let start = Instant::now();
    let env = capture(&shell, &options(Duration::from_millis(400)));
    assert!(
        start.elapsed() < Duration::from_secs(5),
        "{:?}",
        start.elapsed()
    );
    assert!(
        reason_of(&env).contains("did not finish"),
        "{}",
        reason_of(&env)
    );
    assert_eq!(env.vars.get("USER").map(String::as_str), Some("tester"));
    // The shell and what it started are in the session that was killed.
    assert!(gone(pid_in(&pid_file)), "the shell is still running");
    assert!(gone(pid_in(&child_file)), "its child is still running");
}

#[test]
fn what_the_shell_left_running_is_killed_after_the_capture_is_read() {
    let dir = TempRoot::new();
    let child_file = dir.join("child");
    let shell = dir.script(
        "fake-shell",
        &format!(
            "#!/bin/sh\nsleep 60 &\necho $! > {}\neval \"$4\"\nsleep 60\n",
            child_file.display()
        ),
    );
    let env = capture(&shell, &options(Duration::from_secs(10)));
    assert!(matches!(env.source, EnvSource::LoginShell { .. }));
    assert!(
        gone(pid_in(&child_file)),
        "the shell's child is still running"
    );
}

#[test]
fn output_over_the_cap_falls_back_with_the_cap_in_the_reason() {
    let dir = TempRoot::new();
    let shell = dir.script(
        "fake-shell",
        "#!/bin/sh\nhead -c 1200000 /dev/zero | tr '\\0' 'a'\neval \"$4\"\n",
    );
    let env = capture(&shell, &options(Duration::from_secs(10)));
    let reason = reason_of(&env);
    assert!(reason.contains("1048576"), "{reason}");
    assert!(!env.vars.contains_key("PATH") || env.vars["PATH"] == "/usr/bin:/bin");
}

#[test]
fn a_shell_that_exits_without_markers_falls_back_naming_the_status() {
    let dir = TempRoot::new();
    let shell = dir.script(
        "fake-shell",
        "#!/bin/sh\necho 'no environment here'\nexit 3\n",
    );
    let env = capture(&shell, &options(Duration::from_secs(10)));
    let reason = reason_of(&env);
    assert!(reason.contains("exit status 3"), "{reason}");
}

#[test]
fn a_shell_that_does_not_exist_falls_back_naming_it() {
    let env = BaseEnvironment::capture_with(
        "/nonexistent/collide-test-shell",
        &SystemClocks,
        &options(Duration::from_secs(10)),
    );
    let reason = reason_of(&env);
    assert!(
        reason.contains("/nonexistent/collide-test-shell"),
        "{reason}"
    );
    assert_eq!(
        env.source,
        EnvSource::Minimal {
            shell: Some("/nonexistent/collide-test-shell".to_string()),
            reason: reason.to_string()
        }
    );
}

#[test]
fn lang_is_set_when_the_login_shell_has_none() {
    let dir = TempRoot::new();
    let shell = dir.script("fake-shell", "#!/bin/sh\nunset LANG\neval \"$4\"\n");
    let env = capture(&shell, &options(Duration::from_secs(10)));
    assert_eq!(
        env.vars.get("LANG").map(String::as_str),
        Some(FALLBACK_LANG)
    );
}

#[test]
fn a_lang_the_shell_sets_is_kept() {
    let dir = TempRoot::new();
    let shell = dir.script(
        "fake-shell",
        "#!/bin/sh\nexport LANG=de_DE.UTF-8\neval \"$4\"\n",
    );
    let env = capture(&shell, &options(Duration::from_secs(10)));
    assert_eq!(
        env.vars.get("LANG").map(String::as_str),
        Some("de_DE.UTF-8")
    );
}

#[test]
fn lang_is_set_on_the_minimal_environment_too() {
    let env = BaseEnvironment::capture_with(
        "/nonexistent/collide-test-shell",
        &SystemClocks,
        &options(Duration::from_secs(1)),
    );
    assert_eq!(
        env.vars.get("LANG").map(String::as_str),
        Some(FALLBACK_LANG)
    );
}

#[test]
fn a_child_gets_the_capture_minus_the_strips_plus_its_own() {
    let dir = TempRoot::new();
    let shell = dir.script(
        "fake-shell",
        "#!/bin/sh\nexport TERM_SESSION_ID=t ITERM_SESSION_ID=i XPC_FLAGS=0x0 XPC_SERVICE_NAME=x\nexport ADAPTER_SECRET_KEY=k ADAPTER_SECRET_URL=u KEEP=yes\neval \"$4\"\n",
    );
    let env = capture(&shell, &options(Duration::from_secs(10)));
    let strip = EnvStrip {
        names: vec!["KEEP_NOT".to_string()],
        prefixes: vec!["ADAPTER_SECRET_".to_string()],
    };
    let extra = BTreeMap::from([("COLLIDE_RUN".to_string(), "run-1".to_string())]);
    let child = env.for_child(&strip, &extra);
    for gone in [
        "TERM_SESSION_ID",
        "ITERM_SESSION_ID",
        "XPC_FLAGS",
        "XPC_SERVICE_NAME",
        "ADAPTER_SECRET_KEY",
        "ADAPTER_SECRET_URL",
    ] {
        assert!(!child.contains_key(gone), "{gone} reached the child");
    }
    assert_eq!(child.get("KEEP").map(String::as_str), Some("yes"));
    assert_eq!(child.get("COLLIDE_RUN").map(String::as_str), Some("run-1"));
}

#[test]
fn a_child_phase_keeps_the_environment_it_copied_when_a_newer_capture_is_made() {
    let dir = TempRoot::new();
    let shell = dir.script(
        "fake-shell",
        "#!/bin/sh\nexport GEN=$(cat \"$0.gen\")\neval \"$4\"\n",
    );
    fs::write(dir.join("fake-shell.gen"), "one").unwrap();
    let first = capture(&shell, &options(Duration::from_secs(10)));
    let running_phase = first.for_child(&EnvStrip::default(), &BTreeMap::new());
    fs::write(dir.join("fake-shell.gen"), "two").unwrap();
    let second = capture(&shell, &options(Duration::from_secs(10)));
    assert_eq!(running_phase.get("GEN").map(String::as_str), Some("one"));
    assert_eq!(second.vars.get("GEN").map(String::as_str), Some("two"));
    assert!(second.captured_at.nanos >= first.captured_at.nanos);
}
