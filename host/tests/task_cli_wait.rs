//! The CLI's direct path waits for a busy checkout (ts-0114-hr; 0.4, Root busy): `collide task <id> <op>` in a
//! checkout that is in the middle of a rebase says why, once, waits, and applies when the rebase ends. `--no-wait`
//! refuses at once and `--wait <seconds>` sets the wait. The binary runs against a fixture repository in a temp root
//! (a copy of `host/tests/fixtures/task-cli`), on the real plan provider from a bundle built here, with a cleared
//! environment whose `HOME` and daemon root are that temp root. They need `node` on `PATH`, like the loader's tests.

mod common;

use collide_host::task_cli::{Command, TaskOptions, Wait, WriteOptions, parse_task};
use common::git_repo::{GitRepo, git_command};
use common::{Bundle, repo_root};
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Child, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Long enough for a loaded machine: every wait for the child has at least this.
const DEADLINE: Duration = Duration::from_secs(60);

const TASK_FILE: &str = ".collide/planning/epics/ep-001-aa-alpha/tasks/ts-0002-bb-second.md";

fn bundle() -> Bundle {
    common::bundle("task-cli-wait-packs")
}

fn copy_tree(from: &Path, repo: &GitRepo, to: &str) {
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().to_str().unwrap().to_string();
        let target = if to.is_empty() {
            name
        } else {
            format!("{to}/{name}")
        };
        if entry.path().is_dir() {
            copy_tree(&entry.path(), repo, &target);
        } else {
            repo.write(&target, &fs::read(entry.path()).unwrap());
        }
    }
}

/// A fixture repository, on `topic`, stopped in the middle of a rebase that two branches' edits of one line made.
fn repo_in_a_rebase() -> GitRepo {
    let repo = GitRepo::new();
    copy_tree(&repo_root().join("host/tests/fixtures/task-cli"), &repo, "");
    repo.write("line.txt", b"one\n");
    repo.commit_all("fixture");
    repo.git(&["checkout", "--quiet", "-b", "topic"]);
    repo.write("line.txt", b"topic\n");
    repo.commit_all("topic edits the line");
    repo.git(&["checkout", "--quiet", "main"]);
    repo.write("line.txt", b"main\n");
    repo.commit_all("main edits the line");
    repo.git(&["checkout", "--quiet", "topic"]);
    let rebase = git_command(repo.root().path(), repo.work())
        .args(["rebase", "main"])
        .output()
        .expect("git runs");
    assert!(
        !rebase.status.success(),
        "the rebase should stop on the conflict"
    );
    repo
}

fn status_on_disk(repo: &GitRepo) -> String {
    fs::read_to_string(repo.work().join(TASK_FILE))
        .unwrap()
        .lines()
        .find_map(|line| line.strip_prefix("status: "))
        .expect("the file has a status")
        .to_string()
}

/// A `collide` the test started, with its output read on threads. `Drop` kills it and reaps it, pass or fail.
struct Cli {
    child: Child,
    stdout: Arc<Mutex<String>>,
    stderr: Arc<Mutex<String>>,
    stderr_lines: Receiver<String>,
    started: Instant,
}

impl Cli {
    /// Runs `collide <args>` in the repository's checkout, with the environment cleared: `HOME` and the daemon root are
    /// the repository's temp root, git reads no real config, and the packs come from the test's bundle.
    fn start(repo: &GitRepo, packs: &Path, args: &[&str]) -> Cli {
        let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_collide"))
            .args(args)
            .current_dir(repo.work())
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", repo.root().path())
            .env("COLLIDE_DAEMON_ROOT", repo.root().path())
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("COLLIDE_BUILTIN_PACKS", packs)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("collide should start");
        let stdout = Arc::new(Mutex::new(String::new()));
        let mut pipe = child.stdout.take().unwrap();
        let held = stdout.clone();
        std::thread::spawn(move || {
            let mut text = String::new();
            let _ = pipe.read_to_string(&mut text);
            *held.lock().unwrap() = text;
        });
        let stderr = Arc::new(Mutex::new(String::new()));
        let (sender, stderr_lines) = channel();
        let pipe = child.stderr.take().unwrap();
        let held = stderr.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(pipe).lines().map_while(Result::ok) {
                let mut text = held.lock().unwrap();
                text.push_str(&line);
                text.push('\n');
                let _ = sender.send(line);
            }
        });
        Cli {
            child,
            stdout,
            stderr,
            stderr_lines,
            started: Instant::now(),
        }
    }

    /// Waits for a line of standard error that holds `part`, and returns it.
    fn wait_for_line(&self, part: &str) -> String {
        let until = Instant::now() + DEADLINE;
        loop {
            let left = until.saturating_duration_since(Instant::now());
            match self.stderr_lines.recv_timeout(left) {
                Ok(line) if line.contains(part) => return line,
                Ok(_) => {}
                Err(_) => panic!("no line with `{part}` in {DEADLINE:?}: {}", self.stderr()),
            }
        }
    }

    fn still_running(&mut self) -> bool {
        self.child.try_wait().unwrap().is_none()
    }

    /// Waits for the exit and returns the code.
    fn exit_code(&mut self) -> i32 {
        let until = Instant::now() + DEADLINE;
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                // Its pipes close with it; give the readers a moment to hand over what is left.
                let closed = Instant::now() + DEADLINE;
                while (Arc::strong_count(&self.stdout) > 1 || Arc::strong_count(&self.stderr) > 1)
                    && Instant::now() < closed
                {
                    std::thread::sleep(Duration::from_millis(10));
                }
                return status.code().expect("collide exits normally");
            }
            assert!(
                Instant::now() < until,
                "collide did not end in {DEADLINE:?}: {}",
                self.stderr()
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn stdout(&self) -> String {
        self.stdout.lock().unwrap().clone()
    }

    fn stderr(&self) -> String {
        self.stderr.lock().unwrap().clone()
    }
}

