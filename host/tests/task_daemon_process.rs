//! The new forms of `collide task` and `collide tasks` as processes (ts-0120-wm): the real `collide` binary, which
//! starts a real `collide-daemon` under the test's own root when none runs. What is tested here is what only a
//! process shows: the daemon started on use and the one line that says so, a daemon that cannot start, the help text
//! against a stored copy, and the option errors of a bad command line. The commands over a daemon with a plan source
//! are in `task_daemon_cli.rs`. Every process has a cleared environment with `HOME`, `COLLIDE_DAEMON_ROOT` and `SHELL`
//! in the test's own folder, and a daemon that a test started is stopped at the end by the pid it reports on the
//! test's own socket.

mod common;

use collide_host::daemon::DaemonPaths;
use common::git_repo::GitRepo;
use common::{DaemonStop, daemon_status};
use std::fs;
use std::process::{Command, Output, Stdio};

const COLLIDE: &str = env!("CARGO_BIN_EXE_collide");
// Cargo builds the daemon for these tests only when a test names it.
const _DAEMON: &str = env!("CARGO_BIN_EXE_collide-daemon");

/// The built-in packs bundle the daemon loads (`COLLIDE_BUILTIN_PACKS`), built by the test.
const PACKS: &str = "task_daemon_process_packs";

/// The lines and blocks of the help and the usage that the item adds, as stored.
const STORED_HELP: &str = include_str!("fixtures/task-cli-help.txt");

/// `_stop` is first so that it is dropped first: it finds the daemon by the socket in the repository's root.
struct Env {
    _stop: DaemonStop,
    repo: GitRepo,
    /// Kept so the packs bundle lives as long as the daemon.
    _bundle: common::Bundle,
}

impl Env {
    fn new() -> Env {
        let repo = GitRepo::new();
        repo.write("README.md", b"fixture\n");
        repo.commit_all("start");
        let stop = DaemonStop(DaemonPaths::under(repo.root().path()));
        Env {
            _stop: stop,
            repo,
            _bundle: common::bundle(PACKS),
        }
    }

    fn paths(&self) -> DaemonPaths {
        DaemonPaths::under(self.repo.root().path())
    }

    /// `collide <args>` through `binary`, in the checkout, with an environment of the test's own.
    fn run_with(&self, binary: &str, args: &[&str]) -> Output {
        self.run_on(binary, "/usr/bin:/bin", args)
    }

    /// As `run_with`, with the given `PATH`.
    fn run_on(&self, binary: &str, path: &str, args: &[&str]) -> Output {
        let root = self.repo.root();
        let shell = root.script("fake-shell", "#!/bin/sh\neval \"$4\"\n");
        let mut command = Command::new(binary);
        command
            .args(args)
            .current_dir(self.repo.work())
            .env_clear()
            .env("PATH", path)
            .env("HOME", root.path())
            .env("SHELL", shell)
            .env("COLLIDE_DAEMON_ROOT", root.path())
            .env("COLLIDE_BUILTIN_PACKS", common::scratch(PACKS))
            .stdin(Stdio::null());
        common::detached::Detached::start(root, &command).wait()
    }

