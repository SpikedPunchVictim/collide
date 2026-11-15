//! Starting the daemon (0.2, Starting the daemon; Stage 3a). In v0 the app and the CLI start it on demand when the
//! socket does not answer. This module is that start, in the steps the plan gives:
//!
//! 1. **An `flock` on the lock file serializes starts.** The lock file holds the socket path. A start waits up to
//!    [`LOCK_WAIT`], and the daemon adopts existing shims before it binds the socket ([`crate::daemon`]).
//! 2. **On macOS the daemon leaves its launcher's coalition**, because a `setsid` child stays in the coalition of the
//!    app or Terminal that started it (measured S2). The start is a one-shot launchd job: `launchctl bootstrap
//!    gui/<uid>` of a temporary plist, removed once the daemon runs ([`LaunchdJob`]). The plist's paths are absolute
//!    and canonical, and it sets `USER`, `HOME` and `PATH`, because `claude` fails with `api_error` without `USER`
//!    (measured S2). launchd starts the job in `/` with a bare environment. The plist lives in the daemon's run
//!    directory, never in `~/Library/LaunchAgents`. Every call on launchd goes through [`Launchctl`], so a test
//!    can refuse a bootstrap, and a start loads a job only when its caller set [`StartRequest::launchctl`]
//!    ([`launchctl_from_env`] is the rule the binary and the shim use).
//! 3. **Over SSH, or with no GUI session, `launchctl bootstrap` can fail** (inferred). The daemon then starts with
//!    `setsid`, outside the coalition, and the start returns a warning that it may end with its launcher
//!    ([`StartKind::SetsidFallback`]). The run is not refused.
//! 4. **The resident app restarts the daemon** when the socket stops answering, the same way, and after
//!    [`MAX_FAILED_STARTS`] failed starts in [`FAILED_START_WINDOW`] it stops trying ([`RestartPolicy`]).
//!
//! A shim whose child exits while the daemon is down starts the daemon through the same entry point, so a phase
//! boundary is not missed while the app is closed.
//!
//! **What the daemon's environment carries.** Both starts strip `COLLIDE_RUN`, `COLLIDE_RUN_START` and
//! `COLLIDE_SPAN` ([`STRIPPED_ENV`]) and set `COLLIDE_RUNTIME=1`: a daemon started by the CLI from inside a run must
//! not carry the run's id, or a stop of that run would find it. **When the daemon stops** it boots out the job its
//! start recorded ([`boot_out_recorded_job`]).
//!
//! | tag | function | what it does |
//! |---|---|---|
//! | `daemon: lock file start` | [`ensure_daemon`] | the lock, the wait, the socket probe and the choice between 2 and 3 |
//! | `daemon: launchd job` | [`launchd_plist`], [`launchd_job`], [`Launchctl`] | the plist, its inputs made canonical, and the bootstrap, removed once the daemon runs |
//! | `daemon: setsid start` | [`start_with_setsid`] | the fallback start, with its warning |

use crate::daemon::{DaemonPaths, ROOT_ENV};
use crate::stop::RUNTIME_MARKER;
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
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
#[derive(Debug, Clone)]
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
    /// The `launchctl` the start tries before `setsid`. `None` is a start with `setsid` only, which is what
    /// [`StartRequest::new`] gives, so no start touches launchd unless its caller asked ([`launchctl_from_env`]).
    pub launchctl: Option<Arc<dyn Launchctl>>,
    /// The start of the job's label ([`DEFAULT_LABEL_PREFIX`]); the rest is made per start.
    pub label_prefix: String,
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
            launchctl: None,
            label_prefix: DEFAULT_LABEL_PREFIX.to_string(),
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
/// answer, and waits for the answer. The start is a one-shot launchd job when [`StartRequest::launchctl`] is set
/// ([`start_launchd_job`]), and `setsid` ([`start_with_setsid`]) when it is not or when launchd refuses: that start is
/// [`StartKind::SetsidFallback`], with the warning and, when launchd refused, what it said. The temporary plist is
/// gone when this returns, whatever the outcome.
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
    let mut warning = SETSID_WARNING.to_string();
    if let Some(launchctl) = &request.launchctl {
        match start_launchd_job(request, launchctl.as_ref(), &|name| {
            std::env::var(name).ok()
        }) {
            Ok(job) => {
                let mut next_look = Instant::now() + JOB_SETTLE;
                wait_for_answer(request, || {
                    if Instant::now() < next_look {
                        return None;
                    }
                    next_look = Instant::now() + JOB_LOOK;
                    // An error is "cannot tell", which is not an end.
                    match launchctl.process_of(&job.target) {
                        Ok(None) => Some(format!("the launchd job {} has no process", job.label)),
                        _ => None,
                    }
                })?;
                return Ok(StartKind::Launchd);
            }
            Err(error) => warning = format!("{SETSID_WARNING} (launchd said: {error})"),
        }
    }
    let pid = start_with_setsid(request).map_err(|source| StartError::Io {
        context: format!("could not start {}", request.daemon_exe.display()),
        source,
    })?;
    wait_for_answer(request, || reap_if_ended(pid))?;
    Ok(StartKind::SetsidFallback { warning })
}

