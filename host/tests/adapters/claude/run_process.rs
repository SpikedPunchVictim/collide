//! `collide run` as a process, against a `collide-daemon` process it starts itself (ts-0069-bn): the exit codes the help
//! lists, and `--resume` from a second command after the first was killed. Fake Claude is the `claude` the daemon's
//! login shell puts on PATH.
//!
//! SAFETY of these tests. Every process they start has `env_clear()`, with `HOME` and `COLLIDE_DAEMON_ROOT` in the
//! test's own folder under `/tmp`, so the daemon, its shims and fake Claude work only there, and no launchd job is
//! loaded (the CLI does not load one under `COLLIDE_DAEMON_ROOT`). `Guards` ends what is left, in its `Drop`: each
//! run's shim and child by identity (pid and start time), and the daemon by the pid it reports on this test's own socket.
//! Nothing here runs the real `claude`, reads the owner's files or reaches a network.

use super::common::detached::Detached;
use super::common::{TempRoot, daemon_status, kill_child_of, signal_if_same, wait_until};
use super::fake::{BinarySpec, ClaudeBinary, recording};
use super::file_run::git;
use collide_host::daemon::DaemonPaths;
use collide_host::file_run::PHASE_NAME;
use serde_json::Value;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::mpsc;
use std::time::Duration;

const COLLIDE: &str = env!("CARGO_BIN_EXE_collide");
// Cargo builds the daemon for these tests only when a test names it.
const _DAEMON: &str = env!("CARGO_BIN_EXE_collide-daemon");

struct Guards {
    paths: DaemonPaths,
}

impl Drop for Guards {
    fn drop(&mut self) {
        if let Ok(entries) = fs::read_dir(&self.paths.runs_dir) {
            for run in entries.flatten() {
                let phase_dir = run.path().join(PHASE_NAME);
                if let Ok(text) = fs::read_to_string(phase_dir.join("shim.json"))
                    && let Ok(value) = serde_json::from_str::<Value>(&text)
                    && let Some(identity) = collide_host::shim::ChildIdentity::from_json(&value)
                {
                    signal_if_same(identity.pid, identity.started_unix_us, libc::SIGKILL);
                }
                kill_child_of(&phase_dir);
            }
        }
        if let Some(pid) = daemon_status(&self.paths).and_then(|s| s["pid"].as_i64()) {
            // SAFETY: `kill` takes two integers; the pid is the one the daemon on this test's own socket reports.
            unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
            wait_until("the daemon ends", Duration::from_secs(10), || {
                // SAFETY: signal 0 sends nothing.
                unsafe { libc::kill(pid as libc::pid_t, 0) != 0 }
            });
        }
    }
}

/// A temp root with a repository, a task file, a models file, and a login shell whose PATH holds fake Claude.
pub(super) struct Proc {
    // Dropped first: it ends the daemon while the root is still there.
    _guards: Guards,
    pub(super) root: TempRoot,
    pub(super) repo: PathBuf,
    pub(super) task: PathBuf,
    pub(super) shell: PathBuf,
}

impl Proc {
    pub(super) fn new(recording_path: &Path) -> Proc {
        let root = TempRoot::new();
        let bin = root.join("bin");
        fs::create_dir(&bin).unwrap();
        ClaudeBinary::write(&bin, &BinarySpec::new("2.1.325", recording_path));
        let shell = root.script(
            "fake-shell",
            &format!(
                "#!/bin/sh\nexport PATH={}:/usr/bin:/bin\neval \"$4\"\n",
                bin.display()
            ),
        );
        let home = root.join("home");
        fs::create_dir_all(home.join(".config/collide")).unwrap();
        fs::write(
            home.join(".config/collide/models.yaml"),
            "tools:\n  claude:\n    fast: { name: haiku }\n",
        )
        .unwrap();
        let repo = root.join("repo");
        fs::create_dir(&repo).unwrap();
        git(&repo, &["init", "-q", "-b", "main"]);
        fs::write(repo.join("README"), "hello\n").unwrap();
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-q", "-m", "start"]);
        let task = root.join("task.md");
        fs::write(&task, "Add a line to README.\n").unwrap();
        Proc {
            _guards: Guards {
                paths: DaemonPaths::under(root.path()),
            },
            repo: fs::canonicalize(repo).unwrap(),
            task,
            shell,
            root,
        }
    }

    pub(super) fn command(&self, args: &[&str], cwd: &Path) -> Command {
        let mut command = Command::new(COLLIDE);
        command
            .args(args)
            .current_dir(cwd)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", self.root.join("home"))
            .env("SHELL", &self.shell)
            .env("COLLIDE_DAEMON_ROOT", self.root.path())
            .stdin(Stdio::null());
        command
    }

    /// `collide <args>` in `cwd`, started so that this process is not among its parents (`common::detached`): a test run
    /// inside a run of Collide must not be seen as its run's agent.
    pub(super) fn start(&self, args: &[&str], cwd: &Path) -> Detached {
        Detached::start(&self.root, &self.command(args, cwd))
    }

    /// `collide <args>` in the repository, to its end.
    pub(super) fn collide(&self, args: &[&str]) -> Output {
        self.start(args, &self.repo).wait()
    }

    fn daemon_runs(&self) -> bool {
        daemon_status(&DaemonPaths::under(self.root.path())).is_some()
    }
}

pub(super) fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

pub(super) fn task_of(proc: &Proc) -> String {
    proc.task.to_str().unwrap().to_string()
}

