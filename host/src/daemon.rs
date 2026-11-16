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
//! 2. Records its login session ([`crate::login`]), and restarts in a new one. Ending at logout is recorded as host
//!    shutdown, not as a crash: the daemon's end writes a marker beside the runs, and the next start reads it
//!    ([`Daemon::previous_end`]) with the session it finds.
//! 3. Opens the store. A store that cannot be read is moved aside and reported ([`StoreError::Quarantined`]), a fresh
//!    one is made, and the daemon keeps running with the report in `daemon.status`. It **adopts existing shims**
//!    ([`adopt_shims`], [`Phases::adopt`]) here, before it binds the socket, so no client sees a run as gone while its
//!    shim is alive: the phases are settled from the event log, the shims' records and their processes, and a live
//!    shim gets a watcher that reconnects to it.
//! 4. Checks the socket's path and directory ([`crate::auth::prepare_daemon_socket`]), makes or reads the CLI's token
//!    file, and binds ([`bind_socket`]). A socket another daemon answers on is not taken.
//! 5. Starts the capture of the base environment on a thread of its own, so the socket answers at once and a login
//!    shell that takes seconds does not delay the start ([`crate::base_env`]).
//!
//! Then it serves ([`Daemon`]): one thread accepts, and one thread serves each connection, so a slow or hostile
//! client holds up only its own thread. The runtime is `std` threads and not tokio: the plan's tokio was for reaping
//! shims, and [`crate::phases`] does that with one blocked thread per shim the daemon started, which a handful of
//! phases does not strain.
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
//! closed. Only `daemon.status`, `daemon.recapture_environment`, `daemon.agent_versions` (the `claude` version in use and
//! the published one, [`crate::claude_versions`]), `run.stop`, `run.start_file` (a task file run with
//! the built-in builder role, [`crate::file_run`]), `run.start` and `run.diff` (a prompt run of `collide/quick` up to
//! its review, [`crate::file_run::quick_run`]), `queue.list` and `queue.answer` (the stored Queue,
//! [`crate::queue`]; accept on a review merges the run's branch into the checkout, [`crate::file_run`]), `board.open`
//! and `board.list` (a git checkout as a board, [`crate::boards`]), and `task.get` and `task.apply` for scratch tasks
//! ([`crate::scratch`]), `run.list` and `run.get` (from the store's index of runs, [`crate::run_store`]) and
//! `daemon.subscribe` are served here; the other methods are other items' and answer
//! `method not found`.
//!
//! ## The event stream
//!
//! `daemon.subscribe` ([`crate::event_hub`]) makes the connection a subscriber: the answer is written first, then a
//! thread of the connection writes the events after `since` and the live ones, and each `queue.changed`. Each
//! subscriber has a bounded queue ([`DaemonOptions::subscriber_queue`]), so a publisher never waits for a client. A
//! client that does not read until its queue is full is logged and its connection is closed for writing: it reads
//! what was sent, sees the end of the stream, and subscribes again from the last `seq` it has. All writes to a
//! connection go through one lock ([`Conn`]), so a notice is never inside an answer's line.
//!
//! ## Who is asking
//!
//! A peer's pid is read with `LOCAL_PEERPID`, and the daemon walks that process's parents looking for the markers:
//! `COLLIDE_RUN` makes a run's agent, the app's process makes the app, and anything else is the CLI from a terminal
//! ([`crate::auth::Caller`]). The result is the `actor` recorded on every event, and only the app does the person-only
//! operations ([`crate::auth::judge`]). This is a guardrail (0.1), not a boundary. The daemon knows which executable is
//! the app from the file the app writes in the run folder before each connection
//! ([`crate::auth::write_app_executable`]),
//! which it reads at each check, so a daemon that the CLI started first still takes the app as the app.
//!
//! ## Stopping a run, and stopping the daemon
//!
//! `run.stop` is [`Phases::stop_run`] ([`crate::stop`]): it answers when the stop is done, with the run's state, the
//! number of processes signalled and the survivors. A run's agent may stop its own run and no other. When the daemon
//! itself stops it ends its watchers and then boots out the launchd job its start recorded
//! ([`crate::launch::boot_out_recorded_job`], through [`DaemonOptions::launchctl`]); that is the last thing it does,
//! because booting the job out ends the process. The shims go on.
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
//! | `daemon: event stream` | [`Hub`], `subscribe` | `daemon.subscribe`, the replay, the live events, `queue.changed` |
//! | `daemon: adopt shims` | [`adopt_shims`], [`Phases::adopt`] | scan `runs_dir`, classify each shim by `exit.json` and by pid plus start time, reattach, settle |

