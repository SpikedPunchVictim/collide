//! `collide-daemon`'s own parts (0.2; Stage 3a): the entry point, the socket and its security, and the adoption of
//! shims that outlived the last daemon. Starting the daemon is [`crate::launch`], its environment is
//! [`crate::base_env`], its storage is [`crate::store`], its clocks are [`crate::clock`] and its telemetry receiver
//! is [`crate::otlp`]. The binary (`host/src/bin/collide-daemon.rs`) calls [`run`], or [`crate::launch::ensure_daemon`]
//! for its `start` argument.
//!
//! ## What the daemon does at start, in this order
//!
//! 1. Raises its soft open-file limit ([`MIN_OPEN_FILES`], [`raise_open_file_limit`]): launchd's default of 256 broke
//!    herdr at about 40 panes, and the daemon tails three files per child.
//! 2. Records its login session, and restarts in a new one. Ending at logout is recorded as host shutdown, not as a
//!    crash. (Not built yet: it is the daemon's own session to record, and no later item needs it before Stage 3b.)
//! 3. Opens the store. A store that cannot be read is moved aside and reported ([`StoreError::Quarantined`]), a fresh
//!    one is made, and the daemon keeps running with the report in `daemon.status`. It will **adopt existing shims**
//!    ([`adopt_shims`], `ts-0057-ck`) here, before it binds the socket, so no client sees a run as gone while its
//!    shim is alive.
//! 4. Checks the socket's path and directory ([`crate::auth::prepare_daemon_socket`]), makes or reads the CLI's token
//!    file, and binds ([`bind_socket`]). A socket another daemon answers on is not taken.
//! 5. Starts the capture of the base environment on a thread of its own, so the socket answers at once and a login
//!    shell that takes seconds does not delay the start ([`crate::base_env`]).
//!
//! Then it serves ([`Daemon`]): one thread accepts, and one thread serves each connection, so a slow or hostile
//! client holds up only its own thread. The runtime is `std` threads and not tokio, because the plan's tokio is for
//! reaping shims (`ts-0057-ck`) and nothing here waits on more than a socket.
//!
//! ## The socket
//!
//! At a short fixed path, because an `AF_UNIX` path is limited to [`MAX_SOCKET_PATH_BYTES`] bytes on macOS (measured
//! S3), and a longer path fails fast with a message naming the path and the limit ([`SocketError::PathTooLong`]).
//!
//! - Its directory is mode `0700` and the daemon checks the owner and mode at start, refusing to bind in a directory
//!   that is not the user's or is open to others ([`crate::auth::check_socket_dir`]).
//! - A connecting peer's uid must equal the daemon's (`getpeereid`), so a second local user never reaches the token
//!   check ([`SocketError::ForeignUid`], [`crate::auth::admit`]).
//! - The CLI's token is in a file of mode `0600` in that directory, made at the daemon's first start
//!   ([`TOKEN_FILE_MODE`], [`crate::auth::create_token_file`]). It is never put in an environment variable.
//! - A shim's socket follows the same rules with no token, because only the daemon talks to it
//!   ([`crate::auth::prepare_shim_socket`]).
//!
//! The rules themselves, and the actor, are [`crate::auth`]'s: pure code over traits. This module binds, accepts and
//! serves, and calls them.
//!
//! ## The wire
//!
//! The owner's JSON-RPC of `core` ([`collide_core::owner`]): one JSON message per line, at most
//! [`collide_core::owner::MAX_MESSAGE_BYTES`]. The first request of a connection is `initialize`: the uid is checked
//! first, then the token, then the protocol version, and a connection that fails any of them is answered once and
//! closed, so an unauthenticated peer learns no version. A connection that says nothing for [`HANDSHAKE_WAIT`] is
//! closed. Only `daemon.status` and `daemon.recapture_environment` are served here; the other methods are other items'
//! and answer `method not found`.
//!
//! ## Who is asking
//!
//! A peer's pid is read with `LOCAL_PEERPID`, and the daemon walks that process's parents looking for the markers:
//! `COLLIDE_RUN` makes a run's agent, the app's process makes the app, and anything else is the CLI from a terminal
//! ([`crate::auth::Caller`]). The result is the `actor` recorded on every event, and only the app does the person-only
//! operations ([`crate::auth::judge`]). This is a guardrail (0.1), not a boundary.
//!
//! ## The log
//!
//! The daemon writes its log through a [`Write`] it is given, one line each, and a write that fails (a full disk is
//! the case that is tested, with a writer that fails with `ENOSPC` on demand) is counted and dropped:
//! [`Daemon::log_failures`]. Logging never panics and never stops the daemon (0.2, Durability).
//!
//! ## Where the files are
//!
//! [`DaemonPaths`]. Every path is derived from one root that the environment can set ([`ROOT_ENV`]), which is how a
//! test runs a daemon in a folder of its own and never touches the user's. Without it, the defaults are
//! [`DaemonPaths::for_home`].
//!
//! | tag | function | what it does |
//! |---|---|---|
//! | `daemon: entry` | [`run`], [`Daemon::start`] | the start sequence above, then the serve loop |
//! | `daemon: socket server` | [`bind_socket`], [`Daemon`] | the bind, the handshake and the JSON-RPC loop, over [`crate::auth`]'s checks |
//! | `daemon: adopt shims` | [`adopt_shims`] | reattach by pid and start time, read `exit.json` for the ones that ended |