    fn collide(&self, args: &[&str]) -> Output {
        self.run_with(COLLIDE, args)
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// `PATH` for a command that reads the plan directly: the system's, and the folder of the `node` that is on the
/// test's own `PATH` (the plan provider is a Node program).
fn with_node() -> String {
    let own = std::env::var_os("PATH").unwrap_or_default();
    let node = std::env::split_paths(&own)
        .find(|dir| dir.join("node").is_file())
        .expect("node is on PATH");
    format!("/usr/bin:/bin:{}", node.display())
}

const STARTED: &str = "the daemon was not running; started it";

#[test]
fn task_new_starts_the_daemon_says_so_once_and_prints_the_key() {
    let env = Env::new();
    assert!(daemon_status(&env.paths()).is_none(), "no daemon runs yet");

    let first = env.collide(&["task", "new", "try the cache"]);

    assert_eq!(first.status.code(), Some(0), "{}", text(&first.stderr));
    assert_eq!(text(&first.stdout), "scratch:1\n");
    assert_eq!(
        text(&first.stderr).matches(STARTED).count(),
        1,
        "{}",
        text(&first.stderr)
    );
    assert!(
        daemon_status(&env.paths()).is_some(),
        "the daemon the command started is the temp one"
    );
}

#[test]
fn the_second_command_finds_the_daemon_running_and_does_not_say_it_started_one() {
    let env = Env::new();
    env.collide(&["task", "new", "first"]);

    let second = env.collide(&["task", "new", "second"]);

    assert_eq!(second.status.code(), Some(0), "{}", text(&second.stderr));
    assert_eq!(text(&second.stdout), "scratch:2\n");
    assert!(
        !text(&second.stderr).contains("started"),
        "{}",
        text(&second.stderr)
    );
}

#[test]
fn a_task_new_with_json_prints_one_document_and_the_note_goes_to_standard_error() {
    let env = Env::new();
    let run = env.collide(&["task", "new", "try the cache", "--json"]);
    assert_eq!(run.status.code(), Some(0), "{}", text(&run.stderr));
    let document: serde_json::Value = serde_json::from_slice(&run.stdout).unwrap();
    assert_eq!(document["task"]["key"], "scratch:1");
    assert!(text(&run.stderr).contains(STARTED));
}

#[test]
fn when_the_daemon_cannot_start_the_new_forms_exit_1_and_the_old_ones_still_work() {
    let env = Env::new();
    // A copy of `collide` with no `collide-daemon` beside it: the start has nothing to run.
    let lonely = env.repo.root().path().join("lonely-bin");
    fs::create_dir_all(&lonely).unwrap();
    let copy = lonely.join("collide");
    fs::copy(COLLIDE, &copy).unwrap();
    let copy = copy.to_str().unwrap();

    let new = env.run_with(copy, &["task", "new", "try the cache"]);
    let view = env.run_with(copy, &["tasks", "--view", "mine"]);
    let direct = env.run_on(copy, &with_node(), &["tasks"]);

    for run in [&new, &view] {
        assert_eq!(run.status.code(), Some(1), "{}", text(&run.stderr));
        assert!(text(&run.stdout).is_empty());
        assert!(
            text(&run.stderr).contains("the daemon is not running and did not start"),
            "{}",
            text(&run.stderr)
        );
    }
    assert!(daemon_status(&env.paths()).is_none());
    assert_ne!(direct.status.code(), Some(1), "{}", text(&direct.stderr));
    assert!(
        !text(&direct.stderr).contains("daemon"),
        "{}",
        text(&direct.stderr)
    );
}

#[test]
fn a_bad_command_line_exits_2_names_the_option_and_starts_no_daemon() {
    let env = Env::new();
    for (args, named) in [
        (vec!["tasks", "--view"], "--view"),
        (vec!["tasks", "--checkout"], "--checkout"),
        (vec!["tasks", "--viw", "x"], "--viw"),
        (vec!["task", "new"], "prompt"),
        (vec!["task", "new", "x", "--commit"], "--commit"),
        (vec!["task", "new", "x", "--no-wait"], "--no-wait"),
        (vec!["task", "new", "x", "--sideways"], "--sideways"),
    ] {
        let run = env.collide(&args);
        assert_eq!(
            run.status.code(),
            Some(2),
            "{args:?}: {}",
            text(&run.stderr)
        );
        assert!(
            text(&run.stderr).contains(named),
            "{args:?}: {}",
            text(&run.stderr)
        );
        assert!(
            text(&run.stderr).contains("usage:"),
            "{args:?}: {}",
            text(&run.stderr)
        );
    }
    assert!(daemon_status(&env.paths()).is_none());
}

#[test]
fn help_and_the_usage_text_hold_the_stored_lines_of_the_new_forms() {
    let env = Env::new();
    let help = text(&env.collide(&["help"]).stdout);
    let usage = text(&env.collide(&["task"]).stderr);
    let mut blocks = STORED_HELP.trim_end().split("\n\n");
    let usage_lines = blocks.next().unwrap();
    for line in usage_lines.lines() {
        assert!(help.contains(line), "{line} in the help: {help}");
        assert!(usage.contains(line), "{line} in the usage: {usage}");
    }
    for block in blocks {
        assert!(help.contains(block), "{block} in the help: {help}");
    }
    assert!(daemon_status(&env.paths()).is_none());
}

#[test]
fn every_new_option_is_in_help() {
    let env = Env::new();
    let help = text(&env.collide(&["help"]).stdout);
    for option in ["--view", "--checkout", "--no-wait", "--wait", "task new"] {
        assert!(help.contains(option), "{option} in {help}");
    }
}
