//! Starting the daemon (0.2, Starting the daemon; Stage 3a). In v0 the app and the CLI start it on demand when the
//! socket does not answer. This module is that start, in the steps the plan gives:
//!
//! 1. **An `flock` on the lock file serializes starts.** The lock file holds the socket path. A start waits up to
//!    [`LOCK_WAIT`], and the daemon adopts existing shims before it binds the socket ([`crate::daemon`]).
//! 2. **On macOS the daemon leaves its launcher's coalition**, because a `setsid` child stays in the coalition of the
//!    app or Terminal that started it (measured S2). The start is a one-shot launchd job: `launchctl bootstrap
//!    gui/<uid>` of a temporary plist, removed once the daemon runs ([`LaunchdJob`]). The plist's paths are absolute
//!    and canonical, and it sets `USER`, `HOME` and `PATH`, because `claude` fails with `api_error` without `USER`
//!    (measured S2). launchd starts the job in `/` with a bare environment.
//! 3. **Over SSH, or with no GUI session, `launchctl bootstrap` can fail** (inferred). The daemon then starts with
//!    `setsid`, outside the coalition, and the start returns a warning that it may end with its launcher
//!    ([`StartKind::SetsidFallback`]). The run is not refused.
//! 4. **The resident app restarts the daemon** when the socket stops answering, the same way, and after
//!    [`MAX_FAILED_STARTS`] failed starts in [`FAILED_START_WINDOW`] it stops trying ([`RestartPolicy`]).
//!
//! A shim whose child exits while the daemon is down starts the daemon through the same entry point, so a phase
//! boundary is not missed while the app is closed.
//!
//! | tag | function | what it does |
//! |---|---|---|
//! | `daemon: lock file start` | [`ensure_daemon`] | the lock, the wait, the socket probe and the choice between 2 and 3 |
//! | `daemon: launchd job` | [`launchd_plist`], [`bootstrap_launchd_job`] | the plist and its bootstrap, removed once the daemon runs |
//! | `daemon: setsid start` | [`start_with_setsid`] | the fallback start, with its warning |

use std::io;
use std::path::PathBuf;
use std::time::Duration;

/// How long a start waits for the lock file and then for the socket to answer (0.2: 15 s).
pub const LOCK_WAIT: Duration = Duration::from_secs(15);

/// Failed starts in [`FAILED_START_WINDOW`] after which the app stops trying and shows the error and the daemon's log
/// (0.2).
pub const MAX_FAILED_STARTS: u32 = 3;

/// The window the failed starts of [`MAX_FAILED_STARTS`] are counted in: five minutes.
pub const FAILED_START_WINDOW: Duration = Duration::from_secs(5 * 60);

/// The app's rule for starting a daemon again, as the two numbers it is made of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestartPolicy {
    pub max_failed_starts: u32,
    pub window: Duration,
}

impl Default for RestartPolicy {
    fn default() -> Self {
        RestartPolicy {
            max_failed_starts: MAX_FAILED_STARTS,
            window: FAILED_START_WINDOW,
        }
    }
}

/// What a start did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartKind {
    /// The socket answered, so nothing was started.
    AlreadyRunning,
    /// A one-shot launchd job started the daemon, in a coalition of its own.
    Launchd,
    /// launchd could not start it, so `setsid` did. `warning` is for the person: the daemon may end with its
    /// launcher.
    SetsidFallback { warning: String },
}

/// Why a start failed. The message names the path, the command or the limit it hit.
#[derive(Debug)]
pub enum StartError {
    /// The lock file could not be taken within [`LOCK_WAIT`].
    LockTimeout {
        lock_file: PathBuf,
    },
    /// The daemon was started and its socket did not answer within [`LOCK_WAIT`].
    NoAnswer {
        socket: PathBuf,
    },
    Io {
        context: String,
        source: io::Error,
    },
}

impl std::fmt::Display for StartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StartError::LockTimeout { lock_file } => write!(
                f,
                "the lock file {} was not free within {} s",
                lock_file.display(),
                LOCK_WAIT.as_secs()
            ),
            StartError::NoAnswer { socket } => write!(
                f,
                "the daemon was started and {} did not answer within {} s",
                socket.display(),
                LOCK_WAIT.as_secs()
            ),
            StartError::Io { context, source } => write!(f, "{context}: {source}"),
        }
    }
}

impl std::error::Error for StartError {}

/// What a start needs to know.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartRequest {
    /// The `collide-daemon` binary, as an absolute canonical path.
    pub daemon_exe: PathBuf,
    /// The lock file, which holds the socket path.
    pub lock_file: PathBuf,
    pub socket: PathBuf,
}

/// Makes sure a daemon answers on the socket: takes the lock, probes the socket, starts the daemon when it does not
/// answer ([`bootstrap_launchd_job`] on macOS, then [`start_with_setsid`] if that fails), and waits for the answer.
pub fn ensure_daemon(request: &StartRequest) -> Result<StartKind, StartError> {
    let _ = request;
    todo!("daemon: lock file start")
}

/// The one-shot launchd job: a label, the daemon and the three variables launchd's bare environment lacks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchdJob {
    /// The job's label, unique per start so that two starts never name one job.
    pub label: String,
    pub daemon_exe: PathBuf,
    pub user: String,
    pub home: PathBuf,
    /// The `PATH` the daemon starts with, before it captures the login shell's.
    pub path: String,
}

/// The job's plist, as text. The daemon runs in `/` and its paths are all absolute.
pub fn launchd_plist(job: &LaunchdJob) -> String {
    let _ = job;
    todo!("daemon: launchd job")
}

/// Writes the plist to a temporary file and runs `launchctl bootstrap gui/<uid>` on it. The caller removes the plist
/// once the daemon runs. A failure here is what makes the start fall back to `setsid`.
pub fn bootstrap_launchd_job(job: &LaunchdJob) -> io::Result<PathBuf> {
    let _ = job;
    todo!("daemon: launchd job")
}

/// Starts the daemon detached with `setsid`, outside the launcher's coalition only in the sense that it has a session
/// of its own. Returns the pid.
pub fn start_with_setsid(request: &StartRequest) -> io::Result<u32> {
    let _ = request;
    todo!("daemon: setsid start")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_start_waits_fifteen_seconds() {
        assert_eq!(LOCK_WAIT, Duration::from_secs(15));
    }

    #[test]
    fn the_app_gives_up_after_three_failed_starts_in_five_minutes() {
        let policy = RestartPolicy::default();
        assert_eq!(policy.max_failed_starts, 3);
        assert_eq!(policy.window, Duration::from_secs(300));
    }

    #[test]
    #[should_panic(expected = "daemon: lock file start")]
    fn ensure_daemon_is_a_stub_tagged_for_stage_3a() {
        let request = StartRequest {
            daemon_exe: PathBuf::from("/x/collide-daemon"),
            lock_file: PathBuf::from("/x/lock"),
            socket: PathBuf::from("/x/sock"),
        };
        let _ = ensure_daemon(&request);
    }
}