impl Drop for Cli {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn reason(repo: &GitRepo) -> String {
    let work = fs::canonicalize(repo.work()).unwrap();
    format!("a rebase is in progress in {}", work.display())
}

// ---- the options --------------------------------------------------------------------------------------------------

fn task_options(args: &[&str]) -> Result<TaskOptions, String> {
    let args: Vec<String> = args.iter().map(|word| word.to_string()).collect();
    match parse_task(&args) {
        Ok(Command::Task(options)) => Ok(options),
        Ok(other) => panic!("expected a task: {other:?}"),
        Err(error) => Err(error.message),
    }
}

#[test]
fn a_write_waits_by_default_for_ten_minutes() {
    let options = task_options(&["3", "setStatus", "active"]).unwrap();
    assert_eq!(options.write.wait, Wait::Default);
    assert_eq!(
        collide_host::task_cli::DEFAULT_WAIT,
        Duration::from_secs(600)
    );
}

#[test]
fn wait_takes_a_number_of_seconds_in_either_form_and_no_wait_takes_none() {
    for args in [
        ["3", "setStatus", "active", "--wait", "5"],
        ["3", "setStatus", "active", "--wait=5", ""],
    ] {
        let args: Vec<&str> = args.into_iter().filter(|word| !word.is_empty()).collect();
        assert_eq!(
            task_options(&args).unwrap().write.wait,
            Wait::Seconds(5),
            "{args:?}"
        );
    }
    let options = task_options(&["3", "setStatus", "active", "--no-wait"]).unwrap();
    assert_eq!(options.write.wait, Wait::Never);
}

#[test]
fn wait_and_no_wait_together_or_twice_or_with_no_number_are_usage_errors() {
    for args in [
        &["3", "setStatus", "active", "--wait", "5", "--no-wait"][..],
        &["3", "setStatus", "active", "--no-wait", "--wait", "5"],
        &["3", "setStatus", "active", "--wait", "5", "--wait", "6"],
        &["3", "setStatus", "active", "--no-wait", "--no-wait"],
        &["3", "setStatus", "active", "--wait"],
        &["3", "setStatus", "active", "--wait", "soon"],
        &["3", "setStatus", "active", "--wait", "-1"],
        &["3", "setStatus", "active", "--wait="],
    ] {
        assert!(task_options(args).is_err(), "{args:?}");
    }
}

#[test]
fn a_wait_option_without_an_op_is_a_usage_error_like_checkout_and_commit() {
    let message = task_options(&["3", "--no-wait"]).unwrap_err();
    assert!(message.contains("--no-wait"), "{message}");
    assert!(message.contains("are for a write"), "{message}");
    assert_eq!(WriteOptions::default().wait, Wait::Default);
}

#[test]
fn help_and_usage_name_the_wait_options() {
    assert!(collide_host::task_cli::USAGE.contains("[--wait <seconds> | --no-wait]"));
    assert!(collide_host::task_cli::HELP.contains("--no-wait"));
    assert!(collide_host::task_cli::HELP.contains("10 minutes"));
}

// ---- the binary ---------------------------------------------------------------------------------------------------

#[test]
fn a_write_in_a_rebase_says_why_once_waits_and_applies_after_the_rebase_ends() {
    let (bundle, repo) = (bundle(), repo_in_a_rebase());
    let before = status_on_disk(&repo);
    let mut cli = Cli::start(
        &repo,
        &bundle,
        &["task", "ts-0002-bb", "setStatus", "active"],
    );

    let line = cli.wait_for_line("is in progress in");
    assert_eq!(
        line,
        format!("collide: {}; waiting for it to end", reason(&repo))
    );
    assert!(cli.still_running(), "it waits in place");
    assert_eq!(
        status_on_disk(&repo),
        before,
        "nothing is written while the rebase goes on"
    );

    // Another thread ends the rebase, as a person in another terminal would.
    let ended = {
        let (root, work) = (repo.root().path().to_path_buf(), repo.work().to_path_buf());
        std::thread::spawn(move || {
            let abort = git_command(&root, &work)
                .args(["rebase", "--abort"])
                .output()
                .unwrap();
            assert!(abort.status.success());
        })
    };
    ended.join().unwrap();

    assert_eq!(cli.exit_code(), 0, "{}", cli.stderr());
    assert!(cli.stdout().starts_with("ok  "), "{}", cli.stdout());
    assert_eq!(status_on_disk(&repo), "active");
    assert_eq!(
        cli.stderr().matches("is in progress").count(),
        1,
        "the reason is said once: {}",
        cli.stderr()
    );
}

#[test]
fn no_wait_refuses_at_once_with_the_same_reason_and_writes_nothing() {
    let (bundle, repo) = (bundle(), repo_in_a_rebase());
    let before = status_on_disk(&repo);
    let mut cli = Cli::start(
        &repo,
        &bundle,
        &["task", "ts-0002-bb", "setStatus", "active", "--no-wait"],
    );

    assert_eq!(cli.exit_code(), 5);

    let stderr = cli.stderr();
    assert!(stderr.contains(&reason(&repo)), "{stderr}");
    assert!(stderr.contains("nothing was written"), "{stderr}");
    assert!(!stderr.contains("waiting"), "{stderr}");
    assert_eq!(status_on_disk(&repo), before);
}

#[test]
fn a_wait_of_one_second_ends_with_a_refusal_that_names_the_cause() {
    let (bundle, repo) = (bundle(), repo_in_a_rebase());
    let before = status_on_disk(&repo);
    let mut cli = Cli::start(
        &repo,
        &bundle,
        &["task", "ts-0002-bb", "setStatus", "active", "--wait", "1"],
    );

    assert_eq!(cli.exit_code(), 5);

    assert!(
        cli.started.elapsed() >= Duration::from_secs(1),
        "it waited the second out"
    );
    let stderr = cli.stderr();
    assert!(
        stderr.contains(&format!("{}; waiting for it to end", reason(&repo))),
        "{stderr}"
    );
    assert!(
        stderr.contains("did not end in 1 seconds, so nothing was written"),
        "{stderr}"
    );
    assert_eq!(status_on_disk(&repo), before);
}

#[test]
fn create_and_reid_wait_too_and_refuse_with_no_wait() {
    let (bundle, repo) = (bundle(), repo_in_a_rebase());
    let files_before = repo.git(&["status", "--porcelain"]);
    for args in [
        &[
            "task",
            "create",
            "A new task",
            "epic=ep-001-aa",
            "--no-wait",
        ][..],
        &["task", "reid", "ts-0002-bb", "--no-wait"],
    ] {
        let mut cli = Cli::start(&repo, &bundle, args);
        assert_eq!(cli.exit_code(), 5, "{args:?}: {}", cli.stderr());
        assert!(
            cli.stderr().contains(&reason(&repo)),
            "{args:?}: {}",
            cli.stderr()
        );
    }
    assert_eq!(repo.git(&["status", "--porcelain"]), files_before);
}

#[test]
fn a_read_never_waits_for_a_rebase() {
    let (bundle, repo) = (bundle(), repo_in_a_rebase());
    let mut cli = Cli::start(&repo, &bundle, &["task", "ts-0002-bb"]);
    assert_eq!(cli.exit_code(), 0, "{}", cli.stderr());
    assert!(!cli.stderr().contains("waiting"), "{}", cli.stderr());
}

#[test]
fn a_write_in_a_checkout_that_is_not_busy_goes_at_once() {
    let bundle = bundle();
    let repo = GitRepo::new();
    copy_tree(&repo_root().join("host/tests/fixtures/task-cli"), &repo, "");
    repo.commit_all("fixture");
    let mut cli = Cli::start(
        &repo,
        &bundle,
        &["task", "ts-0002-bb", "setStatus", "active"],
    );
    assert_eq!(cli.exit_code(), 0, "{}", cli.stderr());
    assert!(!cli.stderr().contains("waiting"), "{}", cli.stderr());
    assert_eq!(status_on_disk(&repo), "active");
}
