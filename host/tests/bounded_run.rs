//! `run_bounded` (ts-0068-wx): a process with more input than a pipe holds still ends, and its timeout still holds.
//! A pipe that is not read to its end in time is flagged cut (ts-0154-gr).

use collide_host::bounded_run::run_bounded;
use std::process::Command;
use std::sync::mpsc;
use std::time::{Duration, Instant};

mod common;
use common::{Grandchild, TempRoot};

/// Runs `run_bounded` on a thread, so a run that never returns fails the test instead of hanging it.
fn within(
    limit: Duration,
    run: impl FnOnce() -> Result<usize, String> + Send + 'static,
) -> Result<usize, String> {
    let (send, received) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = send.send(run());
    });
    received
        .recv_timeout(limit)
        .unwrap_or_else(|_| panic!("run_bounded did not return within {limit:?}"))
}

fn cat() -> Command {
    let mut command = Command::new("cat");
    command.env_clear().env("PATH", "/bin:/usr/bin");
    command
}

#[test]
fn input_larger_than_a_pipe_is_echoed_back_whole() {
    let input = vec![b'x'; 4 * 1024 * 1024];
    let length = input.len();
    let read = within(Duration::from_secs(30), move || {
        run_bounded(cat(), Some(&input), Duration::from_secs(20))
            .map(|captured| captured.stdout.len())
    });
    assert_eq!(read, Ok(length));
}

#[test]
fn a_process_that_never_reads_its_input_is_ended_at_the_timeout() {
    let mut sleeper = Command::new("sleep");
    sleeper.env_clear().env("PATH", "/bin:/usr/bin").arg("60");
    let input = vec![b'x'; 4 * 1024 * 1024];
    let result = within(Duration::from_secs(20), move || {
        run_bounded(sleeper, Some(&input), Duration::from_secs(1))
            .map(|captured| captured.stdout.len())
    });
    let error = result.expect_err("a run past its time is an error");
    assert!(error.contains("did not end within"), "{error}");
}

/// A shell that runs `body`, with the pid of its background sleeper written to `pid_file` by the body.
fn shell(body: &str) -> Command {
    let mut command = Command::new("/bin/sh");
    command
        .env_clear()
        .env("PATH", "/bin:/usr/bin")
        .arg("-c")
        .arg(body);
    command
}

#[test]
fn a_stdout_a_grandchild_holds_open_is_flagged_cut() {
    let root = TempRoot::new();
    let pid_file = root.join("pid");
    let mut grandchild = Grandchild::new(pid_file.clone());
    // The sleeper is this test's own: it keeps stdout open and its stderr goes nowhere.
    let body = format!(
        "sleep 30 2>/dev/null &\necho $! > {}\necho hello",
        pid_file.display()
    );
    let started = Instant::now();
    let run = run_bounded(shell(&body), None, Duration::from_secs(20)).unwrap();
    let waited = started.elapsed();
    grandchild.pid(Duration::from_secs(10));
    assert_eq!(run.code, Some(0));
    assert!(
        run.stdout_cut,
        "stdout was not read to its end, and nothing says so"
    );
    assert!(!run.stderr_cut, "stderr was closed and read");
    assert!(
        waited < Duration::from_secs(10),
        "the grandchild held the run for {waited:?}"
    );
}

#[test]
fn a_stderr_a_grandchild_holds_open_is_flagged_cut_and_stdout_is_not() {
    let root = TempRoot::new();
    let pid_file = root.join("pid");
    let mut grandchild = Grandchild::new(pid_file.clone());
    let body = format!(
        "sleep 30 >/dev/null &\necho $! > {}\necho hello",
        pid_file.display()
    );
    let run = run_bounded(shell(&body), None, Duration::from_secs(20)).unwrap();
    grandchild.pid(Duration::from_secs(10));
    assert_eq!(run.code, Some(0));
    assert!(
        run.stderr_cut,
        "stderr was not read to its end, and nothing says so"
    );
    assert!(!run.stdout_cut, "stdout was closed and read");
    assert_eq!(String::from_utf8_lossy(&run.stdout), "hello\n");
}

#[test]
fn a_run_whose_pipes_close_has_neither_pipe_flagged() {
    let run = run_bounded(
        shell("echo out\necho err >&2"),
        None,
        Duration::from_secs(20),
    )
    .unwrap();
    assert_eq!(run.code, Some(0));
    assert!(!run.stdout_cut && !run.stderr_cut);
    assert_eq!(String::from_utf8_lossy(&run.stdout), "out\n");
    assert_eq!(String::from_utf8_lossy(&run.stderr), "err\n");
}