/// Waits up to [`StartRequest::wait`] for the socket to answer. `ended` says, when the daemon is gone, how it ended.
fn wait_for_answer(
    request: &StartRequest,
    mut ended: impl FnMut() -> Option<String>,
) -> Result<(), StartError> {
    let until = Instant::now() + request.wait;
    loop {
        if answers(&request.socket) {
            return Ok(());
        }
        if let Some(status) = ended() {
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

/// The label's start when [`StartRequest::label_prefix`] is not set: the product's reverse-DNS name, so a job of ours
/// is told from anyone else's in `launchctl list`.
pub const DEFAULT_LABEL_PREFIX: &str = "dev.collide.daemon";

/// The variable that names the `launchctl` program to use, and that asks for launchd even under an overridden root.
pub const LAUNCHCTL_ENV: &str = "COLLIDE_LAUNCHCTL";

/// The `launchctl` a real start uses.
pub const SYSTEM_LAUNCHCTL: &str = "/bin/launchctl";

/// The file in the run directory that holds the label of the job the last start made, so the next start can end it.
pub const LABEL_FILE: &str = "launchd.label";

/// How long a launchd job may have no process, from the bootstrap, before the start calls it ended. launchd answered
/// about 70 ms after a start in S2, and 500 ms the first time.
const JOB_SETTLE: Duration = Duration::from_secs(3);

/// How often the start asks launchd whether the job has a process, once [`JOB_SETTLE`] is over.
const JOB_LOOK: Duration = Duration::from_millis(250);

/// What a run's process carries that a daemon must not inherit (0.2): started by the CLI from inside a run, a daemon
/// would otherwise carry the run's id and be found, and stopped, with the run. Both starts strip them.
pub const STRIPPED_ENV: [&str; 3] = ["COLLIDE_RUN", "COLLIDE_RUN_START", "COLLIDE_SPAN"];

/// Makes sure a daemon answers, for a CLI command (`collide stop`, `collide run`): [`ensure_daemon`] with the
/// `collide-daemon` that sits beside the running `collide`. What the start forwards ([`FORWARDED_ENV`],
/// [`LAUNCHCTL_ENV`]) is read from this process's environment, and under `COLLIDE_DAEMON_ROOT` no launchd job is loaded
/// ([`launchctl_from_env`]).
pub fn ensure_running_from_cli(paths: &DaemonPaths) -> Result<(), String> {
    let exe = std::fs::canonicalize(std::env::current_exe().map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
        .and_then(|exe| {
            exe.parent()
                .map(|dir| dir.join("collide-daemon"))
                .ok_or_else(|| "the collide binary has no folder".to_string())
        })
        .map_err(|e| format!("cannot find collide-daemon: {e}"))?;
    let mut request = StartRequest::new(exe, paths);
    request.env = FORWARDED_ENV
        .into_iter()
        .chain([LAUNCHCTL_ENV])
        .filter_map(|name| Some((name.to_string(), std::env::var(name).ok()?)))
        .collect();
    request.launchctl = launchctl_from_env(
        std::env::var(ROOT_ENV).ok().as_deref(),
        std::env::var(LAUNCHCTL_ENV).ok().as_deref(),
    );
    ensure_daemon(&request)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// The path to use when the starter's own `PATH` is not set: the system's.
const FALLBACK_PATH: &str = "/usr/bin:/bin:/usr/sbin:/sbin";

/// Variables the start passes on to the job when the starter has them, because launchd's environment is bare.
pub const FORWARDED_ENV: [&str; 3] = ["COLLIDE_DAEMON_ROOT", "HOME", "SHELL"];

/// The variables of the job whose value is a path. Each is made canonical when it exists (`/tmp` and `/var` are
/// symlinks on macOS).
const PATH_VARIABLES: [&str; 4] = ["HOME", "COLLIDE_DAEMON_ROOT", "SHELL", LAUNCHCTL_ENV];

/// The calls a start makes on launchd. This is the seam: a start under test gets a double, so no test calls the real
/// `launchctl` unless it is the opt-in one.
pub trait Launchctl: std::fmt::Debug + Send + Sync {
    /// `launchctl bootstrap <domain> <plist>`: loads the job, which then runs once ([`launchd_plist`]).
    fn bootstrap(&self, domain: &str, plist: &Path) -> io::Result<()>;
    /// `launchctl bootout <target>`: unloads a job and ends its process.
    fn bootout(&self, target: &str) -> io::Result<()>;
    /// The pid of the job's process: `Ok(None)` when the job is loaded and has none, and an error when launchd cannot
    /// say (including a job that is not loaded).
    fn process_of(&self, target: &str) -> io::Result<Option<u32>>;
}

/// The real `launchctl`, or a program that stands in for it (a script that records its arguments).
#[derive(Debug, Clone)]
pub struct SystemLaunchctl {
    program: PathBuf,
}

impl SystemLaunchctl {
    pub fn new(program: impl Into<PathBuf>) -> SystemLaunchctl {
        SystemLaunchctl {
            program: program.into(),
        }
    }

    /// Runs the program with `args`. A failure names the command and carries what it printed on stderr.
    fn run(&self, args: &[&std::ffi::OsStr]) -> io::Result<String> {
        let output = Command::new(&self.program)
            .args(args)
            .stdin(Stdio::null())
            .output()?;
        if !output.status.success() {
            let shown: Vec<String> = args
                .iter()
                .map(|a| a.to_string_lossy().into_owned())
                .collect();
            return Err(io::Error::other(format!(
                "`{} {}` ended with {}: {}",
                self.program.display(),
                shown.join(" "),
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

impl Launchctl for SystemLaunchctl {
    fn bootstrap(&self, domain: &str, plist: &Path) -> io::Result<()> {
        self.run(&["bootstrap".as_ref(), domain.as_ref(), plist.as_os_str()])
            .map(|_| ())
    }

    fn bootout(&self, target: &str) -> io::Result<()> {
        self.run(&["bootout".as_ref(), target.as_ref()]).map(|_| ())
    }

    fn process_of(&self, target: &str) -> io::Result<Option<u32>> {
        let text = self.run(&["print".as_ref(), target.as_ref()])?;
        Ok(pid_in_print(&text))
    }
}

/// The `pid = N` line of `launchctl print <target>`, which a job with no process does not have.
fn pid_in_print(text: &str) -> Option<u32> {
    text.lines()
        .find_map(|line| line.trim().strip_prefix("pid = "))
        .and_then(|pid| pid.trim().parse().ok())
}

/// The `launchctl` a start in this environment uses, if any: none off macOS. `root` is `COLLIDE_DAEMON_ROOT` and
/// `launchctl` is [`LAUNCHCTL_ENV`], both as the starter has them.
///
/// - `launchctl` set: that program. This is how a test names a double, and how the opt-in test names the real one.
/// - Else `root` set: none. A start under an overridden root is a test's or a developer's own, and it never loads a
///   job into the user's launchd unless it says so.
/// - Else [`SYSTEM_LAUNCHCTL`].
pub fn launchctl_from_env(
    root: Option<&str>,
    launchctl: Option<&str>,
) -> Option<Arc<dyn Launchctl>> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    match (launchctl.filter(|p| !p.is_empty()), root) {
        (Some(program), _) => Some(Arc::new(SystemLaunchctl::new(program))),
        (None, Some(_)) => None,
        (None, None) => Some(Arc::new(SystemLaunchctl::new(SYSTEM_LAUNCHCTL))),
    }
}

/// The one-shot launchd job: a label, the daemon, where its output goes and its whole environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchdJob {
    /// The job's label, unique per start so that two starts never name one job.
    pub label: String,
    /// The `collide-daemon` binary, absolute and canonical.
    pub daemon_exe: PathBuf,
    /// The daemon's log, absolute and canonical: launchd opens it for the job's output and error.
    pub log_file: PathBuf,
    /// Everything the daemon starts with. launchd gives a job nothing: this holds `USER`, `HOME` and `PATH`, and what
    /// the start forwards ([`FORWARDED_ENV`]).
    pub env: BTreeMap<String, String>,
}

/// The job's plist, as text. The daemon runs in `/` and its paths are all absolute (the job's inputs are made
/// canonical by [`launchd_job`]).
///
/// The keys, and why:
/// - `RunAtLoad` true and no `KeepAlive`: `bootstrap` runs the daemon once, and launchd never starts it again. The
///   job is not in a `LaunchAgents` folder, so it is not loaded at the next login either. Keeping the daemon alive is
///   deferred (0.2).
/// - `AbandonProcessGroup` true: the shims and the children must outlive the daemon, and launchd would otherwise
///   end what is left in the job's process group when the daemon ends.
/// - `ProcessType` `Interactive`: the daemon serves a person's session, so launchd does not run it as a background job.
/// - `WorkingDirectory` `/`, the same as the `setsid` start.
pub fn launchd_plist(job: &LaunchdJob) -> String {
    let string = |value: &str| format!("<string>{}</string>", xml_escape(value));
    let log = string(&job.log_file.to_string_lossy());
    let mut env = String::new();
    for (name, value) in &job.env {
        env.push_str(&format!(
            "    <key>{}</key>\n    {}\n",
            xml_escape(name),
            string(value)
        ));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  {label}
  <key>ProgramArguments</key>
  <array>
    {exe}
  </array>
  <key>WorkingDirectory</key>
  <string>/</string>
  <key>EnvironmentVariables</key>
  <dict>
{env}  </dict>
  <key>StandardOutPath</key>
  {log}
  <key>StandardErrorPath</key>
  {log}
  <key>RunAtLoad</key>
  <true/>
  <key>AbandonProcessGroup</key>
  <true/>
  <key>ProcessType</key>
  <string>Interactive</string>
</dict>
</plist>
"#,
        label = string(&job.label),
        exe = string(&job.daemon_exe.to_string_lossy()),
    )
}

/// `text` with the five characters XML reads as markup replaced.
fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// A job's plist on disk. The file is removed when this is dropped, so it is gone whether the start worked or not.
#[derive(Debug)]
struct PlistFile(PathBuf);

impl Drop for PlistFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// A job that `launchctl bootstrap` loaded.
#[derive(Debug)]
struct LaunchedJob {
    label: String,
    /// `gui/<uid>/<label>`, which `bootout` and `print` take.
    target: String,
    _plist: PlistFile,
}

fn current_uid() -> u32 {
    // SAFETY: `getuid` takes no arguments, cannot fail and touches no memory.
    unsafe { libc::getuid() }
}

/// A label for one start: the prefix, this process's pid and the time in nanoseconds.
fn new_label(prefix: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    format!("{prefix}.{}-{nanos}", std::process::id())
}

fn invalid(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

/// `path` made canonical when it can be: an absolute path that exists. Anything else is returned as it is.
fn canonical_if_exists(path: &str) -> String {
    if !Path::new(path).is_absolute() {
        return path.to_string();
    }
    fs::canonicalize(path).map_or_else(|_| path.to_string(), |p| p.display().to_string())
}

/// The job for a start: the daemon and the log made canonical, and the environment from the request, then the
/// starter's own (`lookup`). `USER` is required, because `claude` fails with `api_error` without it (measured S2); an
/// unset `HOME` is an error too, and an unset `PATH` is the system's. A start that cannot say these falls back to
/// `setsid`, which passes the starter's environment on whole.
pub fn launchd_job(
    request: &StartRequest,
    label: String,
    lookup: &dyn Fn(&str) -> Option<String>,
) -> io::Result<LaunchdJob> {
    let daemon_exe = fs::canonicalize(&request.daemon_exe).map_err(|e| {
        io::Error::new(
            e.kind(),
            format!("cannot find {}: {e}", request.daemon_exe.display()),
        )
    })?;
    let log_file = canonical_log_file(&request.log_file)?;
    let value = |name: &str| request.env.get(name).cloned().or_else(|| lookup(name));
    let user = value("USER")
        .or_else(|| lookup("LOGNAME"))
        .filter(|u| !u.is_empty())
        .ok_or_else(|| invalid("neither USER nor LOGNAME is set".to_string()))?;
    let home = value("HOME")
        .filter(|h| Path::new(h).is_absolute())
        .ok_or_else(|| invalid("HOME is not set to an absolute path".to_string()))?;
    let path = value("PATH")
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| FALLBACK_PATH.to_string());
    let mut env: BTreeMap<String, String> = request.env.clone();
    for name in STRIPPED_ENV {
        env.remove(name);
    }
    env.insert(RUNTIME_MARKER.to_string(), "1".to_string());
    env.insert("USER".to_string(), user);
    env.insert("HOME".to_string(), home);
    env.insert("PATH".to_string(), path);
    for name in PATH_VARIABLES {
        if let Some(value) = env.get_mut(name) {
            *value = canonical_if_exists(value);
        }
    }
    Ok(LaunchdJob {
        label,
        daemon_exe,
        log_file,
        env,
    })
}

/// The log file's canonical path. The file is made first (mode `0600`, like the `setsid` start's), because launchd
/// opens it for the job and cannot make its folder.
fn canonical_log_file(log_file: &Path) -> io::Result<PathBuf> {
    let (Some(dir), Some(name)) = (log_file.parent(), log_file.file_name()) else {
        return Err(invalid(format!("{} has no folder", log_file.display())));
    };
    fs::create_dir_all(dir)?;
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(log_file)?;
    Ok(fs::canonicalize(dir)?.join(name))
}

/// Starts the daemon as a one-shot launchd job and returns the loaded job; the caller waits for the daemon's answer.
///
/// 1. The job's inputs are made canonical ([`launchd_job`]).
/// 2. A job that an earlier start of ours recorded in [`LABEL_FILE`] is booted out first. The lock is held and the
///    socket does not answer, so that daemon is gone or stuck, and a loaded job of a gone daemon is only litter.
/// 3. The plist is written next to the lock file, in the daemon's run directory and never in `~/Library/LaunchAgents`,
///    and its label is recorded.
/// 4. `launchctl bootstrap gui/<uid> <plist>`. When it fails the plist and the record are removed, and the error says
///    what `launchctl` printed.
fn start_launchd_job(
    request: &StartRequest,
    launchctl: &dyn Launchctl,
    lookup: &dyn Fn(&str) -> Option<String>,
) -> io::Result<LaunchedJob> {
    let job = launchd_job(request, new_label(&request.label_prefix), lookup)?;
    let uid = current_uid();
    let dir = request
        .lock_file
        .parent()
        .ok_or_else(|| invalid(format!("{} has no folder", request.lock_file.display())))
        .and_then(fs::canonicalize)?;
    let label_file = dir.join(LABEL_FILE);
    if let Ok(old) = fs::read_to_string(&label_file) {
        let old = old.trim();
        if !old.is_empty() {
            // Not loaded any more is the usual answer, and not a failure.
            let _ = launchctl.bootout(&format!("gui/{uid}/{old}"));
        }
        let _ = fs::remove_file(&label_file);
    }
    let plist_path = dir.join(format!("{}.plist", job.label));
    fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&plist_path)?
        .write_all(launchd_plist(&job).as_bytes())?;
    let plist = PlistFile(plist_path);
    fs::write(&label_file, &job.label)?;
    if let Err(error) = launchctl.bootstrap(&format!("gui/{uid}"), &plist.0) {
        let _ = fs::remove_file(&label_file);
        return Err(error);
    }
    Ok(LaunchedJob {
        target: format!("gui/{uid}/{}", job.label),
        label: job.label,
        _plist: plist,
    })
}

/// Boots out the launchd job the last start recorded in [`LABEL_FILE`] (the file next to `lock_file`), and removes the
/// record. Returns the label it booted out, or `None` when no job is recorded. A job that is not loaded any more is
/// the usual answer for a daemon that was not started by launchd, and is not an error; any other failure of
/// `launchctl` is returned, with the record kept so a later stop can try again.
///
/// The daemon calls this as its last act when it stops (`launchctl bootout` ends the job's process, which is the
/// daemon), so a stopped daemon leaves no loaded job behind.
pub fn boot_out_recorded_job(
    lock_file: &Path,
    launchctl: &dyn Launchctl,
) -> io::Result<Option<String>> {
    let Some(dir) = lock_file.parent() else {
        return Ok(None);
    };
    let label_file = dir.join(LABEL_FILE);
    let label = match fs::read_to_string(&label_file) {
        Ok(text) => text.trim().to_string(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if label.is_empty() {
        let _ = fs::remove_file(&label_file);
        return Ok(None);
    }
    let target = format!("gui/{}/{label}", current_uid());
    match launchctl.bootout(&target) {
        Ok(()) => {}
        // `launchctl` answers a job it does not have with "No such process" (3) or an input/output error (5); the
        // record is then litter and goes.
        Err(_) if !is_loaded(launchctl, &target) => {}
        Err(error) => return Err(error),
    }
    let _ = fs::remove_file(&label_file);
    Ok(Some(label))
}

/// Whether launchd still has the job: `print` of a job that is not loaded fails.
fn is_loaded(launchctl: &dyn Launchctl, target: &str) -> bool {
    launchctl.process_of(target).is_ok()
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
    // The daemon is runtime, never part of a run, whoever started it.
    for name in STRIPPED_ENV {
        command.env_remove(name);
    }
    command.env(RUNTIME_MARKER, "1");
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
    use std::os::unix::fs::PermissionsExt;

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

    // ---- the launchd job ----------------------------------------------------------------------------------------

    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU32, Ordering};

    static DIRS: AtomicU32 = AtomicU32::new(0);

    /// A folder of the test's own under the system's temporary folder (a symlinked path on macOS), removed by its exact
    /// path when the test ends.
    struct Dir(PathBuf);

    impl Dir {
        fn new() -> Dir {
            let n = DIRS.fetch_add(1, Ordering::SeqCst);
            let dir =
                std::env::temp_dir().join(format!("collide-launch-{}-{n}", std::process::id()));
            fs::create_dir_all(&dir).unwrap();
            Dir(dir)
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// A `launchctl` double: it records each call, and fails the ones it is told to.
    #[derive(Debug, Default)]
    struct Fake {
        calls: Mutex<Vec<String>>,
        /// The plist's text at the moment of each bootstrap.
        plists: Mutex<Vec<String>>,
        fail_bootstrap: bool,
    }

    impl Launchctl for Fake {
        fn bootstrap(&self, domain: &str, plist: &Path) -> io::Result<()> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("bootstrap {domain} {}", plist.display()));
            self.plists.lock().unwrap().push(fs::read_to_string(plist)?);
            if self.fail_bootstrap {
                return Err(io::Error::other("Bootstrap failed: 5: Input/output error"));
            }
            Ok(())
        }

        fn bootout(&self, target: &str) -> io::Result<()> {
            self.calls.lock().unwrap().push(format!("bootout {target}"));
            Ok(())
        }

        fn process_of(&self, _target: &str) -> io::Result<Option<u32>> {
            Ok(None)
        }
    }

    fn env_of(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + use<> {
        let map: BTreeMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |name| map.get(name).cloned()
    }

    fn pinned_job() -> LaunchdJob {
        LaunchdJob {
            label: "dev.collide.daemon.7-9".to_string(),
            daemon_exe: PathBuf::from("/opt/collide/bin/collide-daemon"),
            log_file: PathBuf::from("/Users/x/Library/Application Support/Collide/daemon.log"),
            env: [
                ("HOME", "/Users/x"),
                ("PATH", "/usr/bin:/bin"),
                ("USER", "x"),
            ]
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        }
    }

    const PINNED_PLIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>dev.collide.daemon.7-9</string>
  <key>ProgramArguments</key>
  <array>
    <string>/opt/collide/bin/collide-daemon</string>
  </array>
  <key>WorkingDirectory</key>
  <string>/</string>
  <key>EnvironmentVariables</key>
  <dict>
    <key>HOME</key>
    <string>/Users/x</string>
    <key>PATH</key>
    <string>/usr/bin:/bin</string>
    <key>USER</key>
    <string>x</string>
  </dict>
  <key>StandardOutPath</key>
  <string>/Users/x/Library/Application Support/Collide/daemon.log</string>
  <key>StandardErrorPath</key>
  <string>/Users/x/Library/Application Support/Collide/daemon.log</string>
  <key>RunAtLoad</key>
  <true/>
  <key>AbandonProcessGroup</key>
  <true/>
  <key>ProcessType</key>
  <string>Interactive</string>
</dict>
</plist>
"#;

    #[test]
    fn the_plist_text_is_pinned() {
        assert_eq!(launchd_plist(&pinned_job()), PINNED_PLIST);
    }

    #[test]
    fn the_plist_has_the_three_variables_and_only_absolute_paths() {
        let plist = launchd_plist(&pinned_job());
        for key in ["USER", "HOME", "PATH"] {
            assert!(plist.contains(&format!("<key>{key}</key>")), "{key}");
        }
        let paths = [
            "/opt/collide/bin/collide-daemon",
            "/Users/x/Library/Application Support/Collide/daemon.log",
            "<string>/</string>",
        ];
        for path in paths {
            assert!(plist.contains(path), "{path}");
        }
    }

    #[test]
    fn the_plist_is_a_one_shot_job() {
        let plist = launchd_plist(&pinned_job());
        assert!(!plist.contains("KeepAlive"));
        assert!(!plist.contains("LaunchAgents"));
        assert!(plist.contains("<key>RunAtLoad</key>\n  <true/>"));
        assert!(plist.contains("<key>AbandonProcessGroup</key>\n  <true/>"));
    }

    #[test]
    fn the_plist_escapes_what_xml_reads_as_markup() {
        let mut job = pinned_job();
        job.env
            .insert("NOTE".to_string(), "a<b & \"c\"".to_string());
        assert!(launchd_plist(&job).contains("<string>a&lt;b &amp; &quot;c&quot;</string>"));
    }

    #[test]
    fn a_job_makes_its_paths_canonical_and_carries_the_three_variables() {
        let dir = Dir::new();
        let canonical = fs::canonicalize(&dir.0).unwrap();
        // A symlink to the folder: the job names the folder, not the link.
        let link = dir.0.join("link");
        std::os::unix::fs::symlink(&canonical, &link).unwrap();
        let exe = canonical.join("collide-daemon");
        fs::write(&exe, "").unwrap();
        let mut request =
            StartRequest::new(link.join("collide-daemon"), &DaemonPaths::under(&link));
        request.env.insert(
            "COLLIDE_DAEMON_ROOT".to_string(),
            link.display().to_string(),
        );
        let lookup = env_of(&[
            ("USER", "x"),
            ("HOME", &canonical.display().to_string()),
            ("PATH", "/usr/bin:/bin"),
        ]);
        let job = launchd_job(&request, "l".to_string(), &lookup).unwrap();
        assert_eq!(job.daemon_exe, exe);
        assert_eq!(job.log_file, canonical.join("data").join("daemon.log"));
        assert_eq!(
            job.env["COLLIDE_DAEMON_ROOT"],
            canonical.display().to_string()
        );
        assert_eq!(job.env["HOME"], canonical.display().to_string());
        assert_eq!(job.env["USER"], "x");
        assert_eq!(job.env["PATH"], "/usr/bin:/bin");
        // The log exists, so launchd can open it.
        assert!(job.log_file.is_file());
    }

    #[test]
    fn a_job_takes_the_request_s_variables_before_the_starter_s_own() {
        let dir = Dir::new();
        let exe = dir.0.join("collide-daemon");
        fs::write(&exe, "").unwrap();
        let mut request = StartRequest::new(exe, &DaemonPaths::under(&dir.0));
        request.env.insert("HOME".to_string(), "/tmp".to_string());
        let lookup = env_of(&[("USER", "x"), ("HOME", "/Users/real")]);
        let job = launchd_job(&request, "l".to_string(), &lookup).unwrap();
        assert_eq!(job.env["HOME"], canonical_if_exists("/tmp"));
        assert_eq!(job.env["PATH"], FALLBACK_PATH);
    }

    #[test]
    fn a_job_without_a_user_or_a_home_is_refused() {
        let dir = Dir::new();
        let exe = dir.0.join("collide-daemon");
        fs::write(&exe, "").unwrap();
        let request = StartRequest::new(exe, &DaemonPaths::under(&dir.0));
        let no_user = launchd_job(&request, "l".into(), &env_of(&[("HOME", "/h")])).unwrap_err();
        assert!(no_user.to_string().contains("USER"), "{no_user}");
        let no_home = launchd_job(&request, "l".into(), &env_of(&[("USER", "x")])).unwrap_err();
        assert!(no_home.to_string().contains("HOME"), "{no_home}");
        let logname = launchd_job(
            &request,
            "l".into(),
            &env_of(&[("LOGNAME", "y"), ("HOME", "/h")]),
        )
        .unwrap();
        assert_eq!(logname.env["USER"], "y");
    }

    #[test]
    fn a_job_for_a_daemon_that_is_not_there_says_which() {
        let dir = Dir::new();
        let request = StartRequest::new(dir.0.join("missing"), &DaemonPaths::under(&dir.0));
        let error = launchd_job(
            &request,
            "l".into(),
            &env_of(&[("USER", "x"), ("HOME", "/h")]),
        )
        .unwrap_err();
        assert!(error.to_string().contains("missing"), "{error}");
    }

    fn request_in(dir: &Dir) -> StartRequest {
        let exe = dir.0.join("collide-daemon");
        fs::write(&exe, "").unwrap();
        let mut request = StartRequest::new(exe, &DaemonPaths::under(&dir.0));
        request.label_prefix = "dev.collide.test.unit".to_string();
        fs::create_dir_all(request.lock_file.parent().unwrap()).unwrap();
        request
    }

    #[test]
    fn a_bootstrap_names_the_gui_domain_and_a_plist_in_the_run_directory() {
        let dir = Dir::new();
        let request = request_in(&dir);
        let fake = Fake::default();
        let lookup = env_of(&[("USER", "x"), ("HOME", "/h")]);
        let job = start_launchd_job(&request, &fake, &lookup).unwrap();

        let run = fs::canonicalize(request.lock_file.parent().unwrap()).unwrap();
        let calls = fake.calls.lock().unwrap().clone();
        assert_eq!(calls.len(), 1, "{calls:?}");
        let uid = current_uid();
        let expected_start = format!(
            "bootstrap gui/{uid} {}/dev.collide.test.unit.",
            run.display()
        );
        assert!(calls[0].starts_with(&expected_start), "{calls:?}");
        assert!(calls[0].ends_with(".plist"), "{calls:?}");
        assert!(!calls[0].contains("LaunchAgents"));
        // The plist was on disk when launchctl was called, and it is the job's.
        let plists = fake.plists.lock().unwrap();
        assert!(plists[0].contains(&job.label));
        // The label is recorded for the next start, and the job's target is the one `bootout` takes.
        assert_eq!(fs::read_to_string(run.join(LABEL_FILE)).unwrap(), job.label);
        assert_eq!(job.target, format!("gui/{uid}/{}", job.label));
        assert!(job.label.starts_with("dev.collide.test.unit."));
    }

    #[test]
    fn the_plist_is_removed_when_the_job_is_dropped() {
        let dir = Dir::new();
        let request = request_in(&dir);
        let lookup = env_of(&[("USER", "x"), ("HOME", "/h")]);
        let job = start_launchd_job(&request, &Fake::default(), &lookup).unwrap();
        let plist = job._plist.0.clone();
        assert!(plist.is_file());
        drop(job);
        assert!(!plist.exists());
    }

    #[test]
    fn a_failed_bootstrap_leaves_no_plist_and_no_label_and_returns_what_launchctl_said() {
        let dir = Dir::new();
        let request = request_in(&dir);
        let fake = Fake {
            fail_bootstrap: true,
            ..Fake::default()
        };
        let lookup = env_of(&[("USER", "x"), ("HOME", "/h")]);
        let error = start_launchd_job(&request, &fake, &lookup).unwrap_err();
        assert!(error.to_string().contains("Input/output error"), "{error}");
        let run = request.lock_file.parent().unwrap();
        let left: Vec<_> = fs::read_dir(run)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert!(
            left.iter()
                .all(|n| !n.to_string_lossy().ends_with(".plist")
                    && n.to_string_lossy() != LABEL_FILE),
            "{left:?}"
        );
    }

    #[test]
    fn a_job_an_earlier_start_left_loaded_is_booted_out_before_the_new_one_is_loaded() {
        let dir = Dir::new();
        let request = request_in(&dir);
        let run = request.lock_file.parent().unwrap();
        fs::write(run.join(LABEL_FILE), "dev.collide.test.unit.old-1\n").unwrap();
        let fake = Fake::default();
        let lookup = env_of(&[("USER", "x"), ("HOME", "/h")]);
        let job = start_launchd_job(&request, &fake, &lookup).unwrap();
        let calls = fake.calls.lock().unwrap().clone();
        let uid = current_uid();
        assert_eq!(calls.len(), 2, "{calls:?}");
        assert_eq!(
            calls[0],
            format!("bootout gui/{uid}/dev.collide.test.unit.old-1")
        );
        assert!(
            calls[1].starts_with(&format!("bootstrap gui/{uid} ")),
            "{calls:?}"
        );
        // The record now names the new job only.
        assert_eq!(fs::read_to_string(run.join(LABEL_FILE)).unwrap(), job.label);
    }

    #[test]
    fn each_start_has_a_label_of_its_own() {
        let first = new_label("dev.collide.daemon");
        std::thread::sleep(Duration::from_millis(2));
        assert_ne!(first, new_label("dev.collide.daemon"));
        assert!(first.starts_with("dev.collide.daemon."));
    }

    #[test]
    fn the_pid_is_read_from_the_print_line_and_a_job_without_one_has_none() {
        assert_eq!(
            pid_in_print("x = {\n\tstate = running\n\tpid = 4242\n}\n"),
            Some(4242)
        );
        assert_eq!(
            pid_in_print("x = {\n\tstate = not running\n\tlast exit code = 0\n}\n"),
            None
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn launchd_is_used_only_when_asked_or_when_the_root_is_not_overridden() {
        assert!(launchctl_from_env(None, None).is_some());
        assert!(launchctl_from_env(Some("/tmp/root"), None).is_none());
        assert!(launchctl_from_env(Some("/tmp/root"), Some("/bin/launchctl")).is_some());
        assert!(launchctl_from_env(None, Some("/x/fake")).is_some());
        assert!(launchctl_from_env(Some("/tmp/root"), Some("")).is_none());
    }

    /// A `launchctl` whose bootout fails, and which says whether the job is still loaded.
    #[derive(Debug)]
    struct Refusing {
        still_loaded: bool,
    }

    impl Launchctl for Refusing {
        fn bootstrap(&self, _domain: &str, _plist: &Path) -> io::Result<()> {
            Err(io::Error::other("not used"))
        }

        fn bootout(&self, _target: &str) -> io::Result<()> {
            Err(io::Error::other("Boot-out failed: 5: Input/output error"))
        }

        fn process_of(&self, _target: &str) -> io::Result<Option<u32>> {
            if self.still_loaded {
                Ok(Some(1))
            } else {
                Err(io::Error::other("Could not find service"))
            }
        }
    }

    #[test]
    fn the_recorded_job_is_booted_out_and_its_record_removed() {
        let dir = Dir::new();
        let request = request_in(&dir);
        let run = request.lock_file.parent().unwrap();
        fs::write(run.join(LABEL_FILE), "dev.collide.test.unit.rec-1\n").unwrap();
        let fake = Fake::default();
        let booted = boot_out_recorded_job(&request.lock_file, &fake).unwrap();
        assert_eq!(booted.as_deref(), Some("dev.collide.test.unit.rec-1"));
        assert_eq!(
            fake.calls.lock().unwrap().clone(),
            vec![format!(
                "bootout gui/{}/dev.collide.test.unit.rec-1",
                current_uid()
            )]
        );
        assert!(!run.join(LABEL_FILE).exists());
    }

    #[test]
    fn no_record_means_no_call_to_launchctl() {
        let dir = Dir::new();
        let request = request_in(&dir);
        let fake = Fake::default();
        assert_eq!(
            boot_out_recorded_job(&request.lock_file, &fake).unwrap(),
            None
        );
        assert!(fake.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn a_job_that_is_not_loaded_is_not_an_error_and_its_record_goes() {
        let dir = Dir::new();
        let request = request_in(&dir);
        let run = request.lock_file.parent().unwrap();
        fs::write(run.join(LABEL_FILE), "dev.collide.test.unit.gone-1").unwrap();
        let refusing = Refusing {
            still_loaded: false,
        };
        assert!(
            boot_out_recorded_job(&request.lock_file, &refusing)
                .unwrap()
                .is_some()
        );
        assert!(!run.join(LABEL_FILE).exists());
    }

    #[test]
    fn a_job_that_is_still_loaded_after_a_failed_bootout_is_an_error_and_its_record_stays() {
        let dir = Dir::new();
        let request = request_in(&dir);
        let run = request.lock_file.parent().unwrap();
        fs::write(run.join(LABEL_FILE), "dev.collide.test.unit.stuck-1").unwrap();
        let refusing = Refusing { still_loaded: true };
        assert!(boot_out_recorded_job(&request.lock_file, &refusing).is_err());
        assert!(run.join(LABEL_FILE).exists());
    }

    #[test]
    fn the_launchd_job_carries_the_runtime_marker_and_no_run_variables() {
        let dir = Dir::new();
        let mut request = request_in(&dir);
        for name in STRIPPED_ENV {
            request.env.insert(name.to_string(), "x".to_string());
        }
        let lookup = env_of(&[("USER", "x"), ("HOME", "/h")]);
        let job =
            launchd_job(&request, "dev.collide.test.unit.env-1".to_string(), &lookup).unwrap();
        for name in STRIPPED_ENV {
            assert!(!job.env.contains_key(name), "{name}");
        }
        assert_eq!(job.env.get(RUNTIME_MARKER).map(String::as_str), Some("1"));
    }

    #[test]
    fn a_setsid_start_strips_the_run_variables_and_sets_the_runtime_marker() {
        let dir = Dir::new();
        let out = dir.0.join("env.out");
        let exe = dir.0.join("collide-daemon");
        fs::write(&exe, format!("#!/bin/sh\nenv > '{}'\n", out.display())).unwrap();
        fs::set_permissions(&exe, fs::Permissions::from_mode(0o755)).unwrap();
        let mut request = StartRequest::new(exe, &DaemonPaths::under(&dir.0));
        for name in STRIPPED_ENV {
            request.env.insert(name.to_string(), "x".to_string());
        }
        let pid = start_with_setsid(&request).unwrap();
        let until = Instant::now() + Duration::from_secs(10);
        while reap_if_ended(pid).is_none() {
            assert!(Instant::now() < until, "the script did not end");
            std::thread::sleep(Duration::from_millis(10));
        }
        let text = fs::read_to_string(&out).unwrap();
        for name in STRIPPED_ENV {
            assert!(
                !text.lines().any(|l| l.starts_with(&format!("{name}="))),
                "{name} in {text}"
            );
        }
        assert!(text.lines().any(|l| l == "COLLIDE_RUNTIME=1"), "{text}");
    }

    #[test]
    fn a_request_made_by_new_starts_with_setsid_only() {
        let request = StartRequest::new(
            PathBuf::from("/x/collide-daemon"),
            &DaemonPaths::under(Path::new("/tmp/x")),
        );
        assert!(request.launchctl.is_none());
        assert_eq!(request.label_prefix, DEFAULT_LABEL_PREFIX);
    }
}