use crate::auth::{self, Admitted, Session, StdFs, SystemProcessTree, UnixPeer};
use crate::base_env::{BaseEnvironment, CaptureOptions, EnvSource};
use crate::clock::{Clock, Clocks, Reading, SystemClocks};
use crate::store::{Store, StoreError};
use collide_core::owner::{
    DaemonRecaptureEnvironmentResult, DaemonStatusResult, EnvCapture, MAX_MESSAGE_BYTES, Method,
    OWNER_PROTOCOL, OwnerInitializeResult, Request, Response, Routed, RpcError, StoreHealth, code,
    encode_line, parse_request, route,
};
use collide_core::shim::ExitRecord;
use std::fmt;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::os::unix::io::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// The longest an `AF_UNIX` socket path may be on macOS, in bytes, without the terminating NUL (measured S3).
pub const MAX_SOCKET_PATH_BYTES: usize = 103;

/// The open-file soft limit the daemon raises itself to. The plan names launchd's 256 as too low and gives no target;
/// this is room for the three files of each of about a hundred children.
pub const MIN_OPEN_FILES: u64 = 4096;

/// The mode of the CLI's token file (0.2).
pub const TOKEN_FILE_MODE: u32 = 0o600;

/// The mode of the socket's directory (0.2).
pub const SOCKET_DIR_MODE: u32 = 0o700;

/// The environment variable that moves every path of the daemon below one folder ([`DaemonPaths::from_env`]).
pub const ROOT_ENV: &str = "COLLIDE_DAEMON_ROOT";

/// How long a connection may take to send a whole `initialize` before it is closed. The plan sets none; a client that
/// has connected has the line ready, so this is for a peer that connects and says nothing.
pub const HANDSHAKE_WAIT: Duration = Duration::from_secs(10);

/// How many connections the daemon serves at once. Each has a thread and a descriptor, and the plan sets no number;
/// the app, the CLI and a run's agents are a handful.
pub const MAX_CONNECTIONS: usize = 256;

/// How often a blocked read and the accept loop look at whether the daemon is stopping.
const STOP_CHECK: Duration = Duration::from_millis(100);

/// How long a write to one client may block. A client that does not read is closed and not waited for.
const WRITE_WAIT: Duration = Duration::from_secs(5);

/// How long the daemon reads and drops what a client sent after a line that was too long, so that the answer is
/// delivered before the connection closes.
const DRAIN_WAIT: Duration = Duration::from_millis(500);

/// Set by the signal handler of [`run`]; the accept loop and every connection look at it.
static SIGNALED: AtomicBool = AtomicBool::new(false);

/// Where the daemon's files are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonPaths {
    /// The directory the socket and the token file are in, mode `0700`.
    pub socket_dir: PathBuf,
    pub socket: PathBuf,
    /// The CLI's token, mode `0600`.
    pub token_file: PathBuf,
    /// The lock file that serializes starts ([`crate::launch`]).
    pub lock_file: PathBuf,
    /// The directory that holds one run directory for each run, which the shims write into.
    pub runs_dir: PathBuf,
    /// The store's directory ([`Store::open`]).
    pub store_dir: PathBuf,
    /// The daemon's own log, which a start redirects the daemon's output to and the app shows when starts fail.
    pub log_file: PathBuf,
}

impl DaemonPaths {
    /// Every path below `root`: the socket, the token and the lock file in `root/run`, and the runs, the store and the
    /// log in `root/data`. `root/run/d.sock` has to fit in [`MAX_SOCKET_PATH_BYTES`].
    pub fn under(root: &Path) -> DaemonPaths {
        let run = root.join("run");
        let data = root.join("data");
        DaemonPaths {
            socket: run.join("d.sock"),
            token_file: run.join("token"),
            lock_file: run.join("lock"),
            socket_dir: run,
            runs_dir: data.join("runs"),
            store_dir: data.join("store"),
            log_file: data.join("daemon.log"),
        }
    }

    /// The user's own locations, which a real daemon uses: the socket, the token and the lock file in `~/.collide/run`
    /// (short, and not in a folder macOS cleans out), and the runs, the store and the log in
    /// `~/Library/Application Support/Collide`.
    pub fn for_home(home: &Path) -> DaemonPaths {
        let run = home.join(".collide").join("run");
        let data = home
            .join("Library")
            .join("Application Support")
            .join("Collide");
        DaemonPaths {
            socket: run.join("d.sock"),
            token_file: run.join("token"),
            lock_file: run.join("lock"),
            socket_dir: run,
            runs_dir: data.join("runs"),
            store_dir: data.join("store"),
            log_file: data.join("daemon.log"),
        }
    }

    /// The paths named by [`ROOT_ENV`] when it is set, and the user's own otherwise. `home` is `$HOME`.
    pub fn from_env(root: Option<&str>, home: Option<&str>) -> Result<DaemonPaths, String> {
        match (root, home) {
            (Some(root), _) if !root.is_empty() => Ok(DaemonPaths::under(Path::new(root))),
            (_, Some(home)) if !home.is_empty() => Ok(DaemonPaths::for_home(Path::new(home))),
            _ => Err(format!(
                "neither {ROOT_ENV} nor HOME is set, so there is no place for the daemon's files"
            )),
        }
    }
}

