//! `collide config get` as a process (ts-0098-nx): the real `collide` binary, which starts a real `collide-daemon`
//! under the test's own root. What needs a checkout and packs is in `config_command.rs`; here are the cases that need
//! neither: the user's layers outside a checkout, the usage, the problems of a user file, and `set`, which is still the
//! tagged stub. Every process has a cleared environment with `HOME`, `COLLIDE_DAEMON_ROOT` and `SHELL` in the test's
//! own folder under `/tmp`, runs in that folder (which is not in a repository), and the daemon is stopped at the end by
//! the pid it reports on the test's own socket.

mod common;

use collide_host::daemon::DaemonPaths;
use common::{TempRoot, daemon_status, wait_until};
use std::fs;
use std::process::{Command, Output, Stdio};
use std::time::Duration;

const COLLIDE: &str = env!("CARGO_BIN_EXE_collide");
// Cargo builds the daemon for these tests only when a test names it.
const _DAEMON: &str = env!("CARGO_BIN_EXE_collide-daemon");

/// A login shell that prints its environment, so the daemon's capture never runs the user's own shell.
fn fake_shell(root: &TempRoot) -> String {
    root.script("fake-shell", "#!/bin/sh\neval \"$4\"\n")
        .to_str()
        .unwrap()
        .to_string()
}

/// `collide <args>` from the test's folder with an environment of the test's own. It is not a child of this process
/// (see `common::detached`), so a test run inside a run of Collide is not its run's agent.
fn collide(root: &TempRoot, args: &[&str]) -> Output {
    let mut command = Command::new(COLLIDE);
    command
        .args(args)
        .current_dir(root.path())
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", root.path())
        .env("SHELL", fake_shell(root))
        .env("COLLIDE_DAEMON_ROOT", root.path())
        .stdin(Stdio::null());
    common::detached::Detached::start(root, &command).wait()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Stops the daemon the CLI started, by the pid it reports on this test's own socket.
struct DaemonGuard(DaemonPaths);

impl Drop for DaemonGuard {
    fn drop(&mut self) {
        let pid = daemon_status(&self.0).and_then(|s| s["pid"].as_i64());
        if let Some(pid) = pid {
            // SAFETY: `kill` takes two integers; the pid is the one the daemon on this test's own socket reports.
            unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
            wait_until("the daemon ends", Duration::from_secs(10), || {
                // SAFETY: signal 0 sends nothing.
                unsafe { libc::kill(pid as libc::pid_t, 0) != 0 }
            });
        }
    }
}

fn write_user_config(root: &TempRoot, text: &str) {
    let dir = root.join(".config/collide");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("config.yaml"), text).unwrap();
}

#[test]
fn a_key_outside_a_checkout_prints_the_users_value_and_its_layer() {
    let root = TempRoot::new();
    let _guard = DaemonGuard(DaemonPaths::under(root.path()));
    write_user_config(&root, "disk: { floor_gib: 3 }\n");
    let output = collide(&root, &["config", "get", "disk.floor_gib"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert_eq!(text(&output.stdout), "disk.floor_gib = 3  (user)\n");
    assert!(text(&output.stderr).contains("user's layers only"));
}

#[test]
fn with_no_key_every_setting_of_collides_own_is_listed_with_its_layer() {
    let root = TempRoot::new();
    let _guard = DaemonGuard(DaemonPaths::under(root.path()));
    let output = collide(&root, &["config", "get"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert_eq!(
        text(&output.stdout),
        "ask.agent = <not set>  (default)\n\
         ask.timeout_s = <not set>  (default)\n\
         ask.budget_usd = <not set>  (default)\n\
         disk.floor_gib = 5  (default)\n\
         claude.check_published = true  (default)\n"
    );
}

#[test]
fn a_malformed_user_file_exits_1_naming_the_file_and_prints_nothing_on_standard_output() {
    let root = TempRoot::new();
    let _guard = DaemonGuard(DaemonPaths::under(root.path()));
    write_user_config(&root, "disk: [unclosed\n");
    let output = collide(&root, &["config", "get"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(
        text(&output.stderr).contains("config.yaml"),
        "{}",
        text(&output.stderr)
    );
}

#[test]
fn a_bad_command_line_exits_2_with_the_usage_and_starts_no_daemon() {
    let root = TempRoot::new();
    let output = collide(&root, &["config", "get", "a", "b"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(text(&output.stderr).contains("usage: collide config get"));
    assert!(daemon_status(&DaemonPaths::under(root.path())).is_none());
}

#[test]
fn set_and_a_bare_config_are_still_the_stub_with_the_new_stage() {
    let root = TempRoot::new();
    for args in [
        &["config", "set", "disk.floor_gib", "3"][..],
        &["config"][..],
    ] {
        let output = collide(&root, args);
        assert_eq!(output.status.code(), Some(6), "{args:?}");
        assert_eq!(
            text(&output.stderr),
            "collide config: not built yet (cli: config, Stage 4a)\n",
            "{args:?}"
        );
    }
}

#[test]
fn help_names_the_get_command() {
    let root = TempRoot::new();
    let output = collide(&root, &["--help"]);
    assert!(text(&output.stdout).contains("usage: collide config get [<key>] [--profile <name>]"));
}
