//! Runs a process to its end, within a time (ts-0068-wx): the version read, the smoke check, the proxy lookup and the
//! published-version fetch each run one short process of the daemon's own and must not wait on it for ever.

use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// The last 200 characters of what a process wrote, for a message.
pub fn tail(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let text = text.trim();
    let skip = text.chars().count().saturating_sub(200);
    text.chars().skip(skip).collect()
}

/// What a run says when [`GroupSessions::end_all`] ended it, or came before it could start.
pub const ENDED_REASON: &str = "the daemon is stopping, so the run was ended";

/// The process groups of the runs in flight, so that a daemon that stops ends them (ts-0165-ab's way, for the version
/// read). A run in its own group is not in the daemon's, and the daemon's exit does not reach it: one still in flight
/// would outlive the daemon.
#[derive(Debug, Default)]
pub struct GroupSessions {
    state: Mutex<GroupState>,
}

#[derive(Debug, Default)]
struct GroupState {
    /// The pid of each leader that has not been waited for. It is the id of the leader's process group.
    live: BTreeSet<libc::pid_t>,
    /// Set by [`GroupSessions::end_all`]; no run starts after it.
    ended: bool,
}

impl GroupSessions {
    /// Kills the group of every run in flight, and refuses the runs that come after.
    pub fn end_all(&self) {
        let mut state = self.lock();
        state.ended = true;
        for pid in &state.live {
            kill_group(*pid);
        }
    }

    fn lock(&self) -> MutexGuard<'_, GroupState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn is_ended(&self) -> bool {
        self.lock().ended
    }

    /// Records a leader that was just started. False when the sessions have ended, and the leader is not recorded.
    fn add(&self, pid: u32) -> bool {
        let mut state = self.lock();
        if state.ended {
            return false;
        }
        state.live.insert(pid as libc::pid_t);
        true
    }

    /// `child.try_wait()`, and forgets the child once it has ended. The wait is done under the lock that
    /// [`GroupSessions::end_all`] takes, so a pid in `live` is one that has not been waited for: nothing else can have
    /// taken it, and the kill never reaches a group that is not the child's.
    fn try_wait(&self, child: &mut Child) -> std::io::Result<Option<ExitStatus>> {
        let mut state = self.lock();
        let status = child.try_wait();
        if !matches!(status, Ok(None)) {
            state.live.remove(&(child.id() as libc::pid_t));
        }
        status
    }

    /// Ends the child's group, waits for the child and forgets it, all under the lock (see `try_wait`).
    fn end_and_wait(&self, child: &mut Child) {
        let mut state = self.lock();
        kill_group(child.id() as libc::pid_t);
        let _ = child.kill();
        let _ = child.wait();
        state.live.remove(&(child.id() as libc::pid_t));
    }
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
    command: Command,
    stdin: Option<&[u8]>,
    timeout: Duration,
) -> Result<Captured, String> {
    run(command, stdin, timeout, None)
}

/// [`run_bounded`] for a process that may start children of its own (a `claude` runs a runtime that does): it runs in a
/// process group of its own, and a run that is cut ends the whole group, not only the process, so no child outlives
/// the wait holding the pipes. The group is the child's own, so killing it is safe.
pub fn run_bounded_in_group(
    command: Command,
    stdin: Option<&[u8]>,
    timeout: Duration,
) -> Result<Captured, String> {
    run(command, stdin, timeout, Some(&GroupSessions::default()))
}

/// [`run_bounded_in_group`] with the group kept in `sessions`, so that [`GroupSessions::end_all`] ends it while it
/// runs. A run that was ended says so ([`ENDED_REASON`]); one that comes after `end_all` starts nothing.
pub fn run_bounded_tracked(
    command: Command,
    stdin: Option<&[u8]>,
    timeout: Duration,
    sessions: &GroupSessions,
) -> Result<Captured, String> {
    run(command, stdin, timeout, Some(sessions))
}

fn run(
    mut command: Command,
    stdin: Option<&[u8]>,
    timeout: Duration,
    group: Option<&GroupSessions>,
) -> Result<Captured, String> {
    if let Some(sessions) = group {
        if sessions.is_ended() {
            return Err(ENDED_REASON.to_string());
        }
        std::os::unix::process::CommandExt::process_group(&mut command, 0);
    }
    command
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|error| error.to_string())?;
    // The sessions can have ended between the check above and the start.
    if let Some(sessions) = group
        && !sessions.add(child.id())
    {
        kill_group(child.id() as libc::pid_t);
        let _ = child.kill();
        let _ = child.wait();
        return Err(ENDED_REASON.to_string());
    }
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
        let waited = match group {
            Some(sessions) => sessions.try_wait(&mut child),
            None => child.try_wait(),
        };
        match waited {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if Instant::now() >= until => {
                match group {
                    Some(sessions) => sessions.end_and_wait(&mut child),
                    None => {
                        let _ = child.kill();
                        let _ = child.wait();
                    }
                }
                break Err(format!("it did not end within {} s", timeout.as_secs_f32()));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(error) => break Err(error.to_string()),
        }
    };
    // A run that the end of the sessions killed says so, not that a signal ended it.
    if let (Some(sessions), Ok(status)) = (group, &status)
        && status.code().is_none()
        && sessions.is_ended()
    {
        return Err(ENDED_REASON.to_string());
    }
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

/// Ends the process group `leader` leads, which `process_group(0)` made for it: its id is the group's. The leader is
/// the caller's own child, not yet waited for, so its pid cannot name another group.
fn kill_group(leader: libc::pid_t) {
    // SAFETY: `killpg` takes two integers. The group is the one this run's child leads, and the child has not been
    // waited for, so the id still names that group.
    unsafe {
        libc::killpg(leader, libc::SIGKILL);
    }
}