/// Why the daemon could not start or serve. Each message names the path or the limit.
#[derive(Debug)]
pub enum SocketError {
    /// The socket path is over [`MAX_SOCKET_PATH_BYTES`].
    PathTooLong {
        path: PathBuf,
        len: usize,
    },
    /// The directory is not the daemon's, or its mode lets others in (or is not `0700` at all).
    UnsafeDir {
        dir: PathBuf,
        mode: u32,
        owner_uid: u32,
        daemon_uid: u32,
    },
    /// Where the socket's directory should be there is a file, or a link.
    NotADirectory {
        path: PathBuf,
    },
    /// The token file's mode is not [`TOKEN_FILE_MODE`], so it is not read.
    UnsafeTokenFile {
        path: PathBuf,
        mode: u32,
    },
    /// A peer's uid is not the daemon's.
    ForeignUid {
        uid: u32,
        daemon_uid: u32,
    },
    /// A daemon already answers on the socket, so this one did not take it.
    AlreadyRunning {
        socket: PathBuf,
    },
    /// The store could not be opened, and it was not one that is moved aside (a store from a newer build, a disk
    /// error). The daemon does not run without its store.
    Store(StoreError),
    Io {
        context: String,
        source: io::Error,
    },
}

impl fmt::Display for SocketError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SocketError::PathTooLong { path, len } => write!(
                f,
                "the socket path {} is {len} bytes, over the {MAX_SOCKET_PATH_BYTES}-byte limit of a macOS socket path",
                path.display()
            ),
            SocketError::UnsafeDir {
                dir,
                mode,
                owner_uid,
                daemon_uid,
            } => write!(
                f,
                "the socket directory {} is mode {mode:04o} and owned by uid {owner_uid}, and must be mode {SOCKET_DIR_MODE:04o} and owned by uid {daemon_uid}, the daemon's user",
                dir.display()
            ),
            SocketError::NotADirectory { path } => write!(
                f,
                "the socket directory {} is not a directory (a file or a link is there)",
                path.display()
            ),
            SocketError::UnsafeTokenFile { path, mode } => write!(
                f,
                "the token file {} is mode {mode:04o}, and must be a file of mode {TOKEN_FILE_MODE:04o}, so it was not read",
                path.display()
            ),
            SocketError::ForeignUid { uid, daemon_uid } => write!(
                f,
                "a connection from uid {uid} is refused: the daemon's user is uid {daemon_uid}"
            ),
            SocketError::AlreadyRunning { socket } => write!(
                f,
                "a daemon already answers on {}, so this one did not start",
                socket.display()
            ),
            SocketError::Store(error) => write!(f, "the store could not be opened: {error}"),
            SocketError::Io { context, source } => write!(f, "{context}: {source}"),
        }
    }
}

impl std::error::Error for SocketError {}

/// An existing shim, as the daemon finds it at start: which run, which process, and either the exit record it left
/// or the fact that it is still alive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdoptedShim {
    pub run: String,
    pub pid: u32,
    pub state: AdoptedState,
}

/// What adoption found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdoptedState {
    /// The pid is alive and its start time matches the one recorded, so it is the same shim.
    Alive,
    /// The shim ended and left `exit.json`. The daemon takes the totals from it.
    Exited(ExitRecord),
    /// The shim is gone and left no `exit.json`: its phase is `lost` (0.2). When the child is still running, the
    /// daemon stops it as it marks the phase `lost`.
    Lost,
}

// ---- the open-file limit ----------------------------------------------------------------------------------------

/// What [`raise_open_file_limit`] found and did: the soft limit before and after, and the hard limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenFileLimit {
    pub soft_before: u64,
    pub soft_after: u64,
    pub hard: u64,
}

/// The soft limit to set: `want`, held to the hard limit, and only when it is above the soft one (the limit is never
/// lowered). `None` when nothing needs to change.
pub fn open_file_target(soft: u64, hard: u64, want: u64) -> Option<u64> {
    let target = want.min(hard);
    (target > soft).then_some(target)
}

/// Raises the soft open-file limit to `want`, as far as the hard limit allows (`getrlimit`, `setrlimit`).
pub fn raise_open_file_limit(want: u64) -> io::Result<OpenFileLimit> {
    let mut limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: `getrlimit` writes one `rlimit` through the pointer, which is to a value that lives for the call.
    if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) } != 0 {
        return Err(io::Error::last_os_error());
    }
    // `rlim_t` is `u64` on macOS and differs elsewhere, so the cast stays.
    #[allow(clippy::unnecessary_cast)]
    let (soft, hard) = (limit.rlim_cur as u64, limit.rlim_max as u64);
    let Some(target) = open_file_target(soft, hard, want) else {
        return Ok(OpenFileLimit {
            soft_before: soft,
            soft_after: soft,
            hard,
        });
    };
    limit.rlim_cur = target as libc::rlim_t;
    // SAFETY: `setrlimit` reads one `rlimit` through the pointer, which is to a value that lives for the call.
    if unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &limit) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(OpenFileLimit {
        soft_before: soft,
        soft_after: target,
        hard,
    })
}

// ---- the log ----------------------------------------------------------------------------------------------------

/// The daemon's log: lines through a writer that may fail. A failure is counted and dropped, and nothing here panics.
struct DaemonLog {
    sink: Mutex<Box<dyn Write + Send>>,
    failures: AtomicU64,
    clocks: Arc<dyn Clocks + Send + Sync>,
}

impl DaemonLog {
    fn line(&self, text: &str) {
        let ms = self.clocks.read(Clock::Wall).nanos / 1_000_000;
        let line = format!("{ms} {text}\n");
        // A poisoned lock only means a writer panicked once; the log is still the log.
        let mut sink = self.sink.lock().unwrap_or_else(|e| e.into_inner());
        if sink
            .write_all(line.as_bytes())
            .and_then(|()| sink.flush())
            .is_err()
        {
            self.failures.fetch_add(1, Ordering::SeqCst);
        }
    }
}

