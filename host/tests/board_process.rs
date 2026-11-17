//! `collide init` and `collide trust` as processes (ts-0102-vu): the real `collide` binary and a real `collide-daemon`
//! under the test's own root, in a checkout made there. The command's standard input is empty and not a terminal, so
//! what is tested is what a script gets: `init`, `trust --yes`, and the refusal of `trust` with no terminal. The
//! question at a terminal is in `board_cli.rs`. Every process has a cleared environment with `HOME`,
//! `COLLIDE_DAEMON_ROOT` and `SHELL` in the test's own folder, and the daemon is stopped at the end by the pid it
//! reports on the test's own socket.

mod common;

use collide_host::daemon::DaemonPaths;
use common::git_repo::GitRepo;
use common::{TempRoot, daemon_status, wait_until};
use std::fs;
use std::process::{Command, Output, Stdio};
use std::time::Duration;

const COLLIDE: &str = env!("CARGO_BIN_EXE_collide");
// Cargo builds the daemon for these tests only when a test names it.
const _DAEMON: &str = env!("CARGO_BIN_EXE_collide-daemon");

fn collide(repo: &GitRepo, args: &[&str]) -> Output {
    let root = repo.root();
    let shell = root.script("fake-shell", "#!/bin/sh\neval \"$4\"\n");
    let mut command = Command::new(COLLIDE);
    command
        .args(args)
        .current_dir(repo.work())
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", root.path())
        .env("SHELL", shell)
        .env("COLLIDE_DAEMON_ROOT", root.path())
        .env("COLLIDE_BUILTIN_PACKS", common::scratch(PACKS))
        .stdin(Stdio::null());
    common::detached::Detached::start(root, &command).wait()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// The scratch folder of the built-in packs bundle the daemon loads (`COLLIDE_BUILTIN_PACKS`), built by the test so
/// that no test needs the staged copy `pnpm build:packs` makes.
const PACKS: &str = "board_process_packs";

/// Stops the daemon the CLI started, by the pid it reports on this test's own socket, then lets go of the bundle.
struct DaemonGuard(DaemonPaths, #[allow(dead_code)] common::Bundle);

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

/// A checkout with one commit.
fn repo() -> (GitRepo, DaemonGuard) {
    let repo = GitRepo::new();
    repo.write("README.md", b"fixture\n");
    repo.commit_all("start");
    let guard = DaemonGuard(
        DaemonPaths::under(repo.root().path()),
        common::bundle(PACKS),
    );
    (repo, guard)
}

#[test]
fn init_writes_board_yaml_in_the_checkout_and_trust_yes_then_trusts_the_committed_file() {
    let (repo, _guard) = repo();
    let init = collide(&repo, &["init"]);
    assert_eq!(init.status.code(), Some(0), "{}", text(&init.stderr));
    assert!(text(&init.stdout).contains(".collide/board.yaml"));
    assert!(
        fs::read_to_string(repo.work().join(".collide/board.yaml"))
            .unwrap()
            .contains("packs: []")
    );
    repo.commit_all("share");
    let trust = collide(&repo, &["trust", "--yes"]);
    assert_eq!(trust.status.code(), Some(0), "{}", text(&trust.stderr));
    assert!(
        text(&trust.stdout).contains("Trusted line main"),
        "{}",
        text(&trust.stdout)
    );
    let again = collide(&repo, &["trust"]);
    assert_eq!(again.status.code(), Some(0), "{}", text(&again.stderr));
    assert!(text(&again.stdout).contains("already trusted"));
}

#[test]
fn trust_with_no_terminal_and_no_yes_exits_5_and_trusts_nothing() {
    let (repo, _guard) = repo();
    assert_eq!(collide(&repo, &["init"]).status.code(), Some(0));
    repo.commit_all("share");
    let refused = collide(&repo, &["trust"]);
    assert_eq!(refused.status.code(), Some(5), "{}", text(&refused.stdout));
    let stderr = text(&refused.stderr);
    assert!(
        stderr.contains("--yes") && stderr.contains("terminal"),
        "{stderr}"
    );
    let after = collide(&repo, &["trust"]);
    assert_eq!(
        after.status.code(),
        Some(5),
        "still not trusted: {}",
        text(&after.stdout)
    );
}

#[test]
fn a_bad_command_line_exits_2_with_the_usage_and_starts_no_daemon() {
    let root = TempRoot::new();
    for args in [
        &["init", "x"][..],
        &["trust", "--nope"][..],
        &["trust", "--line"][..],
    ] {
        let output = {
            let mut command = Command::new(COLLIDE);
            command
                .args(args)
                .current_dir(root.path())
                .env_clear()
                .env("PATH", "/usr/bin:/bin")
                .env("HOME", root.path())
                .env("COLLIDE_DAEMON_ROOT", root.path())
                .stdin(Stdio::null());
            common::detached::Detached::start(&root, &command).wait()
        };
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(text(&output.stderr).contains("usage: collide"), "{args:?}");
    }
    assert!(daemon_status(&DaemonPaths::under(root.path())).is_none());
}

#[test]
fn help_lists_init_and_trust_with_their_usage() {
    let root = TempRoot::new();
    let mut command = Command::new(COLLIDE);
    command
        .arg("--help")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", root.path())
        .env("COLLIDE_DAEMON_ROOT", root.path());
    let help = text(&command.output().unwrap().stdout);
    assert!(help.contains("usage: collide init [--dry-run]"), "{help}");
    assert!(
        help.contains("usage: collide trust [--line <ref>] [--yes]"),
        "{help}"
    );
}

/// `collide` as a process inside a run: `COLLIDE_RUN` in its environment makes the daemon see a run's agent.
fn collide_in_a_run(repo: &GitRepo, args: &[&str]) -> Output {
    let root = repo.root();
    let mut command = Command::new(COLLIDE);
    command
        .args(args)
        .current_dir(repo.work())
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", root.path())
        .env("COLLIDE_DAEMON_ROOT", root.path())
        .env("COLLIDE_BUILTIN_PACKS", common::scratch(PACKS))
        .env("COLLIDE_RUN", "run-0042-zz")
        .stdin(Stdio::null());
    common::detached::Detached::start(root, &command).wait()
}

#[test]
fn a_runs_agent_may_not_trust_add_switch_or_set_anything_for_the_person() {
    // 0.9: trust is the person's act. Adding a user pack trusts it and a link trusts its folder, so each write of the
    // person's packs, trust and settings is refused to a run's agent, as `board.trust` is.
    let (repo, _guard) = repo();
    assert_eq!(collide(&repo, &["init"]).status.code(), Some(0));
    repo.commit_all("share");
    let folder = repo.root().path().join("mine");
    fs::create_dir_all(&folder).unwrap();
    fs::write(
        folder.join("pack.yaml"),
        "apiVersion: collide/v0\nname: mine\n",
    )
    .unwrap();
    let dev = format!("dev:{}", folder.display());
    let folder_text = folder.display().to_string();
    for args in [
        &["trust", "--yes"][..],
        &["init"][..],
        &["pack", "add", &dev, "--yes"][..],
        &["pack", "link", "mine", &folder_text][..],
        &["pack", "enable", "mine"][..],
        &["pack", "disable", "mine"][..],
        &["pack", "update", "mine", "--yes"][..],
        &["config", "set", "disk.floor_gib", "1"][..],
    ] {
        let output = collide_in_a_run(&repo, args);
        let stderr = text(&output.stderr);
        assert_ne!(output.status.code(), Some(0), "{args:?}: {stderr}");
        assert!(
            stderr.contains("the agent of run run-0042-zz may not"),
            "{args:?}: {stderr}"
        );
    }
    let config = repo.root().path().join(".config/collide");
    for file in ["packs.yaml", "trust.yaml", "settings.json"] {
        assert!(!config.join(file).exists(), "{file} was written");
    }
}
