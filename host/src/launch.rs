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

use crate::daemon::DaemonPaths;
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// How long a start waits for the lock file and then for the socket to answer (0.2: 15 s).
pub const LOCK_WAIT: Duration = Duration::from_secs(15);

/// Failed starts in [`FAILED_START_WINDOW`] after which the app stops trying and shows the error and the daemon's log
/// (0.2).
pub const MAX_FAILED_STARTS: u32 = 3;

/// The window the failed starts of [`MAX_FAILED_STARTS`] are counted in: five minutes.
pub const FAILED_START_WINDOW: Duration = Duration::from_secs(5 * 60);

/// How often a start looks at the lock file and then at the socket while it waits.
const POLL: Duration = Duration::from_millis(20);

/// What the start says to the person when it used `setsid`: the plan's warning (0.2, Starting the daemon).
pub const SETSID_WARNING: &str = "the daemon was started with setsid: it is in a session of its own, but on macOS it may still end with the app or terminal that started it";

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
    /// The daemon ended before its socket answered. `log_tail` is the end of its log, which says why.
    Exited {
        status: String,
        log_file: PathBuf,
        log_tail: String,
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
            StartError::Exited {
                status,
                log_file,
                log_tail,
            } => write!(
                f,
                "the daemon ended ({status}) before its socket answered; its log {} ends: {log_tail}",
                log_file.display()
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
    /// The file the daemon's output is appended to, so a daemon that cannot start leaves its reason.
    pub log_file: PathBuf,
    /// Variables added to the daemon's environment, on top of the starter's own.
    pub env: BTreeMap<String, String>,
    /// How long to wait for the lock file, and then for the socket to answer ([`LOCK_WAIT`]).
    pub wait: Duration,
}

impl StartRequest {
    /// A request for the daemon at `daemon_exe` with the files of `paths` and the plan's wait.
    pub fn new(daemon_exe: PathBuf, paths: &DaemonPaths) -> StartRequest {
        StartRequest {
            daemon_exe,
            lock_file: paths.lock_file.clone(),
            socket: paths.socket.clone(),
            log_file: paths.log_file.clone(),
            env: BTreeMap::new(),
            wait: LOCK_WAIT,
        }
    }
}

/// Whether a daemon answers on `socket`: a connection that is accepted. The daemon takes a connection that sends
/// nothing and closes as a probe.
fn answers(socket: &Path) -> bool {
    UnixStream::connect(socket).is_ok()
}

/// Takes the lock file's exclusive `flock`, polling for up to `wait`. The lock lasts as long as the returned file is
/// open, so it is dropped when the start is done, and a start that dies releases it.
fn lock(lock_file: &Path, wait: Duration) -> Result<fs::File, StartError> {
    let io_error = |context: String| move |source| StartError::Io { context, source };
    if let Some(dir) = lock_file.parent() {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)
            .map_err(io_error(format!("could not make {}", dir.display())))?;
    }
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .mode(0o600)
        .open(lock_file)
        .map_err(io_error(format!(
            "could not open the lock file {}",
            lock_file.display()
        )))?;
    let until = Instant::now() + wait;
    loop {
        // SAFETY: `flock` takes a descriptor this function holds open and an operation; it touches no memory.
        let taken = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if taken == 0 {
            return Ok(file);
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::WouldBlock {
            return Err(StartError::Io {
                context: format!("could not lock {}", lock_file.display()),
                source: error,
            });
        }
        if Instant::now() >= until {
            return Err(StartError::LockTimeout {
                lock_file: lock_file.to_path_buf(),
            });
        }
        std::thread::sleep(POLL);
    }
}

/// Makes sure a daemon answers on the socket: takes the lock, probes the socket, starts the daemon when it does not
/// answer ([`bootstrap_launchd_job`] on macOS, then [`start_with_setsid`] if that fails), and waits for the answer.
///
/// Today the start is `setsid` only, because the launchd job is `ts-0058-ad`'s: that item puts its attempt before the
/// `setsid` one here, and the result of a `setsid` start is [`StartKind::SetsidFallback`] with its warning.
pub fn ensure_daemon(request: &StartRequest) -> Result<StartKind, StartError> {
    let lock_file = lock(&request.lock_file, request.wait)?;
    // The lock file holds the socket path, for whoever looks at what a start is holding.
    let written = lock_file
        .set_len(0)
        .and_then(|()| (&lock_file).write_all(request.socket.as_os_str().as_encoded_bytes()));
    if let Err(source) = written {
        return Err(StartError::Io {
            context: format!("could not write {}", request.lock_file.display()),
            source,
        });
    }
    if answers(&request.socket) {
        return Ok(StartKind::AlreadyRunning);
    }
    let pid = start_with_setsid(request).map_err(|source| StartError::Io {
        context: format!("could not start {}", request.daemon_exe.display()),
        source,
    })?;
    let until = Instant::now() + request.wait;
    loop {
        if answers(&request.socket) {
            return Ok(StartKind::SetsidFallback {
                warning: SETSID_WARNING.to_string(),
            });
        }
        if let Some(status) = reap_if_ended(pid) {
            return Err(StartError::Exited {
                status,
                log_file: request.log_file.clone(),
                log_tail: log_tail(&request.log_file),
            });
        }
        if Instant::now() >= until {
            return Err(StartError::NoAnswer {
                socket: request.socket.clone(),
            });
        }
        std::thread::sleep(POLL);
    }
}

/// Whether the child `pid` has ended, and if it has, how, reaping it. It is this process's own child, started by
/// [`start_with_setsid`], so the wait is on a pid this process made.
fn reap_if_ended(pid: u32) -> Option<String> {
    let mut status: libc::c_int = 0;
    // SAFETY: `waitpid` writes one `c_int` through the pointer, which is to a value that lives for the call.
    let ended = unsafe { libc::waitpid(pid as libc::pid_t, &mut status, libc::WNOHANG) };
    if ended != pid as libc::pid_t {
        return None;
    }
    Some(if libc::WIFEXITED(status) {
        format!("exit status {}", libc::WEXITSTATUS(status))
    } else if libc::WIFSIGNALED(status) {
        format!("signal {}", libc::WTERMSIG(status))
    } else {
        "ended".to_string()
    })
}

/// The last two non-empty lines of a log, joined, for an error message. A log that cannot be read says so.
fn log_tail(log_file: &Path) -> String {
    match fs::read_to_string(log_file) {
        Ok(text) => {
            let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
            let from = lines.len().saturating_sub(2);
            if lines.is_empty() {
                "(empty)".to_string()
            } else {
                lines[from..].join(" | ")
            }
        }
        Err(e) => format!("(unreadable: {e})"),
    }
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
/// of its own. Its input is `/dev/null` and its output is appended to [`StartRequest::log_file`]. Returns the pid.
///
/// The daemon is this process's child until this process ends: a caller that lives on reaps it with `waitpid`
/// ([`ensure_daemon`] does when the daemon ends during the start).
pub fn start_with_setsid(request: &StartRequest) -> io::Result<u32> {
    if let Some(dir) = request.log_file.parent() {
        fs::create_dir_all(dir)?;
    }
    let log = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(&request.log_file)?;
    let mut command = Command::new(&request.daemon_exe);
    command
        .current_dir("/")
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        .envs(&request.env);
    // SAFETY: `setsid` takes no arguments and is async-signal-safe, which `pre_exec` requires. The daemon becomes the
    // leader of a session with no controlling terminal, so closing the starter's terminal does not hang it up.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    Ok(command.spawn()?.id())
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
    fn a_request_takes_its_files_from_the_daemon_paths_and_waits_fifteen_seconds() {
        let paths = DaemonPaths::under(Path::new("/tmp/x"));
        let request = StartRequest::new(PathBuf::from("/x/collide-daemon"), &paths);
        assert_eq!(request.socket, paths.socket);
        assert_eq!(request.lock_file, paths.lock_file);
        assert_eq!(request.log_file, paths.log_file);
        assert_eq!(request.wait, LOCK_WAIT);
    }

    #[test]
    fn the_setsid_warning_says_the_daemon_may_end_with_its_launcher() {
        assert!(
            SETSID_WARNING.contains("may still end with"),
            "{SETSID_WARNING}"
        );
    }
}