#[test]
fn a_run_through_a_daemon_the_command_starts_prints_the_version_and_the_record_and_exits_0() {
    let proc = Proc::new(&recording("report"));
    let output = proc.collide(&["run", &task_of(&proc)]);
    let (out, err) = (text(&output.stdout), text(&output.stderr));
    assert_eq!(output.status.code(), Some(0), "{out}\n{err}");
    assert!(out.contains("claude in use: 2.1.325"), "{out}");
    assert!(
        !out.contains("published"),
        "a daemon under COLLIDE_DAEMON_ROOT fetches no published version: {out}"
    );
    assert!(out.contains(": ended\n"), "{out}");
    assert!(
        out.contains("The notes start with the first line."),
        "{out}"
    );
    assert!(out.contains("turn 1: wall "), "{out}");
    assert!(out.contains("cost $0.0041"), "{out}");
    assert!(proc.daemon_runs(), "the command started the daemon");
}

#[test]
fn a_command_line_that_is_wrong_exits_2_and_starts_no_daemon() {
    let proc = Proc::new(&recording("report"));
    for args in [
        &["run"][..],
        &["run", "a.md", "b.md"],
        &["run", "a.md", "--budget"],
        &["run", "a.md", "--budget", "-3"],
        &["run", "--resume", "r", "a.md"],
    ] {
        let output = proc.collide(args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(
            text(&output.stderr).contains("usage: collide run"),
            "{args:?}"
        );
    }
    assert!(!proc.daemon_runs());
}

#[test]
fn a_task_file_that_is_not_there_exits_3_and_starts_no_daemon() {
    let proc = Proc::new(&recording("report"));
    let output = proc.collide(&["run", "nothing-here.md"]);
    assert_eq!(output.status.code(), Some(3), "{}", text(&output.stderr));
    assert!(text(&output.stderr).contains("no task file at"));
    assert!(!proc.daemon_runs());
}

#[test]
fn a_task_outside_any_checkout_exits_1() {
    let proc = Proc::new(&recording("report"));
    let loose = proc.root.join("loose");
    fs::create_dir(&loose).unwrap();
    fs::write(loose.join("task.md"), "Do it.\n").unwrap();
    let output = proc.start(&["run", "task.md"], &loose).wait();
    assert_eq!(output.status.code(), Some(1), "{}", text(&output.stderr));
    assert!(text(&output.stderr).contains("git repository"));
}

#[test]
fn an_alias_with_no_mapping_exits_5_and_shows_the_mapping_to_add() {
    let proc = Proc::new(&recording("report"));
    let output = proc.collide(&["run", &task_of(&proc), "--alias", "nosuch"]);
    assert_eq!(output.status.code(), Some(5), "{}", text(&output.stderr));
    let err = text(&output.stderr);
    assert!(err.contains("refused"), "{err}");
    assert!(err.contains("nosuch: { name: haiku }"), "{err}");
    assert!(err.contains(".config/collide/models.yaml"), "{err}");
}

#[test]
fn a_phase_that_fails_exits_7_and_prints_its_record() {
    let scratch = TempRoot::new();
    let no_result = scratch.join("no-result.jsonl");
    fs::write(
        &no_result,
        [
            r#"{"expect":{"type":"user"}}"#,
            r#"{"say":{"type":"system","subtype":"init","session_id":"7c1b0e52-9f3a-4d2e-8b61-0a5d3c9e4f17","cwd":"/work","tools":[],"mcp_servers":[],"model":"m","permissionMode":"default","apiKeySource":"none"}}"#,
            r#"{"exit":0}"#,
        ]
        .join("\n"),
    )
    .unwrap();
    let proc = Proc::new(&no_result);
    let output = proc.collide(&["run", &task_of(&proc)]);
    let out = text(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(7),
        "{out}\n{}",
        text(&output.stderr)
    );
    assert!(out.contains(": failed\n"), "{out}");
    assert!(out.contains("failure: no_result"), "{out}");
}

/// `report.jsonl` that waits `secs` after it has read the turn, so a test can act while the phase runs.
pub(super) fn slowed_report(root: &TempRoot, secs: u32) -> PathBuf {
    let text = fs::read_to_string(recording("report")).unwrap();
    let mut lines = Vec::new();
    let mut inserted = false;
    for line in text.lines() {
        lines.push(line.to_string());
        if !inserted && line.starts_with("{\"expect\"") {
            lines.push(format!("{{\"sleep\": {secs}}}"));
            inserted = true;
        }
    }
    let path = root.join("slow-report.jsonl");
    fs::write(&path, lines.join("\n")).unwrap();
    path
}

/// A command's output lines, read as they come.
pub(super) fn lines_of(child: &mut Child) -> mpsc::Receiver<String> {
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                return;
            }
        }
    });
    rx
}

#[test]
fn a_run_goes_on_when_the_command_is_killed_and_resume_follows_it_to_its_end() {
    let scratch = TempRoot::new();
    let slow = slowed_report(&scratch, 3);
    let proc = Proc::new(&slow);
    let first = proc.start(&["run", &task_of(&proc)], &proc.repo);
    let lines = first.lines();
    let mut run = None;
    while run.is_none() {
        let line = lines
            .recv_timeout(Duration::from_secs(60))
            .unwrap_or_else(|_| {
                first.kill();
                panic!("the command never said it started")
            });
        if let Some(rest) = line.strip_prefix("run ")
            && let Some((id, _)) = rest.split_once(" started on ")
        {
            run = Some(id.to_string());
        }
    }
    let run = run.unwrap();
    // The command ends here, as a closed terminal ends it. This is the test's own child.
    first.kill();
    first.wait();

    let second = proc.collide(&["run", "--resume", &run]);
    let out = text(&second.stdout);
    assert_eq!(
        second.status.code(),
        Some(0),
        "{out}\n{}",
        text(&second.stderr)
    );
    assert!(out.contains(": ended\n"), "{out}");
    assert!(
        out.contains("The notes start with the first line."),
        "{out}"
    );
    assert!(out.contains("turn 1: wall "), "{out}");
}
