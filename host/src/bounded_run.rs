//! Runs a process to its end, within a time (ts-0068-wx): the version read, the smoke check, the proxy lookup and the
//! published-version fetch each run one short process of the daemon's own and must not wait on it for ever.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The last 200 characters of what a process wrote, for a message.
pub fn tail(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let text = text.trim();
    let skip = text.chars().count().saturating_sub(200);
    text.chars().skip(skip).collect()
}

/// How long a run waits for its pipes to close once the process itself has ended.
const PIPE_GRACE: Duration = Duration::from_secs(2);

/// What a bounded run left. A pipe that was not read to its end within [`PIPE_GRACE`] of the process ending (a
/// grandchild still holds it, or its reader was not scheduled in time on a busy machine) is flagged cut, and its bytes
/// are empty: empty output with a cut flag is not an empty reply, and a caller that reads that pipe must say so.
pub struct Captured {
    pub code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub stdout_cut: bool,
    pub stderr_cut: bool,
}

/// What a caller says when the output it needs was cut: `what` is the pipe, such as "stdout".
pub fn cut_message(what: &str) -> String {
    format!(
        "its {what} was not read in time: a process it started may still hold it open, or the machine was too busy"
    )
}

/// Runs `command` with `stdin` written to it and closed, reading stdout and stderr, and ends it if it is not done
/// within `timeout`. The input is written while the output is read, so neither waits on the other. The process is the
/// caller's own child, so killing it is safe.
pub fn run_bounded(
    mut command: Command,
    stdin: Option<&[u8]>,
    timeout: Duration,
) -> Result<Captured, String> {
    command
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|error| error.to_string())?;
    if let (Some(bytes), Some(mut pipe)) = (stdin, child.stdin.take()) {
        // Written on a thread of its own, so input larger than a pipe holds cannot block the reads below or the
        // timeout. A child that ended before it read its turn leaves a broken pipe; its exit says what happened.
        let bytes = bytes.to_vec();
        std::thread::spawn(move || {
            let _ = pipe.write_all(&bytes);
        });
    }
    let read = |mut pipe: Box<dyn Read + Send>| {
        let (send, received) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = pipe.read_to_end(&mut bytes);
            let _ = send.send(bytes);
        });
        received
    };
    let stdout = read(Box::new(child.stdout.take().expect("piped")));
    let stderr = read(Box::new(child.stderr.take().expect("piped")));
    let until = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if Instant::now() >= until => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(format!("it did not end within {} s", timeout.as_secs_f32()));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(error) => break Err(error.to_string()),
        }
    };
    // A grandchild that kept the pipes open (a hook still running) must not hold the daemon: a pipe not read to its
    // end by then is given up on and flagged cut, and its reader ends when the grandchild does.
    let grace_ends = Instant::now() + PIPE_GRACE;
    let left = || grace_ends.saturating_duration_since(Instant::now());
    let stdout = stdout.recv_timeout(left()).ok();
    let stderr = stderr.recv_timeout(left()).ok();
    status.map(|status| Captured {
        code: status.code(),
        stdout_cut: stdout.is_none(),
        stderr_cut: stderr.is_none(),
        stdout: stdout.unwrap_or_default(),
        stderr: stderr.unwrap_or_default(),
    })
}
