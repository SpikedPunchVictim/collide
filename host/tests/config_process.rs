//! `collide config get` as a process (ts-0098-nx): the real `collide` binary, which starts a real `collide-daemon`
//! under the test's own root. What needs a checkout and packs is in `config_command.rs`; here are the cases that need
//! neither: the user's layers outside a checkout, the usage, the problems of a user file, and `set` (ts-0099-bw), whose
//! secret goes in on standard input and into the daemon's keychain, which under a test's root is in memory, so no
//! test reaches the OS keychain. Every process has a cleared environment with `HOME`, `COLLIDE_DAEMON_ROOT` and
//! `SHELL` in the test's own folder under `/tmp`, runs in that folder (which is not in a repository), and the daemon
//! is stopped at the end by the pid it reports on the test's own socket.

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
        "ask.agent = \"claude-haiku\"  (default)\n\
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
fn a_bare_config_or_an_unknown_word_exits_2_with_both_usages_and_starts_no_daemon() {
    let root = TempRoot::new();
    for args in [&["config"][..], &["config", "nothing"][..]] {
        let output = collide(&root, args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        let stderr = text(&output.stderr);
        assert!(stderr.contains("usage: collide config get"), "{stderr}");
        assert!(stderr.contains("usage: collide config set"), "{stderr}");
    }
    assert!(daemon_status(&DaemonPaths::under(root.path())).is_none());
}

/// A value no file, log or argument of these tests may hold, other than the one file it is piped from.
const CANARY: &str = "canary-3d58a1-never-written";

#[test]
fn a_secret_on_the_command_line_exits_2_and_starts_no_daemon() {
    let root = TempRoot::new();
    let output = collide(
        &root,
        &[
            "config",
            "set",
            "agents.mine.secrets.SOME_API_KEY",
            CANARY,
            "--secret",
        ],
    );
    assert_eq!(output.status.code(), Some(2));
    let stderr = text(&output.stderr);
    assert!(stderr.contains("standard input"), "{stderr}");
    assert!(!stderr.contains(CANARY), "{stderr}");
    assert!(daemon_status(&DaemonPaths::under(root.path())).is_none());
}

/// `collide <args>` with the file `stdin` as its standard input, as `collide` above otherwise.
fn collide_reading(root: &TempRoot, args: &[&str], stdin: &std::path::Path) -> Output {
    let mut command = Command::new("/bin/sh");
    command
        .arg("-c")
        .arg(format!("exec \"$0\" \"$@\" < '{}'", stdin.display()))
        .arg(COLLIDE)
        .args(args)
        .current_dir(root.path())
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", root.path())
        .env("SHELL", fake_shell(root))
        .env("COLLIDE_DAEMON_ROOT", root.path());
    common::detached::Detached::start(root, &command).wait()
}

/// The files under `dir` that hold `needle`.
fn holding(dir: &std::path::Path, needle: &str, found: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let kind = entry.file_type().unwrap();
        if kind.is_dir() {
            holding(&path, needle, found);
        } else if kind.is_file()
            && fs::read(&path)
                .is_ok_and(|bytes| bytes.windows(needle.len()).any(|w| w == needle.as_bytes()))
        {
            found.push(path);
        }
    }
}

#[test]
fn a_secret_from_standard_input_reaches_the_daemons_keychain_and_no_file() {
    let root = TempRoot::new();
    let _guard = DaemonGuard(DaemonPaths::under(root.path()));
    write_user_config(
        &root,
        "agents:\n  mine:\n    kind: external\n    secrets: { SOME_API_KEY: my-item }\n",
    );
    // The item is missing, so the load refuses the entry and names the way out.
    let before = collide(&root, &["config", "get"]);
    assert_eq!(before.status.code(), Some(1));
    assert!(
        text(&before.stderr).contains("agents.mine.secrets.SOME_API_KEY"),
        "{}",
        text(&before.stderr)
    );

    let stdin = root.join("stdin.txt");
    fs::write(&stdin, format!("{CANARY}\n")).unwrap();
    let set = collide_reading(
        &root,
        &[
            "config",
            "set",
            "agents.mine.secrets.SOME_API_KEY",
            "--secret",
        ],
        &stdin,
    );
    assert_eq!(set.status.code(), Some(0), "{}", text(&set.stderr));
    assert_eq!(
        text(&set.stdout),
        "agents.mine.secrets.SOME_API_KEY = <secret, in the keychain>  (user)\n"
    );

    let after = collide(&root, &["config", "get"]);
    assert_eq!(after.status.code(), Some(0), "{}", text(&after.stderr));

    let mut found = Vec::new();
    holding(root.path(), CANARY, &mut found);
    // The file it was piped from is the one place it is meant to be; `detached-N` hold the commands' own output, which
    // is checked below for the two that took the secret.
    found.retain(|path| path != &stdin && !path.to_string_lossy().contains("/detached-"));
    assert!(found.is_empty(), "the value is in {found:?}");
    assert!(!text(&set.stdout).contains(CANARY) && !text(&set.stderr).contains(CANARY));
}

#[test]
fn a_plain_value_is_written_through_the_real_binary() {
    let root = TempRoot::new();
    let _guard = DaemonGuard(DaemonPaths::under(root.path()));
    let output = collide(&root, &["config", "set", "disk.floor_gib", "9"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert_eq!(text(&output.stdout), "disk.floor_gib = 9  (user)\n");
    let got = collide(&root, &["config", "get", "disk.floor_gib"]);
    assert_eq!(text(&got.stdout), "disk.floor_gib = 9  (user)\n");
}

#[test]
fn help_names_the_get_and_set_commands() {
    let root = TempRoot::new();
    let output = collide(&root, &["--help"]);
    let help = text(&output.stdout);
    assert!(help.contains("usage: collide config get [<key>] [--profile <name>]"));
    assert!(help.contains("usage: collide config set <key> <value>"));
    assert!(help.contains("collide config set <key> --secret"));
}
