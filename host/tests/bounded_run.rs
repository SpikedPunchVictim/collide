//! `run_bounded` (ts-0068-wx): a process with more input than a pipe holds still ends, and its timeout still holds.

use collide_host::bounded_run::run_bounded;
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

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