// ---- the daemon -------------------------------------------------------------------------------------------------

/// Everything the daemon is started with. [`DaemonOptions::new`] gives the real defaults; a test replaces the log, the
/// clocks, the capture's timeout and the shell.
pub struct DaemonOptions {
    pub paths: DaemonPaths,
    /// The login shell the base environment is captured from (`$SHELL`).
    pub shell: String,
    pub capture: CaptureOptions,
    /// The app's executable, so a peer below it is the app ([`crate::auth::SystemProcessTree`]).
    pub app_executable: Option<PathBuf>,
    pub clocks: Arc<dyn Clocks + Send + Sync>,
    /// Where the log goes.
    pub log: Box<dyn Write + Send>,
    pub handshake_wait: Duration,
    /// The open-file limit asked for at start.
    pub min_open_files: u64,
}

impl DaemonOptions {
    pub fn new(paths: DaemonPaths, shell: String, log: Box<dyn Write + Send>) -> DaemonOptions {
        DaemonOptions {
            paths,
            shell,
            capture: CaptureOptions::default(),
            app_executable: None,
            clocks: Arc::new(SystemClocks),
            log,
            handshake_wait: HANDSHAKE_WAIT,
            min_open_files: MIN_OPEN_FILES,
        }
    }
}

/// What every thread of the daemon shares.
struct Shared {
    paths: DaemonPaths,
    token: String,
    store: Store,
    health: StoreHealth,
    warnings: Mutex<Vec<String>>,
    env: Mutex<Option<BaseEnvironment>>,
    shell: String,
    capture: CaptureOptions,
    tree: SystemProcessTree,
    uid: u32,
    clocks: Arc<dyn Clocks + Send + Sync>,
    log: DaemonLog,
    handshake_wait: Duration,
    started_wall_ms: u64,
    started_uptime: Reading,
    stop: AtomicBool,
    connections: AtomicUsize,
}

impl Shared {
    fn stopping(&self) -> bool {
        self.stop.load(Ordering::SeqCst) || SIGNALED.load(Ordering::SeqCst)
    }

    fn warn(&self, text: String) {
        self.warnings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(text);
    }

    /// Captures the environment now and keeps the newest good capture. A capture that fell back does not replace one
    /// the login shell gave: a failed second capture should not make the daemon forget a working `PATH`.
    fn capture_environment(&self) -> BaseEnvironment {
        let new = BaseEnvironment::capture_with(&self.shell, self.clocks.as_ref(), &self.capture);
        self.log.line(&new.source.describe());
        let mut current = self.env.lock().unwrap_or_else(|e| e.into_inner());
        let keep_old = matches!(new.source, EnvSource::Minimal { .. })
            && matches!(
                current.as_ref().map(|e| &e.source),
                Some(EnvSource::LoginShell { .. })
            );
        if let EnvSource::Minimal { reason, .. } = &new.source {
            self.warn(format!("the base environment is the minimal one: {reason}"));
        }
        if keep_old {
            self.log
                .line("the earlier capture from the login shell is kept");
        } else {
            *current = Some(new.clone());
        }
        new
    }
}

/// A running daemon: the accept thread and what it shares. Dropping it stops it.
pub struct Daemon {
    shared: Arc<Shared>,
    accept: Option<JoinHandle<()>>,
}

impl Daemon {
    /// Runs the start sequence of the module doc and returns once the socket is bound and accepting.
    pub fn start(options: DaemonOptions) -> Result<Daemon, SocketError> {
        let DaemonOptions {
            paths,
            shell,
            capture,
            app_executable,
            clocks,
            log,
            handshake_wait,
            min_open_files,
        } = options;
        let log = DaemonLog {
            sink: Mutex::new(log),
            failures: AtomicU64::new(0),
            clocks: clocks.clone(),
        };
        log.line(&format!(
            "daemon {} starting, pid {}",
            env!("CARGO_PKG_VERSION"),
            std::process::id()
        ));
        let mut warnings = Vec::new();

        // Fail fast, before anything is made: the path's length, then the directory.
        auth::prepare_daemon_socket(&StdFs, auth::daemon_uid(), &paths)
            .inspect_err(|e| log.line(&format!("cannot start: {e}")))?;

        match raise_open_file_limit(min_open_files) {
            Ok(limit) if limit.soft_after < min_open_files => {
                let text = format!(
                    "the open-file limit is {} and the hard limit is {}, under the {min_open_files} the daemon wants",
                    limit.soft_after, limit.hard
                );
                log.line(&text);
                warnings.push(text);
            }
            Ok(limit) => log.line(&format!(
                "open-file soft limit {} (was {})",
                limit.soft_after, limit.soft_before
            )),
            Err(error) => {
                let text = format!("the open-file limit could not be raised: {error}");
                log.line(&text);
                warnings.push(text);
            }
        }

        let (store, health) = open_store(&paths, &log, &mut warnings)?;
        let token = load_or_make_token(&paths.token_file)?;
        let listener =
            bind_socket(&paths).inspect_err(|e| log.line(&format!("cannot start: {e}")))?;
        log.line(&format!("listening on {}", paths.socket.display()));

        let uid = auth::daemon_uid();
        let shared = Arc::new(Shared {
            paths,
            token,
            store,
            health,
            warnings: Mutex::new(warnings),
            env: Mutex::new(None),
            shell,
            capture,
            tree: SystemProcessTree::new(app_executable),
            uid,
            started_wall_ms: clocks.read(Clock::Wall).nanos / 1_000_000,
            started_uptime: clocks.read(Clock::Uptime),
            clocks,
            log,
            handshake_wait,
            stop: AtomicBool::new(false),
            connections: AtomicUsize::new(0),
        });

        let capturing = shared.clone();
        spawn_named("collide-env-capture", move || {
            capturing.capture_environment();
        })
        .map_err(|source| SocketError::Io {
            context: "could not start the environment capture's thread".to_string(),
            source,
        })?;

        let serving = shared.clone();
        let accept = spawn_named("collide-accept", move || accept_loop(&serving, &listener))
            .map_err(|source| SocketError::Io {
                context: "could not start the accept thread".to_string(),
                source,
            })?;
        Ok(Daemon {
            shared,
            accept: Some(accept),
        })
    }