use crate::auth::{
    self, Admitted, Caller, ProcessTree, Session, StdFs, SystemProcessTree, UnixPeer,
};
use crate::base_env::{BaseEnvironment, CaptureOptions, EnvSource};
use crate::boards;
use crate::claude_versions::{ClaudeVersions, PublishedChoice, REFRESH_EVERY, VersionsOptions};
use crate::clock::{Clock, ClockPair, Clocks, Reading, SystemClocks};
use crate::config_get::{
    BoardRef, ConfigService, GetError, LoadContextFactory, SetError, system_load_context,
};
use crate::disk::{Floor, FreeSpace, SystemFreeSpace};
use crate::event_hub::{Hub, Subscription};
use crate::file_run::quick_run::RunDiffError;
use crate::file_run::{AnswerError, FileRunConfig, FileRuns};
use crate::keychain::{Keychain, MemoryKeychain};
use crate::launch::{Launchctl, boot_out_recorded_job};
use crate::login::{self, LoginSessions, SystemLoginSession};
use crate::metrics::Metrics;
use crate::pack_location::Inputs;
use crate::phases::{
    BIND_WAIT, ExitJson, FeedRebuilder, PhaseEndHook, PhaseKey, PhaseOptions, PhasePauseHook,
    Phases, PhasesConfig, StopRunError, process_is_running, read_exit_json, read_identity,
};
use crate::queue::Queue;
use crate::run_store::{RunFilter, RunRow};
use crate::run_summary::summarize;
use crate::sampler::{Sampler, TICK};
use crate::scratch::{self, ScratchSource};
use crate::shim::{ChildIdentity, DaemonStart, files as shim_files};
use crate::stop::{Cleanup, Processes, SystemProcesses};
use crate::store::{Store, StoreError};
use collide_core::events::Event;
use collide_core::owner::{
    BoardListResult, BoardOpenParams, BoardOpenResult, ConfigGetParams, ConfigSetParams,
    DaemonRecaptureEnvironmentResult, DaemonStatusResult, DaemonSubscribeParams,
    DaemonSubscribeResult, EnvCapture, MAX_MESSAGE_BYTES, Method, Notice, OWNER_PROTOCOL,
    OwnerInitializeResult, QueueAnswerParams, QueueItem, QueueListParams, QueueListResult, Request,
    Response, Routed, RpcError, RunDiffParams, RunGetParams, RunGetResult, RunListParams,
    RunListResult, RunRecordParams, RunRecordResult, RunScope, RunStartFileParams, RunStartParams,
    RunStartResult, RunStopParams, RunStopResult, RunSummary, StoreHealth, Survivor,
    TaskApplyParams, TaskApplyResult, TaskGetParams, TaskGetResult, code, encode_line,
    parse_request, route,
};
use collide_core::ports::TaskProvider;
use collide_core::provider::{ApplyResult, GetParams};
use collide_core::shim::ExitRecord;
use std::collections::BTreeMap;
use std::fmt;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::os::unix::io::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
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

    /// The user's own locations, which a real daemon uses: everything in `~/Library/Application Support/Collide`, so
    /// that Collide makes no folder of its own in the home directory. The socket, the token and the lock file are in
    /// its `run` folder (short enough for [`MAX_SOCKET_PATH_BYTES`] for an ordinary home); the runs, the store and the
    /// log are beside it.
    pub fn for_home(home: &Path) -> DaemonPaths {
        let data = home
            .join("Library")
            .join("Application Support")
            .join("Collide");
        let run = data.join("run");
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

    /// The file the app writes its own executable path to, in the run folder ([`crate::auth::write_app_executable`]).
    pub fn app_executable_file(&self) -> PathBuf {
        self.socket_dir.join("app-executable")
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

/// An existing shim, as the daemon finds it at start: which phase, where its directory is, which process, and either
/// the exit record it left or the fact that it is still alive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdoptedShim {
    pub key: PhaseKey,
    /// The phase's directory, `<runs_dir>/<run>/<phase>`.
    pub dir: PathBuf,
    /// The shim's pid and start time from `shim.json`, when it left one.
    pub identity: Option<ChildIdentity>,
    pub state: AdoptedState,
}

/// What adoption found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdoptedState {
    /// The pid is alive and its start time matches the one recorded, so it is the same shim.
    Alive,
    /// The shim ended and left `exit.json`. The daemon takes the totals from it.
    Exited(ExitRecord),
    /// `exit.json` is there and is not a record (cut, empty, or another protocol). The daemon moves it aside and the
    /// phase is `lost`.
    Damaged { reason: String },
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
    /// The app's executable, so a peer below it is the app ([`crate::auth::SystemProcessTree`]). `None`, the default,
    /// is
    /// the app that writes its own path to the run folder ([`system_process_tree`]).
    pub app_executable: Option<PathBuf>,
    /// Who is asking is read from this tree of processes. `None` is the system's own ([`SystemProcessTree`] over
    /// `app_executable`). A test that runs the daemon in its own process gives a tree that stops at the test, because
    /// the test process is the peer's parent, and a test run inside a run of Collide has the run's `COLLIDE_RUN` in
    /// the environment it was started with, which no test can change (ts-0074-cu).
    pub process_tree: Option<Arc<dyn ProcessTree + Send + Sync>>,
    pub clocks: Arc<dyn Clocks + Send + Sync>,
    /// Where the log goes.
    pub log: Box<dyn Write + Send>,
    pub handshake_wait: Duration,
    /// The open-file limit asked for at start.
    pub min_open_files: u64,
    /// The `collide` binary that runs `collide shim <run-dir>`. Without it the daemon starts no phase (it still adopts).
    pub shim_exe: Option<PathBuf>,
    /// What a shim needs to start the daemon again, written into each phase's `spec.json`. `None` makes shims that
    /// never start one: the tests that run a shim without the full product.
    pub daemon_start: Option<DaemonStart>,
    /// The waits of the phases ([`PhaseOptions::default`]).
    pub phases: PhaseOptions,
    /// Called when a phase reaches a final state ([`PhaseEndHook`]).
    pub on_phase_end: Option<PhaseEndHook>,
    /// The process table a stop reads and signals ([`SystemProcesses`]); a test passes one that cannot signal a process
    /// it did not start.
    pub processes: Arc<dyn Processes + Send + Sync>,
    /// The workspace implementation's `cleanup` hook, run by a stop (Stage 7 makes it; `None` until then).
    pub cleanup: Option<Arc<dyn Cleanup + Send + Sync>>,
    /// The `launchctl` that boots out the daemon's recorded job when the daemon stops. `None`, which a daemon under an
    /// overridden root gets ([`crate::launch::launchctl_from_env`]), calls no `launchctl`.
    pub launchctl: Option<Arc<dyn Launchctl>>,
    /// The home directory `models.yaml` is read from, for a file-task run (`$HOME` unless a test gives its own).
    pub home: Option<String>,
    /// The folder the worktrees of file-task runs are made in. `None` is `workspaces` beside the runs.
    pub workspaces_dir: Option<PathBuf>,
    /// Where free space is read from, and the floor under which nothing starts (0.2, Durability).
    pub free_space: Arc<dyn FreeSpace + Send + Sync>,
    pub floor: Floor,
    /// Added to the environment of every git command a file-task run's workspace runs. A test passes
    /// `GIT_CONFIG_GLOBAL=/dev/null` here.
    pub git_env: Vec<(String, String)>,
    /// Where the daemon reads its login session ([`SystemLoginSession`]); a test passes one it can log out of.
    pub login: Arc<dyn LoginSessions>,
    /// How a usage limit's timer waits between two looks at the wall clock (`thread::sleep`). A test passes one that
    /// moves its fake clocks, as a machine that sleeps does.
    pub sleep: Arc<dyn Fn(Duration) + Send + Sync>,
    /// How often a limit's timer looks at the wall clock.
    pub limit_poll: Duration,
    /// The least a run stays paused for a limit before its reset time resumes it, on the continuous clock.
    pub limit_min_pause: Duration,
    /// How often the sampler reads the clocks ([`crate::sampler::TICK`]). A test passes a short one.
    pub sampler_tick: Duration,
    /// Where the published `claude` version comes from. `Off` (the default) never fetches: only the daemon's own binary
    /// asks for the registry, and only when no root is overridden (ts-0068-wx).
    pub claude_published: PublishedChoice,
    /// How often the published version is fetched again ([`REFRESH_EVERY`]). A test passes a short one.
    pub claude_refresh_every: Duration,
    /// How many notices a subscriber may have waiting before the daemon ends its subscription ([`SUBSCRIBER_QUEUE`]).
    /// A test passes a small one.
    pub subscriber_queue: usize,
    /// Rebuilds the feed of an agent phase after a restart, tried before the file runs' own (Claude's): `None`, the
    /// default, leaves a phase that is not a file run's with no session. A test passes the ACP adapter's here, until
    /// Stage 6b chooses the adapter of a run.
    pub feed_rebuilder: Option<FeedRebuilder>,
    /// Makes the loader's context for `config.get` (the cache, the SDK, the built-in packs). `None`, the default,
    /// finds the bundle from the process and keeps the cache beside the runs. A test passes one over fixtures.
    pub load_context: Option<LoadContextFactory>,
    /// Where `config.set --secret` keeps a secret and the load checks look for one. [`DaemonOptions::new`] gives a
    /// store in memory, so no daemon touches the OS keychain unless it is given one: `collide-daemon` gives
    /// [`crate::keychain::system_keychain`] when it runs for a person (ts-0099-bw).
    pub keychain: Arc<dyn Keychain>,
}

/// Who is asking, as the daemon reads it when its options give no tree: the system's own answer, with the app known by
/// `app_executable` when the options name it, and otherwise by the file the app writes in the run folder
/// ([`auth::write_app_executable`]). A test that runs the daemon in its own process wraps this to stop at the test.
pub fn system_process_tree(
    paths: &DaemonPaths,
    app_executable: Option<PathBuf>,
) -> SystemProcessTree {
    SystemProcessTree::new(app_executable).with_app_file(paths)
}

/// How many notices a subscribed connection may have waiting ([`DaemonOptions::subscriber_queue`]).
pub const SUBSCRIBER_QUEUE: usize = 1024;

/// How often a limit's timer looks at the wall clock ([`DaemonOptions::limit_poll`]).
pub const LIMIT_POLL: Duration = Duration::from_secs(1);

/// The least a run stays paused for a limit ([`DaemonOptions::limit_min_pause`]): a reset time that is already past must
/// not resume a run into the same limit again at once.
pub const LIMIT_MIN_PAUSE: Duration = Duration::from_secs(30);

impl DaemonOptions {
    pub fn new(paths: DaemonPaths, shell: String, log: Box<dyn Write + Send>) -> DaemonOptions {
        DaemonOptions {
            paths,
            shell,
            capture: CaptureOptions::default(),
            app_executable: None,
            process_tree: None,
            clocks: Arc::new(SystemClocks),
            log,
            handshake_wait: HANDSHAKE_WAIT,
            min_open_files: MIN_OPEN_FILES,
            shim_exe: None,
            daemon_start: None,
            phases: PhaseOptions::default(),
            on_phase_end: None,
            processes: Arc::new(SystemProcesses),
            cleanup: None,
            launchctl: None,
            home: std::env::var("HOME").ok(),
            workspaces_dir: None,
            free_space: Arc::new(SystemFreeSpace),
            floor: Floor::default(),
            git_env: Vec::new(),
            login: Arc::new(SystemLoginSession),
            sleep: Arc::new(std::thread::sleep),
            limit_poll: LIMIT_POLL,
            limit_min_pause: LIMIT_MIN_PAUSE,
            sampler_tick: TICK,
            claude_published: PublishedChoice::Off,
            claude_refresh_every: REFRESH_EVERY,
            subscriber_queue: SUBSCRIBER_QUEUE,
            feed_rebuilder: None,
            load_context: None,
            keychain: Arc::new(MemoryKeychain::new()),
        }
    }
}

/// What every thread of the daemon shares.
struct Shared {
    paths: DaemonPaths,
    token: String,
    store: Arc<Store>,
    health: StoreHealth,
    warnings: Mutex<Vec<String>>,
    env: Arc<Mutex<Option<BaseEnvironment>>>,
    file_runs: Arc<FileRuns>,
    shell: String,
    capture: CaptureOptions,
    tree: Arc<dyn ProcessTree + Send + Sync>,
    uid: u32,
    clocks: Arc<dyn Clocks + Send + Sync>,
    log: Arc<DaemonLog>,
    phases: Phases,
    launchctl: Option<Arc<dyn Launchctl>>,
    handshake_wait: Duration,
    /// Added to the environment of the git commands that open a board ([`DaemonOptions::git_env`]).
    git_env: Vec<(String, String)>,
    started_wall_ms: u64,
    started_uptime: Reading,
    login: Arc<dyn LoginSessions>,
    /// How the daemon before this one ended ([`login::End`]).
    previous_end: login::End,
    stop: AtomicBool,
    connections: AtomicUsize,
    /// The numbers connections are told apart by in the hub.
    next_connection: AtomicU64,
    /// Who has subscribed to the event stream, and the notices that reach them.
    hub: Arc<Hub>,
    /// The version of `claude` and the published one; the thread that refreshes the second.
    claude: Arc<ClaudeVersions>,
    claude_refresher: Mutex<Option<std::thread::JoinHandle<()>>>,
    /// What `config.get` reads: the user's files and the loader.
    config: ConfigService,
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
    sampler: Option<JoinHandle<()>>,
}