    pub fn paths(&self) -> &DaemonPaths {
        &self.shared.paths
    }

    /// The newest base environment, as a value a phase copies. `None` until the first capture ends.
    pub fn base_environment(&self) -> Option<BaseEnvironment> {
        self.shared
            .env
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// How many log lines a failing writer has dropped.
    pub fn log_failures(&self) -> u64 {
        self.shared.log.failures.load(Ordering::SeqCst)
    }

    /// Stops accepting, closes the connections as they notice, and removes the socket. The token file stays: it is
    /// made once, at the first start.
    pub fn stop(mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        self.join();
    }

    /// Blocks until the daemon is stopped, by [`Daemon::stop`] from another thread or by a signal ([`run`]).
    pub fn wait(mut self) {
        self.join();
    }

    fn join(&mut self) {
        if let Some(accept) = self.accept.take() {
            let _ = accept.join();
        }
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        self.join();
    }
}

fn spawn_named(name: &str, body: impl FnOnce() + Send + 'static) -> io::Result<JoinHandle<()>> {
    std::thread::Builder::new()
        .name(name.to_string())
        .spawn(body)
}

/// Opens the store. One that cannot be read has been moved aside by [`Store::open`], so it is opened again for the
/// fresh one and the move is the health the daemon reports. Any other failure ends the start.
fn open_store(
    paths: &DaemonPaths,
    log: &DaemonLog,
    warnings: &mut Vec<String>,
) -> Result<(Store, StoreHealth), SocketError> {
    match Store::open(&paths.store_dir) {
        Ok(store) => Ok((store, StoreHealth::Ok {})),
        Err(StoreError::Quarantined { moved_to, reason }) => {
            let text = format!(
                "the store could not be read ({reason}); it was moved to {}, and a new store was made",
                moved_to.display()
            );
            log.line(&text);
            warnings.push(text);
            let store = Store::open(&paths.store_dir).map_err(SocketError::Store)?;
            Ok((
                store,
                StoreHealth::Quarantined {
                    moved_to: moved_to.display().to_string(),
                    reason,
                },
            ))
        }
        Err(error) => {
            log.line(&format!("cannot start: {error}"));
            Err(SocketError::Store(error))
        }
    }
}

/// The CLI's token: the one in the token file when there is a file (after the mode check), and a new one, written
/// with mode `0600`, at the first start. A token that is empty is refused, since it would match an empty one.
fn load_or_make_token(path: &Path) -> Result<String, SocketError> {
    let existing = match std::fs::symlink_metadata(path) {
        Ok(_) => true,
        Err(e) if e.kind() == io::ErrorKind::NotFound => false,
        Err(source) => {
            return Err(SocketError::Io {
                context: format!("could not look at the token file {}", path.display()),
                source,
            });
        }
    };
    if !existing {
        // The directory is the socket's, made by the launcher or by [`bind_socket`]'s check; make it here too, so a
        // daemon started by hand finds it.
        if let Some(dir) = path.parent() {
            auth::check_socket_dir(&StdFs, auth::daemon_uid(), dir)?;
        }
        match auth::create_token_file(path, &auth::mint_token()?) {
            Ok(()) => {}
            Err(SocketError::Io { source, .. })
                if source.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
    }
    let text = auth::read_token_file(path)?;
    let token = text.trim().to_string();
    if token.is_empty() {
        return Err(SocketError::Io {
            context: format!("the token file {} is empty", path.display()),
            source: io::Error::from(io::ErrorKind::InvalidData),
        });
    }
    Ok(token)
}

/// Checks the socket's path and directory ([`crate::auth::prepare_daemon_socket`]), and binds it. A file left by a
/// daemon that died is removed; a socket that answers is another daemon's and is not taken
/// ([`SocketError::AlreadyRunning`]).
pub fn bind_socket(paths: &DaemonPaths) -> Result<UnixListener, SocketError> {
    auth::prepare_daemon_socket(&StdFs, auth::daemon_uid(), paths)?;
    let io_error = |context: String| move |source| SocketError::Io { context, source };
    match std::fs::symlink_metadata(&paths.socket) {
        Ok(meta) if meta.file_type().is_socket() => {
            if UnixStream::connect(&paths.socket).is_ok() {
                return Err(SocketError::AlreadyRunning {
                    socket: paths.socket.clone(),
                });
            }
            std::fs::remove_file(&paths.socket).map_err(io_error(format!(
                "could not remove the stale socket {}",
                paths.socket.display()
            )))?;
        }
        Ok(_) => {
            return Err(SocketError::Io {
                context: format!(
                    "{} is there and is not a socket, so it was not replaced",
                    paths.socket.display()
                ),
                source: io::Error::from(io::ErrorKind::AlreadyExists),
            });
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(SocketError::Io {
                context: format!("could not look at {}", paths.socket.display()),
                source,
            });
        }
    }
    let listener = UnixListener::bind(&paths.socket).map_err(io_error(format!(
        "could not bind the socket {}",
        paths.socket.display()
    )))?;
    // The directory is `0700` already; the socket file is made private as well.
    std::fs::set_permissions(&paths.socket, std::fs::Permissions::from_mode(0o600)).map_err(
        io_error(format!(
            "could not set the mode of the socket {}",
            paths.socket.display()
        )),
    )?;
    listener.set_nonblocking(true).map_err(io_error(
        "could not make the listener non-blocking".to_string(),
    ))?;
    Ok(listener)
}

/// Accepts until the daemon stops, a thread for each connection, and removes the socket at the end.
fn accept_loop(shared: &Arc<Shared>, listener: &UnixListener) {
    while !shared.stopping() {
        let mut poll = libc::pollfd {
            fd: listener.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: `poll` points at one `pollfd` that lives for the call, and the count says so.
        let ready = unsafe { libc::poll(&mut poll, 1, STOP_CHECK.as_millis() as i32) };
        if ready <= 0 {
            continue;
        }
        match listener.accept() {
            Ok((stream, _)) => admit_connection(shared, stream),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(e) => {
                shared.log.line(&format!("accept failed: {e}"));
                std::thread::sleep(STOP_CHECK);
            }
        }
    }
    let _ = std::fs::remove_file(&shared.paths.socket);
    shared.log.line("daemon stopped");
}

fn admit_connection(shared: &Arc<Shared>, stream: UnixStream) {
    if shared.connections.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
        shared.connections.fetch_sub(1, Ordering::SeqCst);
        shared.log.line(&format!(
            "a connection was refused: {MAX_CONNECTIONS} are open"
        ));
        return;
    }
    let serving = shared.clone();
    let started = spawn_named("collide-connection", move || {
        serve_connection(&serving, stream);
        serving.connections.fetch_sub(1, Ordering::SeqCst);
    });
    if let Err(e) = started {
        shared.connections.fetch_sub(1, Ordering::SeqCst);
        shared
            .log
            .line(&format!("could not start a connection's thread: {e}"));
    }
}

// ---- one connection ---------------------------------------------------------------------------------------------

/// What reading one line found.
enum Line {
    Text(String),
    /// The bytes were not UTF-8.
    NotText,
    /// More than [`MAX_MESSAGE_BYTES`] and no end of line.
    TooLong,
    Closed,
    Stopping,
    /// The deadline passed.
    TimedOut,
}

/// Reads newline-ended lines from a socket with a read timeout, keeping a partial line across timeouts and never
/// holding more than one message.
struct LineReader {
    reader: BufReader<UnixStream>,
    buf: Vec<u8>,
}

impl LineReader {
    fn next(&mut self, stopping: impl Fn() -> bool, deadline: Option<Instant>) -> io::Result<Line> {
        loop {
            let room = (MAX_MESSAGE_BYTES + 2).saturating_sub(self.buf.len()) as u64;
            match (&mut self.reader)
                .take(room)
                .read_until(b'\n', &mut self.buf)
            {
                Ok(0) => return Ok(Line::Closed),
                Ok(_) => {}
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) =>
                {
                    if stopping() {
                        return Ok(Line::Stopping);
                    }
                    if deadline.is_some_and(|d| Instant::now() >= d) {
                        return Ok(Line::TimedOut);
                    }
                    continue;
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
            if self.buf.last() == Some(&b'\n') {
                self.buf.pop();
                if self.buf.last() == Some(&b'\r') {
                    self.buf.pop();
                }
                let bytes = std::mem::take(&mut self.buf);
                return Ok(match String::from_utf8(bytes) {
                    Ok(text) => Line::Text(text),
                    Err(_) => Line::NotText,
                });
            }
            if self.buf.len() > MAX_MESSAGE_BYTES {
                self.buf = Vec::new();
                return Ok(Line::TooLong);
            }
            // EOF in the middle of a line, or a read that stopped for another reason: the next read says which.
        }
    }
}

fn send(stream: &mut UnixStream, response: &Response) -> io::Result<()> {
    stream.write_all(encode_line(response).as_bytes())
}

fn too_long() -> Response {
    Response::failure(
        None,
        RpcError::new(
            code::INVALID_REQUEST,
            format!("the message is over the {MAX_MESSAGE_BYTES}-byte limit"),
        ),
    )
}

fn not_text() -> Response {
    Response::failure(
        None,
        RpcError::new(code::PARSE_ERROR, "not JSON: the line is not UTF-8 text"),
    )
}

/// After an answer that ends a connection with bytes still coming, reads and drops them for a moment, so that the
/// answer reaches the client before the close.
fn drain_then_close(stream: &mut UnixStream) {
    let _ = stream.shutdown(std::net::Shutdown::Write);
    let _ = stream.set_read_timeout(Some(STOP_CHECK));
    let until = Instant::now() + DRAIN_WAIT;
    let mut sink = [0u8; 65536];
    while Instant::now() < until {
        match stream.read(&mut sink) {
            Ok(0) => break,
            Ok(_) => {}
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) => {}
            Err(_) => break,
        }
    }
}

fn serve_connection(shared: &Shared, mut stream: UnixStream) {
    let pid = crate::sysproc::peer_pid(&stream).ok();
    let admitted = match auth::admit(shared.uid, &UnixPeer { stream: &stream }, &shared.tree) {
        Ok(admitted) => admitted,
        Err(refused) => {
            shared
                .log
                .line(&format!("a connection was refused: {refused}"));
            return;
        }
    };
    let configured = stream
        .set_nonblocking(false)
        .and_then(|()| stream.set_read_timeout(Some(STOP_CHECK)))
        .and_then(|()| stream.set_write_timeout(Some(WRITE_WAIT)));
    let reader = match configured.and_then(|()| stream.try_clone()) {
        Ok(clone) => clone,
        Err(e) => {
            shared
                .log
                .line(&format!("a connection could not be set up: {e}"));
            return;
        }
    };
    let mut lines = LineReader {
        reader: BufReader::new(reader),
        buf: Vec::new(),
    };
    let Some(session) = handshake(shared, &mut lines, &mut stream, admitted, pid) else {
        return;
    };
    shared.log.line(&format!(
        "pid {} connected as {:?}",
        pid.map_or("?".to_string(), |p| p.to_string()),
        session.actor().kind
    ));
    converse(shared, &mut lines, &mut stream, &session);
}

/// The first request: it must be `initialize`, with the token and then the version right. Anything else is answered
/// once and the connection is closed. Returns the session when it passed.
fn handshake(
    shared: &Shared,
    lines: &mut LineReader,
    stream: &mut UnixStream,
    admitted: Admitted,
    pid: Option<u32>,
) -> Option<Session> {
    let deadline = Instant::now() + shared.handshake_wait;
    let line = match lines.next(|| shared.stopping(), Some(deadline)) {
        Ok(Line::Text(line)) => line,
        Ok(Line::NotText) => {
            let _ = send(stream, &not_text());
            return None;
        }
        Ok(Line::TooLong) => {
            let _ = send(stream, &too_long());
            drain_then_close(stream);
            return None;
        }
        Ok(Line::TimedOut) => {
            shared.log.line(&format!(
                "a connection from pid {} said nothing for {} s and was closed",
                pid.map_or("?".to_string(), |p| p.to_string()),
                shared.handshake_wait.as_secs_f64()
            ));
            return None;
        }
        // A probe connects and leaves, or the daemon is stopping.
        Ok(Line::Closed | Line::Stopping) | Err(_) => return None,
    };
    let request = match parse_request(&line) {
        Ok(request) => request,
        Err(response) => {
            let _ = send(stream, &response);
            return None;
        }
    };
    let refuse = |stream: &mut UnixStream, error: RpcError| {
        shared.log.line(&format!(
            "initialize from pid {} refused: {}",
            pid.map_or("?".to_string(), |p| p.to_string()),
            error.message
        ));
        let _ = send(stream, &Response::failure(Some(request.id.clone()), error));
    };
    let params = match route(false, &request) {
        Ok(Routed::Initialize(params)) => params,
        Ok(Routed::Call(_)) => unreachable!("route(false, ..) never routes a call"),
        Err(error) => {
            refuse(stream, error);
            return None;
        }
    };
    match admitted.initialize(|| params, &shared.token) {
        Ok(session) => {
            let result = OwnerInitializeResult {
                protocol: OWNER_PROTOCOL,
                daemon_version: env!("CARGO_PKG_VERSION").to_string(),
                actor: session.actor(),
            };
            let value = serde_json::to_value(&result).expect("a result is JSON");
            send(stream, &Response::success(request.id, value)).ok()?;
            Some(session)
        }
        Err(error) => {
            refuse(stream, error);
            None
        }
    }
}

/// The requests after `initialize`, until the client closes or the daemon stops.
fn converse(shared: &Shared, lines: &mut LineReader, stream: &mut UnixStream, session: &Session) {
    loop {
        let line = match lines.next(|| shared.stopping(), None) {
            Ok(Line::Text(line)) => line,
            Ok(Line::NotText) => {
                if send(stream, &not_text()).is_err() {
                    return;
                }
                continue;
            }
            Ok(Line::TooLong) => {
                let _ = send(stream, &too_long());
                drain_then_close(stream);
                return;
            }
            Ok(Line::Closed | Line::Stopping | Line::TimedOut) | Err(_) => return,
        };
        let response = match parse_request(&line) {
            Err(response) => response,
            Ok(request) => answer(shared, session, request),
        };
        if send(stream, &response).is_err() {
            return;
        }
    }
}

fn answer(shared: &Shared, session: &Session, request: Request) -> Response {
    let id = request.id.clone();
    let result = match route(true, &request) {
        Err(error) => Err(error),
        Ok(Routed::Initialize(_)) => unreachable!("route(true, ..) refuses a second initialize"),
        Ok(Routed::Call(method)) => call(shared, session, method),
    };
    match result {
        Ok(value) => Response::success(id, value),
        Err(error) => Response::failure(Some(id), error),
    }
}

fn call(
    shared: &Shared,
    _session: &Session,
    method: Method,
) -> Result<serde_json::Value, RpcError> {
    match method {
        Method::DaemonStatus => {
            let uptime = shared.clocks.read(Clock::Uptime);
            let result = DaemonStatusResult {
                daemon_version: env!("CARGO_PKG_VERSION").to_string(),
                protocol: OWNER_PROTOCOL,
                pid: std::process::id(),
                started_ms: shared.started_wall_ms,
                uptime_secs: uptime.nanos.saturating_sub(shared.started_uptime.nanos)
                    / 1_000_000_000,
                head_seq: shared.store.last_seq().unwrap_or(0),
                live_runs: 0,
                store: shared.health.clone(),
                warnings: shared
                    .warnings
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone(),
            };
            Ok(serde_json::to_value(result).expect("a result is JSON"))
        }
        Method::DaemonRecaptureEnvironment => {
            let captured = shared.capture_environment();
            let capture = match captured.source {
                EnvSource::LoginShell { shell } => EnvCapture::Captured {
                    shell,
                    variables: captured.vars.len() as u32,
                },
                EnvSource::Minimal { shell, reason } => EnvCapture::FellBack {
                    shell: shell.unwrap_or_default(),
                    reason,
                },
            };
            Ok(
                serde_json::to_value(DaemonRecaptureEnvironmentResult { capture })
                    .expect("a result is JSON"),
            )
        }
        other => Err(RpcError::new(
            code::METHOD_NOT_FOUND,
            format!(
                "method not found: `{}` is not served by this daemon yet",
                other.name()
            ),
        )),
    }
}

// ---- running ----------------------------------------------------------------------------------------------------

extern "C" fn on_signal(_: libc::c_int) {
    SIGNALED.store(true, Ordering::SeqCst);
}

/// Runs the daemon until `SIGTERM` or `SIGINT`: the start sequence in the module doc, then the serve loop. `SIGHUP` is
/// left alone: a daemon that is in its launcher's session by mistake should end with it, and the start test says so.
pub fn run(options: DaemonOptions) -> Result<(), SocketError> {
    // SAFETY: `on_signal` only stores to an atomic, which is async-signal-safe, and `signal` takes the handler's
    // address as `sighandler_t`.
    unsafe {
        let handler = on_signal as extern "C" fn(libc::c_int) as libc::sighandler_t;
        libc::signal(libc::SIGTERM, handler);
        libc::signal(libc::SIGINT, handler);
    }
    Daemon::start(options)?.wait();
    Ok(())
}

/// Finds every shim of an earlier daemon under `runs_dir`: reattaches by pid plus start time, and reads `exit.json`
/// for the shims that ended while no daemon was running. It runs before the socket is bound.
pub fn adopt_shims(runs_dir: &Path) -> Result<Vec<AdoptedShim>, SocketError> {
    let _ = runs_dir;
    todo!("daemon: adopt shims")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_socket_path_error_names_the_path_and_the_limit() {
        let message = SocketError::PathTooLong {
            path: PathBuf::from("/very/long/path.sock"),
            len: 120,
        }
        .to_string();
        assert!(message.contains("/very/long/path.sock"), "{message}");
        assert!(message.contains("103"), "{message}");
    }

    #[test]
    fn an_unsafe_directory_error_names_the_directory_both_modes_and_both_owners() {
        let message = SocketError::UnsafeDir {
            dir: PathBuf::from("/tmp/collide"),
            mode: 0o755,
            owner_uid: 0,
            daemon_uid: 501,
        }
        .to_string();
        for part in ["/tmp/collide", "0755", "0700", "uid 0", "501"] {
            assert!(message.contains(part), "{part} in {message}");
        }
    }

    #[test]
    fn the_token_file_is_private_to_the_user() {
        assert_eq!(TOKEN_FILE_MODE, 0o600);
        assert_eq!(SOCKET_DIR_MODE, 0o700);
    }

    #[test]
    fn the_open_file_limit_is_raised_to_the_wish_and_held_to_the_hard_limit() {
        assert_eq!(open_file_target(256, 10_240, 4096), Some(4096));
        assert_eq!(open_file_target(256, 1024, 4096), Some(1024));
    }

    #[test]
    fn the_open_file_limit_is_never_lowered() {
        assert_eq!(open_file_target(8192, 10_240, 4096), None);
        assert_eq!(open_file_target(4096, 4096, 4096), None);
        assert_eq!(open_file_target(1024, 1024, 4096), None);
    }

    #[test]
    fn every_path_is_below_the_one_root_given() {
        let paths = DaemonPaths::under(Path::new("/tmp/x"));
        for path in [
            &paths.socket_dir,
            &paths.socket,
            &paths.token_file,
            &paths.lock_file,
            &paths.runs_dir,
            &paths.store_dir,
            &paths.log_file,
        ] {
            assert!(path.starts_with("/tmp/x"), "{}", path.display());
        }
        assert!(paths.socket.starts_with(&paths.socket_dir));
        assert!(paths.token_file.starts_with(&paths.socket_dir));
    }

    #[test]
    fn the_root_variable_wins_over_home_and_neither_is_an_error() {
        let paths = DaemonPaths::from_env(Some("/tmp/x"), Some("/Users/someone")).unwrap();
        assert!(paths.socket.starts_with("/tmp/x"));
        let paths = DaemonPaths::from_env(None, Some("/Users/someone")).unwrap();
        assert!(paths.socket.starts_with("/Users/someone/.collide/run"));
        assert!(DaemonPaths::from_env(None, None).is_err());
        assert!(DaemonPaths::from_env(Some(""), Some("")).is_err());
    }

    #[test]
    fn the_default_socket_fits_for_an_ordinary_home() {
        let paths = DaemonPaths::for_home(Path::new("/Users/someone-with-a-longer-name"));
        assert!(paths.socket.as_os_str().len() <= MAX_SOCKET_PATH_BYTES);
    }
}