impl Daemon {
    /// Runs the start sequence of the module doc and returns once the socket is bound and accepting.
    pub fn start(options: DaemonOptions) -> Result<Daemon, SocketError> {
        let DaemonOptions {
            paths,
            shell,
            capture,
            app_executable,
            process_tree,
            clocks,
            log,
            handshake_wait,
            min_open_files,
            shim_exe,
            daemon_start,
            phases: phase_options,
            on_phase_end,
            processes,
            cleanup,
            launchctl,
            home,
            workspaces_dir,
            free_space,
            floor,
            git_env,
            login,
            sleep,
            limit_poll,
            limit_min_pause,
            sampler_tick,
            claude_published,
            claude_refresh_every,
            subscriber_queue,
            feed_rebuilder,
            load_context,
            keychain,
        } = options;
        let log = Arc::new(DaemonLog {
            sink: Mutex::new(log),
            failures: AtomicU64::new(0),
            clocks: clocks.clone(),
        });
        log.line(&format!(
            "daemon {} starting, pid {}",
            env!("CARGO_PKG_VERSION"),
            std::process::id()
        ));
        let mut warnings = Vec::new();

        // The login session (step 2): how the last daemon ended, and this one's record.
        let session_file = paths.runs_dir.with_file_name(login::FILE);
        let previous_end = match login::begin(
            &session_file,
            login.as_ref(),
            clocks.read(Clock::Wall).nanos / 1_000_000,
        ) {
            Ok(end) => {
                log.line(end.describe());
                end
            }
            Err(error) => {
                log.line(&format!("the login session could not be recorded: {error}"));
                login::End::First
            }
        };

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
        let store = Arc::new(store);
        let token = load_or_make_token(&paths.token_file)?;
        let queue = Arc::new(Queue::load(store.clone(), {
            let log = log.clone();
            Arc::new(move |text: &str| log.line(text))
        }));
        // The event stream's hub: the Queue tells it of each change, and the events' door tells it of each event.
        let hub = Arc::new(Hub::new(subscriber_queue, {
            let log = log.clone();
            Arc::new(move |text: &str| log.line(text))
        }));
        queue.on_change({
            let hub = hub.clone();
            Arc::new(move |changed| hub.publish_queue(changed))
        });

        // The metrics every run is recorded in, whatever the observers are set to (ts-0067-kw).
        let metrics = Arc::new(Metrics::new(store.clone(), clocks.clone(), {
            let log = log.clone();
            Arc::new(move |text: &str| log.line(text))
        }));

        // File-task runs (ts-0065-kx). They wait for the first capture of the base environment, which is in `env`, and
        // they end their run and release their workspace in the phase-end hook, ahead of the hook the options carry.
        let env: Arc<Mutex<Option<BaseEnvironment>>> = Arc::new(Mutex::new(None));
        let claude = ClaudeVersions::new(VersionsOptions {
            refresh_every: claude_refresh_every,
            home: home.clone(),
            log: {
                let log = log.clone();
                Arc::new(move |text: &str| log.line(text))
            },
        });
        let file_runs = Arc::new(FileRuns::new(FileRunConfig {
            runs_dir: paths.runs_dir.clone(),
            workspaces_dir: workspaces_dir
                .unwrap_or_else(|| paths.runs_dir.with_file_name("workspaces")),
            home: home.clone(),
            clocks: clocks.clone(),
            free_space: free_space.clone(),
            floor,
            git_env: git_env.clone(),
            base_env: {
                let env = env.clone();
                Arc::new(move |wait| wait_for_environment(&env, wait))
            },
            capture_wait: capture.timeout + CAPTURE_WAIT_MARGIN,
            log: {
                let log = log.clone();
                Arc::new(move |text: &str| log.line(text))
            },
            store: store.clone(),
            queue: queue.clone(),
            sleep,
            limit_poll,
            limit_min_pause,
            metrics: metrics.clone(),
            claude: claude.clone(),
        }));
        let on_phase_pause: Option<PhasePauseHook> = {
            let runs = file_runs.clone();
            Some(Box::new(move |phases, paused| {
                runs.on_phase_pause(phases, paused);
            }))
        };
        let on_phase_end: Option<PhaseEndHook> = {
            let runs = file_runs.clone();
            Some(Box::new(move |phases, end| {
                runs.on_phase_end(phases, end);
                if let Some(hook) = &on_phase_end {
                    hook(phases, end);
                }
            }))
        };
        // Adoption comes before the bind: a client that connects sees every phase in the state it is in.
        let phases = Phases::new(PhasesConfig {
            runs_dir: paths.runs_dir.clone(),
            shim_exe,
            daemon_start,
            options: phase_options,
            on_phase_end,
            store: store.clone(),
            clocks: clocks.clone(),
            log: {
                let log = log.clone();
                Arc::new(move |text: &str| log.line(text))
            },
            processes,
            cleanup,
            disk: Some((free_space.clone(), floor)),
            queue,
            on_phase_pause,
            rebuild_feed: {
                let runs = file_runs.clone();
                Some(Box::new(move |key, dir| {
                    feed_rebuilder
                        .as_ref()
                        .and_then(|rebuild| rebuild(key, dir))
                        .or_else(|| runs.rebuild_feed(key, dir))
                }))
            },
            metrics: metrics.clone(),
            on_event: {
                let hub = hub.clone();
                Some(Arc::new(move |event: &Event| hub.publish_event(event)))
            },
        })
        .map_err(SocketError::Store)?;
        match adopt_shims(&paths.runs_dir) {
            Ok(found) => {
                log.line(&format!("adopting {} shim directories", found.len()));
                phases.adopt(found);
            }
            Err(error) => {
                let text = format!("the runs could not be scanned for shims: {error}");
                log.line(&text);
                warnings.push(text);
            }
        }
        // A limit that reset while no daemon ran is resumed by this one, with no client connected.
        file_runs.recover(&phases);
        let listener =
            bind_socket(&paths).inspect_err(|e| log.line(&format!("cannot start: {e}")))?;
        log.line(&format!("listening on {}", paths.socket.display()));

        // `config.get` reads the repo's layer through the loader. The cache is beside the runs, like the workspaces.
        let config = ConfigService::new(
            home,
            load_context.unwrap_or_else(|| {
                system_load_context(
                    paths.runs_dir.with_file_name("cache"),
                    Inputs::from_process(),
                )
            }),
            git_env.clone(),
            keychain,
        );
        let uid = auth::daemon_uid();
        let tree =
            process_tree.unwrap_or_else(|| Arc::new(system_process_tree(&paths, app_executable)));
        let shared = Arc::new(Shared {
            paths,
            token,
            store,
            health,
            warnings: Mutex::new(warnings),
            env,
            file_runs,
            shell,
            capture,
            tree,
            uid,
            started_wall_ms: clocks.read(Clock::Wall).nanos / 1_000_000,
            started_uptime: clocks.read(Clock::Uptime),
            clocks,
            log,
            phases,
            launchctl,
            handshake_wait,
            git_env,
            login,
            previous_end,
            stop: AtomicBool::new(false),
            connections: AtomicUsize::new(0),
            next_connection: AtomicU64::new(1),
            hub,
            claude,
            claude_refresher: Mutex::new(None),
            config,
        });

        let capturing = shared.clone();
        spawn_named("collide-env-capture", move || {
            let captured = capturing.capture_environment();
            // The version of `claude` is read at start, and the published one is fetched, once there is an
            // environment to find the binary and the proxy in.
            capturing.file_runs.read_claude_version(Duration::ZERO);
            if let Some(source) = claude_published.source(captured.vars) {
                let refresher = capturing.claude.start_refreshing(source);
                *capturing
                    .claude_refresher
                    .lock()
                    .unwrap_or_else(|e| e.into_inner()) = Some(refresher);
            }
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
        let sampling = shared.clone();
        let sampler = spawn_named("collide-sampler", move || {
            sampler_loop(&sampling, &metrics, sampler_tick);
        })
        .map_err(|source| SocketError::Io {
            context: "could not start the sampler's thread".to_string(),
            source,
        })?;
        Ok(Daemon {
            shared,
            accept: Some(accept),
            sampler: Some(sampler),
        })
    }

    /// How the daemon before this one ended: a stop, host shutdown at logout, or a crash.
    pub fn previous_end(&self) -> login::End {
        self.shared.previous_end
    }

    pub fn paths(&self) -> &DaemonPaths {
        &self.shared.paths
    }

    /// The daemon's phases: start one, write to one, ask its state. Cloning it shares them.
    pub fn phases(&self) -> Phases {
        self.shared.phases.clone()
    }

    /// The daemon's store, for a reader of the events and of the metrics tables.
    pub fn store(&self) -> Arc<Store> {
        self.shared.store.clone()
    }

    /// The Queue items the daemon holds, those of `run` when it is given.
    pub fn queue_items(&self, run: Option<&str>) -> Vec<QueueItem> {
        self.shared.file_runs.queue_items(run)
    }

    /// The events of the store after `seq`, oldest first.
    pub fn events_since(&self, seq: u64) -> Result<Vec<Event>, StoreError> {
        self.shared.store.events_since(seq)
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
        // The sampler ends within a slice of the stop flag; nothing else ends it.
        if let Some(sampler) = self.sampler.take() {
            self.shared.stop.store(true, Ordering::SeqCst);
            let _ = sampler.join();
        }
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        self.join();
        // The limit timers end first: one that is resuming a run needs the phases.
        self.shared.file_runs.shutdown();
        self.shared.claude.stop();
        let refresher = self
            .shared
            .claude_refresher
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        if let Some(refresher) = refresher {
            let _ = refresher.join();
        }
        // The shims go on: they belong to no daemon. Only the daemon's watchers of them end.
        self.shared.phases.stop();
        // Whether the daemon ends because the login session does is the last thing it records.
        match login::finish(
            &self.shared.paths.runs_dir.with_file_name(login::FILE),
            self.shared.login.as_ref(),
        ) {
            Ok(end) => self.shared.log.line(end.describe()),
            Err(error) => self
                .shared
                .log
                .line(&format!("the login session was not marked ended: {error}")),
        }
        // Last, because booting out the job ends this process: a stopped daemon leaves no loaded job behind.
        if let Some(launchctl) = &self.shared.launchctl {
            match boot_out_recorded_job(&self.shared.paths.lock_file, launchctl.as_ref()) {
                Ok(Some(label)) => self
                    .shared
                    .log
                    .line(&format!("booted out the launchd job {label}")),
                Ok(None) => {}
                Err(error) => self
                    .shared
                    .log
                    .line(&format!("the launchd job could not be booted out: {error}")),
            }
        }
    }
}

/// The longest the sampler sleeps before it looks at the stop flag again.
const SAMPLER_SLICE: Duration = Duration::from_millis(50);

/// One tick every `tick`: reads the clocks and records a sleep the sampler finds ([`Sampler::tick`]). It sleeps in
/// slices so a stop is seen within one.
fn sampler_loop(shared: &Shared, metrics: &Metrics, tick: Duration) {
    let mut sampler = Sampler::new(ClockPair::read(shared.clocks.as_ref()));
    let slice = SAMPLER_SLICE.min(tick);
    loop {
        let mut waited = Duration::ZERO;
        while waited < tick {
            if shared.stopping() {
                return;
            }
            std::thread::sleep(slice);
            waited += slice;
        }
        let ticked = sampler.tick(shared.clocks.as_ref());
        if let Some(sleep) = ticked.sleep {
            metrics.sleep_interval(sleep.length, &ticked.now);
        }
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
    let admitted = match auth::admit(
        shared.uid,
        &UnixPeer { stream: &stream },
        shared.tree.as_ref(),
    ) {
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
        session.caller()
    ));
    let conn = Arc::new(Conn {
        id: shared.next_connection.fetch_add(1, Ordering::SeqCst),
        writer: Mutex::new(stream),
    });
    converse(shared, &mut lines, &conn, &session);
    shared.hub.remove(conn.id);
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

/// A connection after its handshake. Everything written to the socket goes through `writer`, so an answer and the
/// notices of a subscription never mix inside a line.
struct Conn {
    id: u64,
    writer: Mutex<UnixStream>,
}

impl Conn {
    fn stream(&self) -> MutexGuard<'_, UnixStream> {
        self.writer.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn send(&self, response: &Response) -> io::Result<()> {
        send(&mut self.stream(), response)
    }

    /// Writes a notice of a subscription, unless the subscription was cancelled. The flag is read with the lock held:
    /// a subscription that is replaced writes nothing after the answer of the one that replaced it. A failed write
    /// has left half a line, so the connection is closed.
    fn send_notice(&self, cancelled: &AtomicBool, notice: &Notice) -> bool {
        let mut stream = self.stream();
        if cancelled.load(Ordering::SeqCst) {
            return false;
        }
        let line = encode_line(&notice.to_notification());
        if stream.write_all(line.as_bytes()).is_err() {
            let _ = stream.shutdown(std::net::Shutdown::Both);
            return false;
        }
        true
    }

    /// Ends what the daemon writes to the connection, after the notices already written: the client reads those,
    /// then the end of the stream. What the client writes is still read, until it closes.
    fn close(&self, cancelled: &AtomicBool) {
        let stream = self.stream();
        if !cancelled.load(Ordering::SeqCst) {
            let _ = stream.shutdown(std::net::Shutdown::Write);
        }
    }
}

/// What a subscription sends after its answer: the events the client asked for, then the live ones. A live event whose
/// `seq` is at or below `floor` was in the replay or came before the answer, and is not sent again.
struct Pump {
    subscription: Subscription,
    replay: Vec<Event>,
    floor: u64,
}

/// The requests after `initialize`, until the client closes or the daemon stops.
fn converse(shared: &Shared, lines: &mut LineReader, conn: &Arc<Conn>, session: &Session) {
    loop {
        let line = match lines.next(|| shared.stopping(), None) {
            Ok(Line::Text(line)) => line,
            Ok(Line::NotText) => {
                if conn.send(&not_text()).is_err() {
                    return;
                }
                continue;
            }
            Ok(Line::TooLong) => {
                let _ = conn.send(&too_long());
                drain_then_close(&mut conn.stream());
                return;
            }
            Ok(Line::Closed | Line::Stopping | Line::TimedOut) | Err(_) => return,
        };
        let (response, pump) = match parse_request(&line) {
            Err(response) => (response, None),
            Ok(request) => answer(shared, conn, session, request),
        };
        if conn.send(&response).is_err() {
            return;
        }
        // The answer is written: the replay and the live events come after it.
        if let Some(pump) = pump {
            start_pump(shared, conn, pump);
        }
    }
}

fn answer(
    shared: &Shared,
    conn: &Conn,
    session: &Session,
    request: Request,
) -> (Response, Option<Pump>) {
    let id = request.id.clone();
    let mut pump = None;
    let result = match route(true, &request) {
        Err(error) => Err(error),
        Ok(Routed::Initialize(_)) => unreachable!("route(true, ..) refuses a second initialize"),
        Ok(Routed::Call(Method::DaemonSubscribe)) => subscribe(shared, conn.id, &request.params)
            .map(|(value, started)| {
                pump = Some(started);
                value
            }),
        Ok(Routed::Call(method)) => call(shared, session, method, &request.params),
    };
    let response = match result {
        Ok(value) => Response::success(id, value),
        Err(error) => Response::failure(Some(id), error),
    };
    (response, pump)
}

/// `daemon.subscribe` (ts-0076-cw): registers for the live events first, then reads the head and the events after
/// `since`, so an event is in the replay, in the queue, or in both, and never in neither. The queue's copy of an event
/// the replay has is dropped by its `seq` ([`Pump::floor`]).
fn subscribe(
    shared: &Shared,
    connection: u64,
    params: &serde_json::Value,
) -> Result<(serde_json::Value, Pump), RpcError> {
    let params: DaemonSubscribeParams = read_params("daemon.subscribe", params)?;
    let subscription = shared.hub.register(connection);
    let read = || -> Result<(u64, u64, Vec<Event>), StoreError> {
        let head = shared.store.last_seq()?;
        let oldest = shared.store.oldest_seq()?;
        let replay = match params.since {
            Some(since) => shared
                .store
                .events_since(since)?
                .into_iter()
                .filter(|event| event.seq <= head)
                .collect(),
            None => Vec::new(),
        };
        Ok((head, oldest, replay))
    };
    let (head, oldest, replay) = read().map_err(|error| {
        shared.hub.remove(connection);
        store_failed(error)
    })?;
    let result = DaemonSubscribeResult {
        head_seq: head,
        oldest_seq: oldest,
        replayed: replay.len() as u64,
    };
    let value = serde_json::to_value(result).expect("a result is JSON");
    Ok((
        value,
        Pump {
            subscription,
            replay,
            floor: head,
        },
    ))
}

/// Starts the thread that writes a subscription's notices to its connection.
fn start_pump(shared: &Shared, conn: &Arc<Conn>, pump: Pump) {
    let log = shared.log.clone();
    let writing = conn.clone();
    let started = spawn_named("collide-subscription", move || {
        run_pump(&log, &writing, pump);
    });
    if let Err(e) = started {
        shared
            .log
            .line(&format!("could not start a subscription's thread: {e}"));
        shared.hub.remove(conn.id);
    }
}

/// Writes the replay, then each notice of the queue, until the subscription is cancelled, the client is gone, or the
/// queue was full: then the socket is closed, and the client subscribes again from the last `seq` it has.
fn run_pump(log: &DaemonLog, conn: &Conn, pump: Pump) {
    let Pump {
        subscription,
        replay,
        floor,
    } = pump;
    let cancelled = &subscription.cancelled;
    for event in replay {
        if !conn.send_notice(cancelled, &Notice::DaemonEvent(Box::new(event))) {
            return;
        }
    }
    while let Ok(notice) = subscription.notices.recv() {
        if matches!(&notice, Notice::DaemonEvent(event) if event.seq <= floor) {
            continue;
        }
        if !conn.send_notice(cancelled, &notice) {
            return;
        }
    }
    if subscription.overflowed.load(Ordering::SeqCst) {
        log.line(&format!(
            "connection {} was closed: it did not read its events fast enough",
            conn.id
        ));
        conn.close(cancelled);
    }
}

fn call(
    shared: &Shared,
    session: &Session,
    method: Method,
    params: &serde_json::Value,
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
                live_runs: u32::try_from(shared.phases.live()).unwrap_or(u32::MAX),
                store: shared.health.clone(),
                warnings: {
                    let mut warnings = shared
                        .warnings
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .clone();
                    warnings.extend(
                        shared
                            .phases
                            .unresponsive()
                            .into_iter()
                            .map(|name| format!("the shim of phase {name} is not answering")),
                    );
                    warnings
                },
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
        Method::DaemonAgentVersions => {
            // What is known now; a daemon whose capture has ended and that has read nothing yet reads the binary.
            shared
                .file_runs
                .read_claude_version(shared.capture.timeout + CAPTURE_WAIT_MARGIN);
            Ok(serde_json::to_value(shared.claude.status()).expect("a result is JSON"))
        }
        Method::RunStop => run_stop(shared, session, params),
        Method::RunStartFile => run_start_file(shared, session, params),
        Method::RunStart => run_start(shared, session, params),
        Method::RunDiff => run_diff(shared, params),
        Method::RunRecord => run_record(shared, params),
        Method::RunList => run_list(shared, params),
        Method::RunGet => run_get(shared, params),
        Method::QueueList => queue_list(shared, params),
        Method::QueueAnswer => queue_answer(shared, session, params),
        Method::ConfigGet => config_get(shared, params),
        Method::ConfigSet => config_set(shared, params),
        Method::BoardOpen => board_open(shared, params),
        Method::BoardList => board_list(shared),
        Method::TaskGet => task_get(shared, params),
        Method::TaskApply => task_apply(shared, session, params),
        other => Err(RpcError::new(
            code::METHOD_NOT_FOUND,
            format!(
                "method not found: `{}` is not served by this daemon yet",
                other.name()
            ),
        )),
    }
}

/// How long past the capture's own timeout a run waits for the first capture: the capture always ends, in the login
/// shell's environment or the minimal one, within its timeout.
const CAPTURE_WAIT_MARGIN: Duration = Duration::from_secs(5);

/// The base environment once the first capture has finished, waiting for it for up to `wait`.
fn wait_for_environment(
    env: &Mutex<Option<BaseEnvironment>>,
    wait: Duration,
) -> Option<BaseEnvironment> {
    let until = std::time::Instant::now() + wait;
    loop {
        if let Some(captured) = env.lock().unwrap_or_else(|e| e.into_inner()).clone() {
            return Some(captured);
        }
        if std::time::Instant::now() >= until {
            return None;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// `run.start_file` (`collide run`, for one task file): makes the worktree, starts the builder phase and answers with
/// the run's id. A run paused for disk is started and answers the same; its state and its Queue item say it is paused.
/// It may take as long as the first capture of the base environment and the worktree take.
fn run_start_file(
    shared: &Shared,
    session: &Session,
    params: &serde_json::Value,
) -> Result<serde_json::Value, RpcError> {
    let params: RunStartFileParams = serde_json::from_value(params.clone()).map_err(|e| {
        RpcError::new(
            code::INVALID_PARAMS,
            format!("the params of run.start_file are not readable: {e}"),
        )
    })?;
    if let Caller::RunAgent { run } = session.caller() {
        return Err(RpcError::new(
            code::NOT_PERMITTED,
            format!("the agent of run {run} may not start a run"),
        ));
    }
    if shared.stopping() {
        return Err(RpcError::new(
            code::UNAVAILABLE,
            "the daemon is shutting down",
        ));
    }
    let result = match shared.file_runs.start(&shared.phases, &params) {
        Ok(run) => RunStartResult::Started { run },
        Err(problems) => RunStartResult::Refused { problems },
    };
    Ok(serde_json::to_value(result).expect("a result is JSON"))
}

/// `run.start` for a prompt (ts-0078-ye): one scratch task of a board, run through `collide/quick` in a worktree up to
/// its review. A board the daemon does not know is `NOT_FOUND`; everything the run is refused for is in the answer, in
/// Collide's words. It may take as long as the first capture of the base environment and the worktree take.
fn run_start(
    shared: &Shared,
    session: &Session,
    params: &serde_json::Value,
) -> Result<serde_json::Value, RpcError> {
    let params: RunStartParams = read_params("run.start", params)?;
    if let Caller::RunAgent { run } = session.caller() {
        return Err(RpcError::new(
            code::NOT_PERMITTED,
            format!("the agent of run {run} may not start a run"),
        ));
    }
    if shared.stopping() {
        return Err(RpcError::new(
            code::UNAVAILABLE,
            "the daemon is shutting down",
        ));
    }
    let root = board_root(shared, &params.board)?;
    let result = match shared.file_runs.start_quick(&shared.phases, &params, &root) {
        Ok(run) => RunStartResult::Started { run },
        Err(problems) => RunStartResult::Refused { problems },
    };
    Ok(serde_json::to_value(result).expect("a result is JSON"))
}

/// `run.diff` (ts-0078-ye): the files a prompt run's branch changed since the commit it started from, and with `path`
/// one file's patch. A run that is not a prompt run, a branch that is gone and a file with no change are `NOT_FOUND`.
fn run_diff(shared: &Shared, params: &serde_json::Value) -> Result<serde_json::Value, RpcError> {
    let params: RunDiffParams = read_params("run.diff", params)?;
    let diff = shared
        .file_runs
        .diff(&params.run, params.path.as_deref())
        .map_err(|error| match error {
            RunDiffError::NotFound(message) => RpcError::new(code::NOT_FOUND, message),
            RunDiffError::Failed(message) => RpcError::new(code::INTERNAL_ERROR, message),
        })?;
    Ok(serde_json::to_value(diff).expect("a result is JSON"))
}

/// `run.record` (`collide run`, ts-0069-bn): the run's state, its report and failure cause, and one line for each model
/// call, read from the store. A run the daemon has never heard of is `NOT_FOUND`.
fn run_record(shared: &Shared, params: &serde_json::Value) -> Result<serde_json::Value, RpcError> {
    let params: RunRecordParams = serde_json::from_value(params.clone()).map_err(|e| {
        RpcError::new(
            code::INVALID_PARAMS,
            format!("the params of run.record are not readable: {e}"),
        )
    })?;
    let run = params.run;
    let state = shared
        .phases
        .settled_run_state(&run)
        .ok_or_else(|| RpcError::new(code::NOT_FOUND, format!("the daemon has no run {run:?}")))?;
    let unreadable = |what: &str, error: crate::store::StoreError| {
        RpcError::new(
            code::INTERNAL_ERROR,
            format!("the {what} of run {run} could not be read: {error}"),
        )
    };
    let now_ns = shared.clocks.read(Clock::Continuous).nanos;
    let spans = shared
        .store
        .spans(&run)
        .map_err(|e| unreadable("spans", e))?;
    let rows = shared
        .store
        .usage_rows(&run, None)
        .map_err(|e| unreadable("usage", e))?;
    let record = shared
        .store
        .phase_record(&run, crate::file_run::PHASE_NAME)
        .map_err(|e| unreadable("phase record", e))?
        .unwrap_or_default();
    let result = RunRecordResult {
        run: run.clone(),
        state,
        failure: record.failure,
        failure_evidence: record.failure_evidence,
        report: record.report,
        workspace: shared
            .file_runs
            .workspace_path(&run)
            .map(|path| path.to_string_lossy().into_owned()),
        wall_ms: crate::run_record::run_wall_ms(&spans, now_ns),
        calls: crate::run_record::recorded_calls(&spans, &rows, now_ns),
    };
    Ok(serde_json::to_value(result).expect("a result is JSON"))
}

/// How many runs `run.list` gives when the request names no limit, and the most it gives.
const RUN_PAGE: u32 = 50;
const RUN_PAGE_MAX: u32 = 200;

/// The run as `run.list` and `run.get` give it: its row, its phases from the daemon, the times of their spans from the
/// store, and its workspace folder when it has one.
fn summary_of(
    shared: &Shared,
    row: &RunRow,
) -> Result<(RunSummary, BTreeMap<String, u32>), RpcError> {
    let spans = shared.store.phase_spans(&row.run).map_err(store_failed)?;
    let workspace = shared
        .file_runs
        .workspace_path(&row.run)
        .map(|path| path.to_string_lossy().into_owned());
    Ok(summarize(
        row,
        &shared.phases.run_phases(&row.run),
        &spans,
        workspace,
    ))
}

/// `run.list` (ts-0076-cw): the runs of the store, newest first, by board, task and scope, a page at a time. A run
/// that has no board (one `run.start_file` made) is in a list that names no board. A page's cursor is the number of its
/// last run's first event, and the next page is the runs opened before it. A board that was not opened is `NOT_FOUND`,
/// and a cursor that is not a number is `INVALID_PARAMS`.
fn run_list(shared: &Shared, params: &serde_json::Value) -> Result<serde_json::Value, RpcError> {
    let params: RunListParams = read_params("run.list", params)?;
    if let Some(board) = &params.board {
        board_root(shared, board)?;
    }
    let before = params
        .cursor
        .as_deref()
        .map(|cursor| {
            cursor.parse::<u64>().map_err(|_| {
                RpcError::new(
                    code::INVALID_PARAMS,
                    format!("{cursor:?} is not a cursor run.list gave"),
                )
            })
        })
        .transpose()?;
    let limit = params.limit.unwrap_or(RUN_PAGE).clamp(1, RUN_PAGE_MAX);
    // One more than the page, to know whether there is a next one.
    let mut rows = shared
        .store
        .runs(&RunFilter {
            board: params.board.as_deref(),
            task: params.task.as_deref(),
            scope: params.scope,
            before,
            limit: limit + 1,
        })
        .map_err(store_failed)?;
    let more = rows.len() > limit as usize;
    rows.truncate(limit as usize);
    let next_cursor = more
        .then(|| rows.last().map(|row| row.opened_seq.to_string()))
        .flatten();
    let runs = rows
        .iter()
        .map(|row| summary_of(shared, row).map(|(summary, _)| summary))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(serde_json::to_value(RunListResult { runs, next_cursor }).expect("a result is JSON"))
}

/// `run.get`: one run's summary and the phases' rounds. What `run.start` pins at launch (the snapshot, the fingerprint)
/// is not recorded yet, so those are empty; a prompt run's workspace is the built-in one, and that is all its
/// `implementations` say so far. A run the store has no row of is `NOT_FOUND`.
fn run_get(shared: &Shared, params: &serde_json::Value) -> Result<serde_json::Value, RpcError> {
    let params: RunGetParams = read_params("run.get", params)?;
    let row = shared
        .store
        .run(&params.run)
        .map_err(store_failed)?
        .ok_or_else(|| {
            RpcError::new(
                code::NOT_FOUND,
                format!("the daemon has no run {:?}", params.run),
            )
        })?;
    let (summary, rounds) = summary_of(shared, &row)?;
    let result = RunGetResult {
        summary,
        snapshot_commit: String::new(),
        launch: BTreeMap::new(),
        implementations: if shared.file_runs.is_prompt_run(&params.run) {
            BTreeMap::from([("workspace".to_string(), "builtin".to_string())])
        } else {
            BTreeMap::new()
        },
        rounds,
        fingerprint: BTreeMap::new(),
        annotations: Vec::new(),
    };
    Ok(serde_json::to_value(result).expect("a result is JSON"))
}

/// `queue.list`: the Queue items the daemon holds, so far those of a paused file-task run, by run and kind.
fn queue_list(shared: &Shared, params: &serde_json::Value) -> Result<serde_json::Value, RpcError> {
    let params: QueueListParams = serde_json::from_value(params.clone()).map_err(|e| {
        RpcError::new(
            code::INVALID_PARAMS,
            format!("the params of queue.list are not readable: {e}"),
        )
    })?;
    let items = shared
        .file_runs
        .queue_items(params.run.as_deref())
        .into_iter()
        .filter(|item| params.kinds.is_empty() || params.kinds.contains(&item.kind))
        .collect();
    Ok(serde_json::to_value(QueueListResult { items }).expect("a result is JSON"))
}

/// `queue.answer`: answers one Queue item (a permission request, a usage limit's Resume, a Stop, an accept). Only a
/// person does it: a run's agent that could answer its own permission request would make the request pointless.
fn queue_answer(
    shared: &Shared,
    session: &Session,
    params: &serde_json::Value,
) -> Result<serde_json::Value, RpcError> {
    let params: QueueAnswerParams = serde_json::from_value(params.clone()).map_err(|e| {
        RpcError::new(
            code::INVALID_PARAMS,
            format!("the params of queue.answer are not readable: {e}"),
        )
    })?;
    if let Caller::RunAgent { run } = session.caller() {
        return Err(RpcError::new(
            code::NOT_PERMITTED,
            format!("the agent of run {run} may not answer the Queue"),
        ));
    }
    if shared.stopping() {
        return Err(RpcError::new(
            code::UNAVAILABLE,
            "the daemon is shutting down",
        ));
    }
    match shared
        .file_runs
        .answer(&shared.phases, &params.item, &params.answer)
    {
        Ok(result) => Ok(serde_json::to_value(result).expect("a result is JSON")),
        Err(AnswerError::NotFound(message)) => Err(RpcError::new(code::NOT_FOUND, message)),
        Err(AnswerError::Invalid(message)) => Err(RpcError::new(code::INVALID_PARAMS, message)),
    }
}

/// Reads a request's params as `T`, or says which method they were not readable for.
fn read_params<T: serde::de::DeserializeOwned>(
    method: &str,
    params: &serde_json::Value,
) -> Result<T, RpcError> {
    serde_json::from_value(params.clone()).map_err(|e| {
        RpcError::new(
            code::INVALID_PARAMS,
            format!("the params of {method} are not readable: {e}"),
        )
    })
}

/// A store that failed, as the error of a request.
fn store_failed(error: StoreError) -> RpcError {
    RpcError::new(code::INTERNAL_ERROR, error.to_string())
}

/// The root of the board a request names, or `NOT_FOUND` naming the board.
fn board_root(shared: &Shared, board: &str) -> Result<String, RpcError> {
    shared
        .store
        .board_root(board)
        .map_err(store_failed)?
        .ok_or_else(|| {
            RpcError::new(
                code::NOT_FOUND,
                format!("the daemon has no board {board:?}; open its folder with board.open first"),
            )
        })
}

/// `config.get` (ts-0098-nx): the settings of a board (the user's layers alone with no board), each with the layer it
/// came from. A problem in a file or a pack is a `CONFLICT` whose message has one line for each, and a key, a profile
/// or a board that is not there is `NOT_FOUND` ([`crate::config_get`]).
fn config_get(shared: &Shared, params: &serde_json::Value) -> Result<serde_json::Value, RpcError> {
    let params: ConfigGetParams = read_params("config.get", params)?;
    if shared.stopping() {
        return Err(RpcError::new(
            code::UNAVAILABLE,
            "the daemon is shutting down",
        ));
    }
    let root = board_root_of(shared, params.board.as_deref())?;
    let board = params
        .board
        .as_deref()
        .zip(root.as_deref())
        .map(|(id, root)| BoardRef { id, root });
    match shared.config.get(&params, board, &tools_path(shared)) {
        Ok(result) => Ok(serde_json::to_value(result).expect("a result is JSON")),
        Err(GetError::NotFound(message)) => Err(RpcError::new(code::NOT_FOUND, message)),
        Err(GetError::Refused(message)) => Err(RpcError::new(code::CONFLICT, message)),
    }
}

/// `config.set` (ts-0099-bw): a secret to the keychain, or a plain value to the user's file ([`crate::config_get`]).
/// The request's `value` may be a secret, so nothing here reads it into a message: the params that cannot be read are
/// answered without the serde error (which can quote a value), no line is logged for the call, and the answer holds no
/// value.
fn config_set(shared: &Shared, params: &serde_json::Value) -> Result<serde_json::Value, RpcError> {
    let params: ConfigSetParams = serde_json::from_value(params.clone()).map_err(|_| {
        RpcError::new(
            code::INVALID_PARAMS,
            "the params of config.set are not readable: they need `key`, `value` and `layer`, and may have `board` \
             and `secret`",
        )
    })?;
    if shared.stopping() {
        return Err(RpcError::new(
            code::UNAVAILABLE,
            "the daemon is shutting down",
        ));
    }
    let root = board_root_of(shared, params.board.as_deref())?;
    let board = params
        .board
        .as_deref()
        .zip(root.as_deref())
        .map(|(id, root)| BoardRef { id, root });
    match shared.config.set(&params, board, &tools_path(shared)) {
        Ok(result) => Ok(serde_json::to_value(result).expect("a result is JSON")),
        Err(SetError::NotFound(message)) => Err(RpcError::new(code::NOT_FOUND, message)),
        Err(SetError::Invalid(message)) => Err(RpcError::new(code::INVALID_PARAMS, message)),
        Err(SetError::Refused(message)) => Err(RpcError::new(code::CONFLICT, message)),
        Err(SetError::Failed(message)) => Err(RpcError::new(code::INTERNAL_ERROR, message)),
    }
}

/// The root of the board a request names, or `None` for a request with no board.
fn board_root_of(
    shared: &Shared,
    board: Option<&str>,
) -> Result<Option<std::path::PathBuf>, RpcError> {
    board
        .map(|id| board_root(shared, id).map(std::path::PathBuf::from))
        .transpose()
}

/// The `PATH` the tools a pack needs are looked for on: where the user's login shell finds them, once it has been
/// captured.
fn tools_path(shared: &Shared) -> std::ffi::OsString {
    shared
        .env
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .and_then(|env| env.vars.get("PATH").cloned())
        .map(std::ffi::OsString::from)
        .or_else(|| std::env::var_os("PATH"))
        .unwrap_or_default()
}

/// `board.open` (ts-0075-hj): any folder in a git checkout becomes the board of that checkout's top level, and the same
/// board each time. A folder that is not in a checkout answers no board and a failed `git_repo` check. `git init` is
/// Stage 6a's, so `init_git` is refused.
fn board_open(shared: &Shared, params: &serde_json::Value) -> Result<serde_json::Value, RpcError> {
    let params: BoardOpenParams = read_params("board.open", params)?;
    if params.init_git {
        return Err(RpcError::new(
            code::INVALID_PARAMS,
            "`init_git` is not built yet (Stage 6a). Run `git init` in the folder, then open it again.",
        ));
    }
    if shared.stopping() {
        return Err(RpcError::new(
            code::UNAVAILABLE,
            "the daemon is shutting down",
        ));
    }
    let folder = Path::new(&params.root);
    if !folder.is_absolute() {
        return Err(RpcError::new(
            code::INVALID_PARAMS,
            format!(
                "`root` must be an absolute path, and `{}` is not",
                params.root
            ),
        ));
    }
    let result = match boards::checkout_of(folder, &shared.git_env) {
        Err(message) => BoardOpenResult {
            board: None,
            checks: vec![boards::git_repo_failed(message)],
        },
        Ok(checkout) => {
            let root = checkout.root.display().to_string();
            let id = shared.store.open_board(&root).map_err(store_failed)?;
            BoardOpenResult {
                board: Some(boards::summary(&id, &root, Some(&checkout))),
                checks: vec![boards::git_repo_ok(&checkout)],
            }
        }
    };
    Ok(serde_json::to_value(result).expect("a result is JSON"))
}

/// `board.list`: the boards opened before, from the store. A board's line is the branch its checkout is on now, read
/// from git; a board whose folder is gone or is no longer a checkout is listed with no line.
fn board_list(shared: &Shared) -> Result<serde_json::Value, RpcError> {
    let boards = shared
        .store
        .boards()
        .map_err(store_failed)?
        .into_iter()
        .map(|board| {
            let checkout = boards::checkout_of(Path::new(&board.root), &shared.git_env).ok();
            boards::summary(&board.id, &board.root, checkout.as_ref())
        })
        .collect();
    Ok(serde_json::to_value(BoardListResult { boards }).expect("a result is JSON"))
}

/// `task.get` for a scratch task (ts-0075-hj): the task with its whole prompt, or no task when the board has none with
/// that id. A key of another source is not found: only the scratch source is built yet.
fn task_get(shared: &Shared, params: &serde_json::Value) -> Result<serde_json::Value, RpcError> {
    let params: TaskGetParams = read_params("task.get", params)?;
    board_root(shared, &params.board)?;
    let (source, id) = scratch::split_id(&params.id);
    let task = if source.is_none_or(|source| source == scratch::SOURCE) {
        ScratchSource::new(shared.store.clone(), &params.board)
            .get(&GetParams {
                id: id.to_string(),
                checkout: None,
            })
            .map_err(|e| RpcError::new(code::INTERNAL_ERROR, e.to_string()))?
            .task
    } else {
        None
    };
    let runs = match &task {
        Some(task) => shared
            .store
            .runs(&RunFilter {
                board: Some(&params.board),
                task: Some(&task.key),
                scope: RunScope::All,
                before: None,
                limit: RUN_PAGE_MAX,
            })
            .map_err(store_failed)?
            .into_iter()
            .map(|row| row.run)
            .collect(),
        None => Vec::new(),
    };
    let result = TaskGetResult {
        task,
        ambiguous: Vec::new(),
        copies: Vec::new(),
        collisions: Vec::new(),
        runs,
        latest_run: None,
    };
    Ok(serde_json::to_value(result).expect("a result is JSON"))
}

/// `task.apply` on the scratch source (ts-0075-hj): `create` and `setStatus`. The source's own refusal (an unknown
/// status, an unknown task) is an answer, not an error. Each write records a `task.changed` event with the actor of
/// the connection. A scratch task is never committed, so `commit` is refused.
fn task_apply(
    shared: &Shared,
    session: &Session,
    params: &serde_json::Value,
) -> Result<serde_json::Value, RpcError> {
    let params: TaskApplyParams = read_params("task.apply", params)?;
    if shared.stopping() {
        return Err(RpcError::new(
            code::UNAVAILABLE,
            "the daemon is shutting down",
        ));
    }
    if params.commit {
        return Err(RpcError::new(
            code::INVALID_PARAMS,
            "a scratch task is never committed: it lives in Collide's store, not in the repository",
        ));
    }
    board_root(shared, &params.board)?;
    let mut apply = params.apply;
    let (from_id, bare_id) = match apply.id.as_deref().map(scratch::split_id) {
        Some((source, bare)) => (source, Some(bare.to_string())),
        None => (None, None),
    };
    let source = match (params.source.as_deref(), from_id) {
        (Some(named), Some(in_id)) if named != in_id => {
            return Err(RpcError::new(
                code::INVALID_PARAMS,
                format!("`source` is `{named}`, and the id names the source `{in_id}`"),
            ));
        }
        (Some(source), _) | (None, Some(source)) => source,
        (None, None) => {
            return Err(RpcError::new(
                code::INVALID_PARAMS,
                "name the source: an op that makes a task has no id to name it, so give `source`",
            ));
        }
    };
    if source != scratch::SOURCE {
        return Err(RpcError::new(
            code::INVALID_PARAMS,
            format!(
                "the daemon serves only the `{}` source so far, and `{source}` is not built yet",
                scratch::SOURCE
            ),
        ));
    }
    apply.id = bare_id;
    let result = ScratchSource::new(shared.store.clone(), &params.board)
        .apply(&apply)
        .map_err(|e| RpcError::new(code::INTERNAL_ERROR, e.to_string()))?;
    if let ApplyResult::Ok { items, .. } = &result {
        for effect in items {
            if let Err(error) = shared
                .phases
                .append_event(session.actor(), scratch::changed_event(effect))
            {
                shared.log.line(&format!(
                    "the change of {} was written but its event was not: {error}",
                    effect.key
                ));
            }
        }
    }
    Ok(serde_json::to_value(TaskApplyResult {
        result,
        commit: None,
    })
    .expect("a result is JSON"))
}

/// `run.stop` (`collide stop`, 0.5): stops the run's processes in the order of [`crate::stop::ORDER`] and answers with
/// the run's state, how many processes the first scan signalled, and the survivors. A survivor's `command` carries how
/// it came to be listed (`found`, `recorded` or `suspect`). A run's agent may stop its own run and no other.
fn run_stop(
    shared: &Shared,
    session: &Session,
    params: &serde_json::Value,
) -> Result<serde_json::Value, RpcError> {
    let params: RunStopParams = serde_json::from_value(params.clone()).map_err(|e| {
        RpcError::new(
            code::INVALID_PARAMS,
            format!("the params of run.stop are not readable: {e}"),
        )
    })?;
    if let Caller::RunAgent { run } = session.caller()
        && *run != params.run
    {
        return Err(RpcError::new(
            code::NOT_PERMITTED,
            format!("run {run} may not stop run {}", params.run),
        ));
    }
    if shared.stopping() {
        return Err(RpcError::new(
            code::UNAVAILABLE,
            "the daemon is shutting down",
        ));
    }
    // A run that paused for disk before it had a phase has nothing to signal.
    if !shared.phases.has_run(&params.run)
        && shared.file_runs.stop_unstarted(&shared.phases, &params.run)
    {
        let result = RunStopResult {
            state: collide_core::state::RunState::Stopped {},
            signalled: 0,
            survivors: Vec::new(),
        };
        return Ok(serde_json::to_value(result).expect("a result is JSON"));
    }
    let grace = params
        .grace_secs
        .map(|secs| Duration::from_secs(u64::from(secs)));
    let report = shared
        .phases
        .stop_run(&params.run, grace)
        .map_err(|error| match error {
            StopRunError::Unknown(_) => RpcError::new(code::NOT_FOUND, error.to_string()),
            StopRunError::Name(_) | StopRunError::Refused(_) => {
                RpcError::new(code::INVALID_PARAMS, error.to_string())
            }
        })?;
    let state = shared.phases.run_state(&params.run).ok_or_else(|| {
        RpcError::new(
            code::NOT_FOUND,
            format!("the daemon has no run {:?}", params.run),
        )
    })?;
    let result = RunStopResult {
        state,
        signalled: u32::try_from(report.signalled.len()).unwrap_or(u32::MAX),
        survivors: report
            .survivors
            .iter()
            .map(|s| Survivor {
                pid: s.pid,
                command: format!("{} ({})", s.command, s.kind.word()),
            })
            .collect(),
    };
    for problem in &report.problems {
        shared
            .log
            .line(&format!("stop of run {}: {problem}", params.run));
    }
    Ok(serde_json::to_value(result).expect("a result is JSON"))
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

/// Finds every shim of an earlier daemon under `runs_dir`: each `<run>/<phase>` directory with a `spec.json` (what a
/// shim is started from) is one. For each, `exit.json` is read first (a record, or a file that is not one), then the
/// shim's own `shim.json` identity: the shim is alive when a process with that pid started at that time and is not a
/// zombie. A shim that left no `shim.json` is alive when its socket accepts a connection. Anything else is gone with
/// no record: `lost`. A directory that is not a phase of this daemon (a name [`PhaseKey`] refuses, or no `spec.json`)
/// is left alone. It runs before the socket is bound.
pub fn adopt_shims(runs_dir: &Path) -> Result<Vec<AdoptedShim>, SocketError> {
    let io_error = |context: String| move |source| SocketError::Io { context, source };
    let runs = match std::fs::read_dir(runs_dir) {
        Ok(runs) => runs,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io_error(format!("reading {}", runs_dir.display()))(e)),
    };
    let mut found = Vec::new();
    for run in runs {
        let run = run.map_err(io_error(format!("reading {}", runs_dir.display())))?;
        let run_dir = run.path();
        if !run_dir.is_dir() {
            continue;
        }
        let Some(run_name) = run.file_name().to_str().map(str::to_string) else {
            continue;
        };
        let phases = std::fs::read_dir(&run_dir)
            .map_err(io_error(format!("reading {}", run_dir.display())))?;
        for phase in phases {
            let phase = phase.map_err(io_error(format!("reading {}", run_dir.display())))?;
            let dir = phase.path();
            let Some(phase_name) = phase.file_name().to_str().map(str::to_string) else {
                continue;
            };
            let Ok(key) = PhaseKey::new(&run_name, &phase_name) else {
                continue;
            };
            if !dir.join(shim_files::SPEC).is_file() {
                continue;
            }
            found.push(classify_shim(key, dir));
        }
    }
    found.sort_by(|a, b| a.key.cmp(&b.key));
    Ok(found)
}

/// Whether `spec.json` was written less than `age` ago.
fn spec_is_younger_than(dir: &Path, age: Duration) -> bool {
    std::fs::metadata(dir.join(shim_files::SPEC))
        .and_then(|m| m.modified())
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .is_some_and(|elapsed| elapsed < age)
}

fn classify_shim(key: PhaseKey, dir: PathBuf) -> AdoptedShim {
    let identity = read_identity(&dir.join(shim_files::SHIM));
    let state = match read_exit_json(&dir) {
        ExitJson::Record(record) => AdoptedState::Exited(record),
        ExitJson::Damaged(reason) => AdoptedState::Damaged { reason },
        ExitJson::Missing => {
            let alive = match &identity {
                Some(id) => process_is_running(id.pid, Some(id.started_unix_us)),
                // No identity yet: a shim that is still starting has not written `shim.json` or bound its socket. It
                // is alive when its socket answers, or when its `spec.json` is too young for it to have done either.
                None => {
                    UnixStream::connect(dir.join(shim_files::SOCKET_DIR).join(shim_files::SOCKET))
                        .is_ok()
                        || spec_is_younger_than(&dir, BIND_WAIT)
                }
            };
            if alive {
                AdoptedState::Alive
            } else {
                AdoptedState::Lost
            }
        }
    };
    AdoptedShim {
        key,
        dir,
        identity,
        state,
    }
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
        assert!(
            paths
                .socket
                .starts_with("/Users/someone/Library/Application Support/Collide/run")
        );
        assert!(DaemonPaths::from_env(None, None).is_err());
        assert!(DaemonPaths::from_env(Some(""), Some("")).is_err());
    }

    #[test]
    fn a_home_gives_the_run_files_in_application_support_and_the_data_where_it_was() {
        let paths = DaemonPaths::for_home(Path::new("/Users/someone"));
        let support = "/Users/someone/Library/Application Support/Collide";
        assert_eq!(paths.socket_dir, Path::new(support).join("run"));
        assert_eq!(paths.socket, Path::new(support).join("run/d.sock"));
        assert_eq!(paths.token_file, Path::new(support).join("run/token"));
        assert_eq!(paths.lock_file, Path::new(support).join("run/lock"));
        assert_eq!(paths.runs_dir, Path::new(support).join("runs"));
        assert_eq!(paths.store_dir, Path::new(support).join("store"));
        assert_eq!(paths.log_file, Path::new(support).join("daemon.log"));
    }

    #[test]
    fn a_home_makes_no_folder_of_its_own() {
        let paths = DaemonPaths::for_home(Path::new("/Users/someone"));
        for path in [
            &paths.socket_dir,
            &paths.socket,
            &paths.token_file,
            &paths.lock_file,
            &paths.runs_dir,
            &paths.store_dir,
            &paths.log_file,
        ] {
            assert!(
                !path.to_string_lossy().contains("/.collide"),
                "{}",
                path.display()
            );
        }
    }

    #[test]
    fn the_default_socket_fits_for_an_ordinary_home() {
        let paths = DaemonPaths::for_home(Path::new("/Users/someone-with-a-longer-name"));
        assert!(paths.socket.as_os_str().len() <= MAX_SOCKET_PATH_BYTES);
        assert!(crate::auth::check_socket_path(&paths.socket).is_ok());
    }

    #[test]
    fn a_home_too_long_for_the_socket_fails_with_the_path_and_the_limit() {
        let home = format!("/Users/{}", "a".repeat(80));
        let paths = DaemonPaths::for_home(Path::new(&home));
        let error = crate::auth::check_socket_path(&paths.socket).unwrap_err();
        let message = error.to_string();
        assert!(
            message.contains(&paths.socket.display().to_string()),
            "{message}"
        );
        assert!(
            message.contains(&MAX_SOCKET_PATH_BYTES.to_string()),
            "{message}"
        );
    }
}
