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
//! and `board.list` (a git checkout as a board, [`crate::boards`]), and `task.get`, `task.apply` and `task.preview` for
//! any source of a board ([`crate::sources`]; the scratch source is [`crate::scratch`]), `run.list` and `run.get`
//! (from the store's index of runs, [`crate::run_store`]) and `daemon.subscribe` are served here; the other methods
//! are other items' and answer `method not found`.
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
//! itself stops it first ends the environment captures in flight ([`crate::base_env::CaptureSessions`]), so no login
//! shell outlives it, then ends its watchers and then boots out the launchd job its start recorded
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
    self, Admitted, Caller, ProcessTree, Session, StdFs, SystemProcessTree, UnixPeer, Verdict,
};
use crate::base_env::{BaseEnvironment, CaptureOptions, CaptureSessions, EnvSource};
use crate::board_lines::LineError;
use crate::boards;
use crate::checkout::root_busy;
use crate::claude_versions::{ClaudeVersions, PublishedChoice, REFRESH_EVERY, VersionsOptions};
use crate::clock::{Clock, ClockPair, Clocks, Reading, SystemClocks, civil_date, local_utc_offset};
use crate::config_get::{
    BoardRef, ConfigService, GetError, LoadContextFactory, SetError, system_load_context,
};
use crate::corpus_service::{Corpora, CorporaConfig};
use crate::disk::{FloorSource, FreeSpace, SystemFreeSpace};
use crate::event_hub::{Hub, Subscription};
use crate::file_run::quick_run::RunDiffError;
use crate::file_run::{AnswerError, FileRunConfig, FileRuns};
use crate::follow_ups::{RunTask, shape_create};
use crate::launch::{Launchctl, boot_out_recorded_job};
use crate::login::{self, LoginSessions, SystemLoginSession};
use crate::metrics::Metrics;
use crate::pack_location::Inputs;
use crate::pack_update::PackError;
use crate::phases::{
    BIND_WAIT, ExitJson, FeedRebuilder, PhaseEndHook, PhaseKey, PhaseOptions, PhasePauseHook,
    Phases, PhasesConfig, StopRunError, process_is_running, read_exit_json, read_identity,
};
use crate::queue::Queue;
use crate::run_diff::DiffError;
use crate::run_store::{RunFilter, RunRow};
use crate::run_summary::summarize;
use crate::sampler::{Sampler, TICK};
use crate::scratch;
use crate::secret_store::{MemorySecretStore, SecretStore};
use crate::shim::{ChildIdentity, DaemonStart, files as shim_files};
use crate::sources::{
    PLANNING_SOURCE, Source, SourceError, Sources, SourcesConfig, changed_event, plan,
};
use crate::stop::{Cleanup, Processes, SystemProcesses};
use crate::store::{Store, StoreError};
use crate::supervisor::SupervisorPolicy;
use crate::task_changes;
use crate::views::{
    BeforeParse, Loaded, Lookup, SaveError, ViewCache, builtin_candidates, find_view,
    pack_candidates, personal_candidates, personal_dir, repo_candidates, repo_dir, save_view,
};
use crate::workspace::Git;
use crate::write_guard::{
    self, Applied, Fate, LivePhases, QueuedWrite, WriteGuard, WriteGuardConfig,
};
use collide_core::change::{Actor, ActorKind};
use collide_core::corpus::{ListFilter, Page};
use collide_core::events::Event;
use collide_core::item::Category;
use collide_core::owner::{
    BoardInitParams, BoardListResult, BoardOpenParams, BoardOpenResult, BoardSummary,
    BoardTrustParams, BoardTrustReviewParams, ChangeWords, ChangesPlace, CommitChanges,
    ConfigGetParams, ConfigPackAddParams, ConfigPackEnableParams, ConfigPackLinkParams,
    ConfigPackListParams, ConfigPackListResult, ConfigPackUpdateParams, ConfigSetParams,
    ConfigWriteLayer, DaemonRecaptureEnvironmentResult, DaemonStatusResult, DaemonSubscribeParams,
    DaemonSubscribeResult, EnvCapture, MAX_MESSAGE_BYTES, Method, Notice, OWNER_PROTOCOL,
    OwnerInitializeResult, QueueAnswer, QueueAnswerParams, QueueAnswerResult, QueueItem,
    QueueListParams, QueueListResult, Request, Response, Routed, RpcError, RunDiffParams,
    RunGetParams, RunGetResult, RunListParams, RunListResult, RunRecordParams, RunRecordResult,
    RunScope, RunStartFileParams, RunStartParams, RunStartResult, RunStopParams, RunStopResult,
    RunSummary, StoreHealth, Survivor, TaskApplyParams, TaskApplyResult, TaskChangesParams,
    TaskChangesResult, TaskCreateParams, TaskDiffParams, TaskDiffResult, TaskGetParams,
    TaskGetResult, TaskListParams, TaskPreviewParams, TaskReidParams, TaskReidResult,
    TaskSaveViewParams, TaskSaveViewResult, TaskSectionsParams, TaskViewsParams, TaskViewsResult,
    ViewEntry, ViewRef, ViewReport, ViewTarget, WriteQueued, code, encode_line, parse_request,
    route,
};
use collide_core::provider::{ApplyActor, ApplyActorKind, ApplyParams, ApplyResult, GetParams};
use collide_core::section::{ProposalFeatures, section_uses};
use collide_core::shim::ExitRecord;
use collide_core::view_eval::view_report;
use std::collections::{BTreeMap, BTreeSet, HashSet};
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

/// Refuses a `HOME` that names no folder (ts-0164-pa). A root below a deleted home would otherwise make the home again
/// as it makes its own folders, and the daemon would run in a home nobody asked for. An unset or empty `HOME` passes:
/// it names nothing, and [`DaemonPaths::from_env`] says when a root is needed and none is given. The check runs before
/// anything is made, in the daemon, in the start that runs it, and in the app.
pub fn check_home(home: Option<&str>) -> Result<(), String> {
    match home {
        Some(home) if !home.is_empty() && !Path::new(home).is_dir() => Err(format!(
            "HOME names no folder: {home}. Collide does not make it, so nothing was started. \
             Make the folder, or set HOME to one that exists"
        )),
        _ => Ok(()),
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
    /// A peer closed before its pid could be read (`ENOTCONN`). Nothing asked to come in and nothing was refused: a
    /// probe that connects and leaves ends here, and [`log_refusal`] does not log it.
    PeerClosed,
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
            SocketError::PeerClosed => {
                write!(f, "the connecting peer closed before its pid was read")
            }
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
    /// The zone, in seconds east of UTC, that dates the `at` of an actor ([`ApplyActor`]). `None`, the default, is the
    /// machine's own zone. A test gives one so its date does not depend on where it runs.
    pub utc_offset_secs: Option<i64>,
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
    /// Where free space is read from. The floor under which nothing starts is the `disk.floor_gib` setting of the
    /// run's board, 5 GiB when none sets it (0.2, Durability).
    pub free_space: Arc<dyn FreeSpace + Send + Sync>,
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
    /// [`crate::secret_store::system_secret_store`] when it runs for a person (ts-0099-bw).
    pub secret_store: Arc<dyn SecretStore>,
    /// How the providers of a board's sources are supervised: the timeout of each method and the backoff between
    /// restarts. A test gives short ones.
    pub supervisor: SupervisorPolicy,
    /// How often the writer looks at the queued writes of a checkout that is still busy ([`WRITE_POLL`]). A write
    /// also goes at once when a phase ends. A test passes a short one.
    pub write_poll: Duration,
    /// How often a board's corpus looks at its checkouts and at the sources that need a new list ([`CORPUS_RECHECK`]).
    /// A test passes a short one.
    pub corpus_recheck: Duration,
    /// Called with a view's `where` text just before it is parsed. A test makes it panic, to see that one view whose
    /// compile panics is that view's error and nothing else's. `None`, the default, parses as usual.
    pub view_before_parse: Option<BeforeParse>,
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
            utc_offset_secs: None,
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
            secret_store: Arc::new(MemorySecretStore::new()),
            supervisor: SupervisorPolicy::default(),
            write_poll: WRITE_POLL,
            corpus_recheck: CORPUS_RECHECK,
            view_before_parse: None,
        }
    }
}

/// How often a board's corpus reads the checkouts again and lists the sources that have no `watch` (0.10, Corpus).
pub const CORPUS_RECHECK: Duration = Duration::from_secs(10);

/// How often a queued write looks again at a checkout that was busy, for what no phase's end tells it: a rebase that
/// ended (ts-0114-hr).
pub const WRITE_POLL: Duration = Duration::from_secs(1);

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
    /// The sessions of the captures in flight; the daemon's stop ends them ([`CaptureSessions::end_all`]).
    captures: CaptureSessions,
    tree: Arc<dyn ProcessTree + Send + Sync>,
    uid: u32,
    clocks: Arc<dyn Clocks + Send + Sync>,
    /// [`DaemonOptions::utc_offset_secs`].
    utc_offset_secs: Option<i64>,
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
    /// The providers behind each board's sources ([`crate::sources`]). They stop with the daemon.
    sources: Arc<Sources>,
    /// The corpus of each board ([`crate::corpus_service`]). Set once, right after `Shared` is made, because its
    /// threads reach the daemon through a weak reference.
    corpora: std::sync::OnceLock<Corpora>,
    /// The line locks, the live phases and the queued writes ([`crate::write_guard`]).
    guard: WriteGuard,
    /// What each view's members were, for the corpus they were worked out on ([`crate::views`]).
    views: ViewCache,
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
        let new = BaseEnvironment::capture_in(
            &self.shell,
            self.clocks.as_ref(),
            &self.capture,
            &self.captures,
        );
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
    writer: Option<JoinHandle<()>>,
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
            utc_offset_secs,
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
            git_env,
            login,
            sleep,
            limit_poll,
            limit_min_pause,
            sampler_tick,
            claude_published,
            claude_refresh_every,
            subscriber_queue,
            write_poll,
            corpus_recheck,
            view_before_parse,
            feed_rebuilder,
            load_context,
            secret_store,
            supervisor,
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
        // A source is made once the start capture ends: until then the published version is "not known yet"
        // (ts-0166-rq).
        if !matches!(claude_published, PublishedChoice::Off) {
            claude.expect_source();
        }
        // `config.get` reads the repo's layer through the loader. The cache is beside the runs, like the workspaces.
        let config = ConfigService::new(
            home.clone(),
            load_context.unwrap_or_else(|| {
                system_load_context(
                    paths.runs_dir.with_file_name("cache"),
                    Inputs::from_process(),
                    home.clone().filter(|home| !home.is_empty()).map(Into::into),
                )
            }),
            git_env.clone(),
            secret_store,
        )
        .with_workspaces({
            // The worktree root and the paths the store recorded: a `dev:` folder may not be under either.
            let root = paths.runs_dir.with_file_name("workspaces");
            let store = store.clone();
            Arc::new(move || {
                let mut found = vec![root.clone()];
                if let Ok(contexts) = store.run_contexts() {
                    for (_, json) in contexts {
                        let value: serde_json::Value =
                            serde_json::from_str(&json).unwrap_or_default();
                        if let Some(path) =
                            value.get("workspace_path").and_then(|path| path.as_str())
                        {
                            found.push(std::path::PathBuf::from(path));
                        }
                    }
                }
                found
            })
        });
        // The floor a run starts above is the `disk.floor_gib` of its board, read at each check.
        let floor: FloorSource = {
            let (config, store, env, log) =
                (config.clone(), store.clone(), env.clone(), log.clone());
            Arc::new(move |board| {
                let root = board.and_then(|id| store.board_root(id).ok().flatten());
                let board = board.zip(root.as_deref()).map(|(id, root)| BoardRef {
                    id,
                    root: Path::new(root),
                });
                let read = config.floor(board, &captured_path(&env));
                for problem in &read.problems {
                    log.line(&format!("disk.floor_gib: {problem}"));
                }
                read.floor
            })
        };
        let live = Arc::new(LivePhases::new());
        let guard = WriteGuard::new(WriteGuardConfig {
            queue: queue.clone(),
            live: live.clone(),
            git_env: git_env.clone(),
            now_ms: {
                let clocks = clocks.clone();
                Arc::new(move || clocks.read(Clock::Wall).nanos / 1_000_000)
            },
            log: {
                let log = log.clone();
                Arc::new(move |text: &str| log.line(text))
            },
        });
        let file_runs = Arc::new(FileRuns::new(FileRunConfig {
            live,
            runs_dir: paths.runs_dir.clone(),
            workspaces_dir: workspaces_dir
                .clone()
                .unwrap_or_else(|| paths.runs_dir.with_file_name("workspaces")),
            home: home.clone(),
            clocks: clocks.clone(),
            free_space: free_space.clone(),
            floor: floor.clone(),
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
            collide_exe: shim_exe.clone(),
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
            queue: queue.clone(),
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

        // The providers of a board's sources: their data folders are beside the runs, and they wait for the first
        // capture of the base environment like a run does.
        let sources = Arc::new(Sources::new(SourcesConfig {
            config: config.clone(),
            store: store.clone(),
            data_root: paths.runs_dir.with_file_name("pack-data"),
            policy: supervisor,
            base_env: {
                let env = env.clone();
                let wait = capture.timeout + CAPTURE_WAIT_MARGIN;
                Arc::new(move || wait_for_environment(&env, wait))
            },
        }));

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
            captures: CaptureSessions::default(),
            tree,
            uid,
            started_wall_ms: clocks.read(Clock::Wall).nanos / 1_000_000,
            started_uptime: clocks.read(Clock::Uptime),
            clocks,
            utc_offset_secs,
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
            sources,
            corpora: std::sync::OnceLock::new(),
            guard,
            views: ViewCache::new(view_before_parse),
        });
        let corpora = make_corpora(&shared, workspaces_dir, corpus_recheck);
        let _ = shared.corpora.set(corpora);

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
        let writing = shared.clone();
        let writer = spawn_named("collide-write-queue", move || {
            writer_loop(&writing, write_poll);
        })
        .map_err(|source| SocketError::Io {
            context: "could not start the queued writes' thread".to_string(),
            source,
        })?;
        // Each stored board's corpus fills on its own thread, so nothing here waits for a provider.
        if let Some(corpora) = shared.corpora.get() {
            corpora.start_stored();
        }
        Ok(Daemon {
            shared,
            accept: Some(accept),
            sampler: Some(sampler),
            writer: Some(writer),
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

    /// The live phases the line guards read. A test registers a phase here, as a run's workspace does, to see a write
    /// wait for it.
    pub fn live_phases(&self) -> Arc<LivePhases> {
        self.shared.guard.live().clone()
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
        if let Some(writer) = self.writer.take() {
            self.shared.stop.store(true, Ordering::SeqCst);
            let _ = writer.join();
        }
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        // A capture runs in a session of its own, which nothing else ends: the daemon would leave its shell behind.
        self.shared.captures.end_all();
        self.join();
        // The providers of the sources end with the daemon: no child outlives it.
        self.shared.sources.stop();
        // The corpus threads end once the sources do: a call that waited on a provider has returned.
        if let Some(corpora) = self.shared.corpora.get() {
            corpora.shutdown();
        }
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

/// Applies the queued writes of every checkout that can have them applied now, at start and then whenever a phase ended
/// or `poll` passed. It sleeps in slices so a stop is seen within one.
fn writer_loop(shared: &Shared, poll: Duration) {
    let slice = SAMPLER_SLICE.min(poll);
    drain_all(shared);
    loop {
        let mut waited = Duration::ZERO;
        while waited < poll {
            if shared.stopping() {
                return;
            }
            if shared.guard.live().take_released() {
                break;
            }
            std::thread::sleep(slice);
            waited += slice;
        }
        if shared.stopping() {
            return;
        }
        drain_all(shared);
    }
}

fn drain_all(shared: &Shared) {
    for checkout in shared.guard.checkouts_waiting() {
        shared
            .guard
            .hold(&checkout, || drain_checkout(shared, &checkout));
    }
}

/// Applies the queued writes of `checkout`, oldest first. The caller holds its line lock.
fn drain_checkout(shared: &Shared, checkout: &Path) {
    shared
        .guard
        .drain(checkout, &mut |write| apply_queued(shared, write));
}

/// Applies a stored write as the person who asked: the same `apply` and the same events as a write that was not
/// queued. A write whose version is old is not applied: the source says conflict, and the Queue says which task moved.
fn apply_queued(shared: &Shared, write: QueuedWrite) -> Applied {
    let root = match board_root(shared, &write.board) {
        Ok(root) => root,
        Err(error) => return Applied::Problem(error.message),
    };
    let source = match source_of(shared, &write.board, &root, &write.source) {
        Ok(source) => source,
        Err(error) => return Applied::Problem(source_failed(error).message),
    };
    match apply_and_record(
        shared,
        &write.board,
        write.actor.clone(),
        &source,
        &write.apply,
    ) {
        Ok(result) => settled(&result),
        Err(error) => Applied::Problem(source_failed(error).message),
    }
}

/// How a stored write ended, from the source's answer to it, in Collide's words.
fn settled(result: &ApplyResult) -> Applied {
    match result {
        ApplyResult::Ok { .. } => Applied::Done,
        ApplyResult::Conflict { current } => Applied::Problem(format!(
            "{} changed since this write was asked for; it is now {}. Read it and ask again",
            current.key, current.state.status
        )),
        ApplyResult::Refused { message } => Applied::Problem(message.clone()),
    }
}

/// The corpus services of the daemon's boards. Their threads hold the daemon weakly: a daemon that is dropped ends
/// them.
fn make_corpora(
    shared: &Arc<Shared>,
    workspaces_dir: Option<PathBuf>,
    recheck: Duration,
) -> Corpora {
    let weak = Arc::downgrade(shared);
    let phases = shared.phases.clone();
    let log = shared.log.clone();
    Corpora::new(CorporaConfig {
        sources: shared.sources.clone(),
        store: shared.store.clone(),
        git_env: shared.git_env.clone(),
        workspaces_dir: workspaces_dir
            .unwrap_or_else(|| shared.paths.runs_dir.with_file_name("workspaces")),
        tools: {
            let weak = weak.clone();
            Arc::new(move || {
                weak.upgrade()
                    .map(|shared| tools_path(&shared))
                    .unwrap_or_default()
            })
        },
        record: {
            let log = log.clone();
            Arc::new(move |body| {
                let actor = Actor {
                    kind: collide_core::change::ActorKind::Workflow,
                    run: None,
                    phase: None,
                };
                if let Err(error) = phases.append_event(actor, body) {
                    log.line(&format!("a corpus event was not recorded: {error}"));
                }
            })
        },
        log: {
            let log = log.clone();
            Arc::new(move |text: &str| log.line(text))
        },
        stopping: Arc::new(move || weak.upgrade().is_none_or(|shared| shared.stopping())),
        recheck,
    })
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

/// Logs why [`auth::admit`] turned a connection away. A peer that closed before it was read asked for nothing, so it
/// is not logged; every other refusal is.
fn log_refusal(log: &DaemonLog, refused: &SocketError) {
    if matches!(refused, SocketError::PeerClosed) {
        return;
    }
    log.line(&format!("a connection was refused: {refused}"));
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
            log_refusal(&shared.log, &refused);
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
        Method::ConfigSet => config_set(shared, session, params),
        Method::ConfigPackUpdate => config_pack_update(shared, session, params),
        Method::ConfigPackList => config_pack_list(shared, params),
        Method::ConfigPackAdd => config_pack_add(shared, session, params),
        Method::ConfigPackLink => config_pack_link(shared, session, params),
        Method::ConfigPackEnable => config_pack_enable(shared, session, params),
        Method::BoardOpen => board_open(shared, params),
        Method::BoardList => board_list(shared),
        Method::BoardInit => board_init(shared, session, params),
        Method::BoardTrust => board_trust(shared, session, params),
        Method::BoardTrustReview => board_trust_review(shared, params),
        Method::TaskList => task_list(shared, params),
        Method::TaskGet => task_get(shared, params),
        Method::TaskApply => task_apply(shared, session, params),
        Method::TaskCreate => task_create(shared, session, params),
        Method::TaskReid => task_reid(shared, session, params),
        Method::TaskSections => task_sections(shared, params),
        Method::TaskChanges => task_changes(shared, params),
        Method::TaskDiff => task_diff(shared, params),
        Method::TaskPreview => task_preview(shared, session, params),
        Method::TaskViews => task_views(shared, params),
        Method::TaskSaveView => task_save_view(shared, session, params),
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
        warnings: shared
            .store
            .run_warnings(&run)
            .map_err(|e| unreadable("warnings", e))?,
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
    let heads = shared.store.phase_heads(&row.run).map_err(store_failed)?;
    let (mut summary, rounds) = summarize(
        row,
        &shared.phases.run_phases(&row.run),
        &spans,
        &heads,
        workspace,
    );
    summary.warnings = shared.store.run_warnings(&row.run).map_err(store_failed)?;
    Ok((summary, rounds))
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
    if shared.guard.is_confirmation(&params.item) {
        return confirmation_answer(shared, session, &params);
    }
    if shared.guard.is_queued_write(&params.item) || shared.guard.fate_of(&params.item).is_some() {
        return queued_write_answer(shared, &params);
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

/// The answer to the Queue item of a queued write (ts-0114-hr). Retry looks again now: the write is applied when the
/// checkout is free, and the item stays, with its reason, when it is not. Dismiss drops the write.
///
/// A Retry also says what became of a write that left the Queue (ts-0142-ua), once the drain that took it has ended:
/// `done` when it was applied, `refused` with the reason when it was taken and could not be. A write that a person
/// dismissed, or one the daemon has no record of (the record is kept in memory and for the last few writes only), is
/// not found, and the command that asks does not take that for applied.
fn queued_write_answer(
    shared: &Shared,
    params: &QueueAnswerParams,
) -> Result<serde_json::Value, RpcError> {
    let result = match &params.answer {
        QueueAnswer::Dismiss {} => {
            shared.guard.dismiss(&params.item);
            QueueAnswerResult::Done {}
        }
        QueueAnswer::Retry {} => {
            if let Some(checkout) = shared.guard.checkout_of(&params.item) {
                shared
                    .guard
                    .hold(&checkout, || drain_checkout(shared, &checkout));
            }
            if shared.guard.is_queued_write(&params.item) {
                QueueAnswerResult::Queued {
                    item: params.item.clone(),
                }
            } else {
                match shared.guard.settled_fate(&params.item) {
                    Some(Fate::Applied) => QueueAnswerResult::Done {},
                    Some(Fate::NotApplied(message)) => QueueAnswerResult::Refused { message },
                    Some(Fate::InFlight(_)) | None => {
                        return Err(RpcError::new(
                            code::NOT_FOUND,
                            format!("the Queue has no item {:?}", params.item),
                        ));
                    }
                }
            }
        }
        _ => {
            return Err(RpcError::new(
                code::INVALID_PARAMS,
                "a queued write takes Retry or Dismiss",
            ));
        }
    };
    Ok(serde_json::to_value(result).expect("a result is JSON"))
}

/// The answer to the Queue item of a confirmation (ts-0128-bb; 0.10, Planning sources): a person-only write the CLI
/// asked for. Accept writes it as a person, once, and only the app may say it: the CLI that asked is not the person.
/// If the checkout is busy the write is queued like any other and the answer says so. A write the source then refuses,
/// or finds changed, is not written and a `Conflict` item says why. Dismiss drops the write.
fn confirmation_answer(
    shared: &Shared,
    session: &Session,
    params: &QueueAnswerParams,
) -> Result<serde_json::Value, RpcError> {
    let result = match &params.answer {
        QueueAnswer::Dismiss {} => {
            shared.guard.dismiss(&params.item);
            QueueAnswerResult::Done {}
        }
        QueueAnswer::Accept {} => {
            if !matches!(session.caller(), Caller::App) {
                return Err(RpcError::new(
                    code::NOT_PERMITTED,
                    "only a person in the app accepts a confirmation",
                ));
            }
            let Some(write) = shared.guard.take_confirmation(&params.item) else {
                return Err(RpcError::new(
                    code::NOT_FOUND,
                    format!("the daemon has no confirmation {:?}", params.item),
                ));
            };
            accept_confirmed(shared, &write)
        }
        _ => {
            return Err(RpcError::new(
                code::INVALID_PARAMS,
                "a confirmation takes Accept or Dismiss",
            ));
        }
    };
    Ok(serde_json::to_value(result).expect("a result is JSON"))
}

/// Writes a confirmed write as a person, through the one path every write takes ([`write`]).
fn accept_confirmed(shared: &Shared, confirmed: &QueuedWrite) -> QueueAnswerResult {
    let done = board_root(shared, &confirmed.board).and_then(|root| {
        let source =
            source_of(shared, &confirmed.board, &root, &confirmed.source).map_err(source_failed)?;
        write(
            shared,
            Caller::App.actor(),
            &confirmed.board,
            &root,
            &source,
            &confirmed.apply,
        )
    });
    match done {
        Ok(Written::Applied(result)) => {
            if let Applied::Problem(message) = settled(&result) {
                shared.guard.give_up(confirmed, &message);
            }
            QueueAnswerResult::Done {}
        }
        Ok(Written::Queued(queued)) => QueueAnswerResult::Queued { item: queued.item },
        Err(error) => {
            shared.guard.give_up(confirmed, &error.message);
            QueueAnswerResult::Done {}
        }
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

/// `config.pack_update` (ts-0100-pu): `collide pack update`. The board is required: a pack is updated for one. A fetch
/// may take as long as its timeout, so this answers when it is done and no other request waits on it.
fn config_pack_update(
    shared: &Shared,
    session: &Session,
    params: &serde_json::Value,
) -> Result<serde_json::Value, RpcError> {
    let params: ConfigPackUpdateParams = read_params("config.pack_update", params)?;
    refuse_run_agent(session, "move a pack")?;
    if shared.stopping() {
        return Err(RpcError::new(
            code::UNAVAILABLE,
            "the daemon is shutting down",
        ));
    }
    let root = std::path::PathBuf::from(board_root(shared, &params.board)?);
    let board = BoardRef {
        id: &params.board,
        root: &root,
    };
    match shared
        .config
        .pack_update(&params, board, &tools_path(shared))
    {
        Ok(result) => Ok(serde_json::to_value(result).expect("a result is JSON")),
        Err(PackError::NotFound(message)) => Err(RpcError::new(code::NOT_FOUND, message)),
        Err(PackError::Refused(message)) => Err(RpcError::new(code::CONFLICT, message)),
        Err(PackError::Failed(message)) => Err(RpcError::new(code::INTERNAL_ERROR, message)),
    }
}

/// How a pack request that failed is answered: not found, refused (a conflict with what is there) or broken.
fn pack_error(error: PackError) -> RpcError {
    match error {
        PackError::NotFound(message) => RpcError::new(code::NOT_FOUND, message),
        PackError::Refused(message) => RpcError::new(code::CONFLICT, message),
        PackError::Failed(message) => RpcError::new(code::INTERNAL_ERROR, message),
    }
}

/// The checkout of the board a `config.pack_*` request names, or why the daemon cannot answer.
fn pack_board(shared: &Shared, id: &str) -> Result<std::path::PathBuf, RpcError> {
    if shared.stopping() {
        return Err(RpcError::new(
            code::UNAVAILABLE,
            "the daemon is shutting down",
        ));
    }
    Ok(std::path::PathBuf::from(board_root(shared, id)?))
}

/// `config.pack_list` (ts-0101-bd): `collide pack list`.
fn config_pack_list(
    shared: &Shared,
    params: &serde_json::Value,
) -> Result<serde_json::Value, RpcError> {
    let params: ConfigPackListParams = read_params("config.pack_list", params)?;
    let root = pack_board(shared, &params.board)?;
    let board = BoardRef {
        id: &params.board,
        root: &root,
    };
    let packs = shared
        .config
        .pack_list(board, &tools_path(shared))
        .map_err(pack_error)?;
    Ok(serde_json::to_value(ConfigPackListResult { packs }).expect("a result is JSON"))
}

/// `config.pack_add` (ts-0101-bd): `collide pack add`. No board: a user pack is the user's on every board. A fetch
/// may take as long as its timeout.
fn config_pack_add(
    shared: &Shared,
    session: &Session,
    params: &serde_json::Value,
) -> Result<serde_json::Value, RpcError> {
    let params: ConfigPackAddParams = read_params("config.pack_add", params)?;
    refuse_run_agent(session, "add a pack")?;
    if shared.stopping() {
        return Err(RpcError::new(
            code::UNAVAILABLE,
            "the daemon is shutting down",
        ));
    }
    let result = shared
        .config
        .pack_add(&params, &tools_path(shared))
        .map_err(pack_error)?;
    Ok(serde_json::to_value(result).expect("a result is JSON"))
}

/// `config.pack_link` (ts-0101-bd): `collide pack link`.
fn config_pack_link(
    shared: &Shared,
    session: &Session,
    params: &serde_json::Value,
) -> Result<serde_json::Value, RpcError> {
    let params: ConfigPackLinkParams = read_params("config.pack_link", params)?;
    refuse_run_agent(session, "link a pack")?;
    let root = pack_board(shared, &params.board)?;
    let board = BoardRef {
        id: &params.board,
        root: &root,
    };
    let result = shared
        .config
        .pack_link(&params, board, &tools_path(shared))
        .map_err(pack_error)?;
    Ok(serde_json::to_value(result).expect("a result is JSON"))
}

/// `config.pack_enable` (ts-0101-bd): `collide pack enable` and `disable`.
fn config_pack_enable(
    shared: &Shared,
    session: &Session,
    params: &serde_json::Value,
) -> Result<serde_json::Value, RpcError> {
    let params: ConfigPackEnableParams = read_params("config.pack_enable", params)?;
    refuse_run_agent(session, "switch a pack on or off")?;
    let root = pack_board(shared, &params.board)?;
    let board = BoardRef {
        id: &params.board,
        root: &root,
    };
    let result = shared
        .config
        .pack_enable(&params, board, &tools_path(shared))
        .map_err(pack_error)?;
    Ok(serde_json::to_value(result).expect("a result is JSON"))
}

/// `config.set` (ts-0099-bw): a secret to the keychain, or a plain value to the user's file ([`crate::config_get`]).
/// The request's `value` may be a secret, so nothing here reads it into a message: the params that cannot be read are
/// answered without the serde error (which can quote a value), no line is logged for the call, and the answer holds no
/// value.
fn config_set(
    shared: &Shared,
    session: &Session,
    params: &serde_json::Value,
) -> Result<serde_json::Value, RpcError> {
    let params: ConfigSetParams = serde_json::from_value(params.clone()).map_err(|_| {
        RpcError::new(
            code::INVALID_PARAMS,
            "the params of config.set are not readable: they need `key`, `value` and `layer`, and may have `board` \
             and `secret`",
        )
    })?;
    refuse_run_agent(session, "change the person's settings")?;
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
        Ok(result) => {
            // A provider reads its settings and secrets when it starts: the next call starts it with the new ones
            // (ts-0113-ck). A user key stops the pack's providers on every board, a repo key on its board's.
            let only = match params.layer {
                ConfigWriteLayer::User => None,
                ConfigWriteLayer::Repo => params.board.as_deref(),
            };
            shared.sources.stop_pack(only, &params.key);
            Ok(serde_json::to_value(result).expect("a result is JSON"))
        }
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
    captured_path(&shared.env)
}

/// The `PATH` of the captured environment, or the process's own before there is one.
fn captured_path(env: &Mutex<Option<BaseEnvironment>>) -> std::ffi::OsString {
    env.lock()
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
                board: Some(with_role_problems(
                    shared,
                    shared
                        .config
                        .lines()
                        .summary(&id, &root, Some(&checkout), &tools_path(shared)),
                )),
                checks: vec![boards::git_repo_ok(&checkout)],
            }
        }
    };
    Ok(serde_json::to_value(result).expect("a result is JSON"))
}

/// A run's agent may not change what the person trusts, what the repo holds or the person's settings (0.9: trust is
/// the person's act, and adding a user pack or linking a folder trusts it). So `board.init`, `board.trust`,
/// `config.set` and the `config.pack_*` writes are refused to it; the reads are not.
fn refuse_run_agent(session: &Session, what: &str) -> Result<(), RpcError> {
    match session.caller() {
        Caller::RunAgent { run } => Err(RpcError::new(
            code::NOT_PERMITTED,
            format!("the agent of run {run} may not {what}"),
        )),
        _ => Ok(()),
    }
}

/// `board.init` (ts-0102-vu): `collide init`, the starter `.collide/board.yaml` of a board ([`crate::board_init`]).
fn board_init(
    shared: &Shared,
    session: &Session,
    params: &serde_json::Value,
) -> Result<serde_json::Value, RpcError> {
    let params: BoardInitParams = read_params("board.init", params)?;
    refuse_run_agent(session, "write a board's files")?;
    let root = pack_board(shared, &params.board)?;
    let result = crate::board_init::init(&root, params.dry_run).map_err(|error| match error {
        crate::board_init::InitError::Exists(message) => RpcError::new(code::CONFLICT, message),
        crate::board_init::InitError::Failed(message) => {
            RpcError::new(code::INTERNAL_ERROR, message)
        }
    })?;
    Ok(serde_json::to_value(result).expect("a result is JSON"))
}

fn line_error(error: LineError) -> RpcError {
    match error {
        LineError::NotFound(message) => RpcError::new(code::NOT_FOUND, message),
        LineError::Refused(message) => RpcError::new(code::CONFLICT, message),
        LineError::Invalid(message) => RpcError::new(code::INVALID_PARAMS, message),
        LineError::Failed(message) => RpcError::new(code::INTERNAL_ERROR, message),
    }
}

/// `board.trust` (ts-0102-vu): `collide trust`, the line's hash recorded outside the repo ([`crate::board_lines`]).
fn board_trust(
    shared: &Shared,
    session: &Session,
    params: &serde_json::Value,
) -> Result<serde_json::Value, RpcError> {
    let params: BoardTrustParams = read_params("board.trust", params)?;
    refuse_run_agent(session, "trust a line's packs")?;
    let root = pack_board(shared, &params.board)?;
    let board = BoardRef {
        id: &params.board,
        root: &root,
    };
    let result = shared
        .config
        .lines()
        .trust(board, &params, &tools_path(shared))
        .map_err(line_error)?;
    // The corpus lists the packs that were refused until now at its next look; this makes that look now.
    if let Some(service) = shared
        .corpora
        .get()
        .and_then(|corpora| corpora.get(&params.board))
    {
        service.wake();
    }
    Ok(serde_json::to_value(result).expect("a result is JSON"))
}

/// `board.trust_review` (ts-0102-vu): what a person sees before they trust a line. It writes nothing.
fn board_trust_review(
    shared: &Shared,
    params: &serde_json::Value,
) -> Result<serde_json::Value, RpcError> {
    let params: BoardTrustReviewParams = read_params("board.trust_review", params)?;
    let root = pack_board(shared, &params.board)?;
    let board = BoardRef {
        id: &params.board,
        root: &root,
    };
    let result = shared
        .config
        .lines()
        .review(board, &params.line, &tools_path(shared))
        .map_err(line_error)?;
    Ok(serde_json::to_value(result).expect("a result is JSON"))
}

/// `summary` with the problems the board's `roles` setting has against the planning source's kinds (ts-0135-cx),
/// added to its line. They are known once the source has started, which is at the first look at the board's tasks.
fn with_role_problems(shared: &Shared, mut summary: BoardSummary) -> BoardSummary {
    let problems = shared.sources.role_problems(&summary.id);
    if let Some(line) = summary.lines.first_mut() {
        for problem in problems {
            if !line.problems.contains(&problem) {
                line.problems.push(problem);
            }
        }
    }
    summary
}

/// `board.list`: the boards opened before, from the store. A board's line is the branch its checkout is on now, read
/// from git; a board whose folder is gone or is no longer a checkout is listed with no line, and with the reason the
/// corpus gives for not reading its sources.
fn board_list(shared: &Shared) -> Result<serde_json::Value, RpcError> {
    let lines = shared.config.lines();
    let tools = tools_path(shared);
    let boards = shared
        .store
        .boards()
        .map_err(store_failed)?
        .into_iter()
        .map(|board| {
            let checkout = boards::checkout_of(Path::new(&board.root), &shared.git_env).ok();
            let summary = with_role_problems(
                shared,
                lines.summary(&board.id, &board.root, checkout.as_ref(), &tools),
            );
            BoardSummary {
                sources_unread: shared
                    .corpora
                    .get()
                    .and_then(|corpora| corpora.get(&board.id))
                    .and_then(|service| service.sources_unread()),
                ..summary
            }
        })
        .collect();
    Ok(serde_json::to_value(BoardListResult { boards }).expect("a result is JSON"))
}

/// The source `name` of `board`, started if need be ([`Sources::source`]).
fn source_of(shared: &Shared, board: &str, root: &str, name: &str) -> Result<Source, SourceError> {
    let board = BoardRef {
        id: board,
        root: Path::new(root),
    };
    shared.sources.source(board, name, &tools_path(shared))
}

/// A source's failure as the error of a request.
fn source_failed(error: SourceError) -> RpcError {
    match error {
        SourceError::Refused(message) => RpcError::new(code::CONFLICT, message),
        SourceError::NoSource(message) | SourceError::Unsupported(message) => {
            RpcError::new(code::INVALID_PARAMS, message)
        }
        SourceError::Unavailable(message) => RpcError::new(code::UNAVAILABLE, message),
        SourceError::Failed(message) => RpcError::new(code::INTERNAL_ERROR, message),
    }
}

/// `task.list` (ts-0116-wh, ts-0119-tj): the board's corpus, through a view when the request names or gives one. It
/// starts the board's corpus if it has not, and answers at once with the cached tasks (`refreshing`) while the first
/// listing runs. A named view is one of [`task_views`], by its full name or by its bare one ([`find_view`],
/// ts-0135-cx): a name no view has is `NOT_FOUND`, a bare name several views have is `INVALID_PARAMS` that lists them,
/// and a view that has an error lists no task and carries the error in `view`. An inline view is checked as a saved one
/// is, and a bad one is `INVALID_PARAMS` with the reason. `ready`, `kind` and `checkout` narrow what the view gives,
/// `limit` and `cursor` page it, and the board-wide lists come with the first page.
fn task_list(shared: &Shared, params: &serde_json::Value) -> Result<serde_json::Value, RpcError> {
    let params: TaskListParams = read_params("task.list", params)?;
    let root = board_root(shared, &params.board)?;
    let service = corpus_of(shared, &params.board, &root)?;
    if let Some(kind) = &params.kind {
        let declared = service.read(|corpus| corpus.declared_kinds());
        if !declared.is_empty() && !declared.contains(kind) {
            return Err(RpcError::new(
                code::INVALID_PARAMS,
                format!(
                    "unknown kind `{kind}`; the board's sources declare: {}",
                    declared.join(", ")
                ),
            )
            .with_data(serde_json::json!({ "unknown_kind": kind, "declared": declared })));
        }
    }
    let through = match &params.view {
        None => None,
        Some(ViewRef::Inline { view }) => {
            let tasks = service.read(|corpus| corpus.rows().len());
            let compiled = shared
                .views
                .compile(view, tasks)
                .map_err(|failure| RpcError::new(code::INVALID_PARAMS, failure.to_string()))?;
            let result = evaluated(shared, &params.board, &service, &compiled)
                .map_err(|failure| RpcError::new(code::INVALID_PARAMS, failure.to_string()))?;
            Some((view_report(&view.name, &result), result.members.clone()))
        }
        Some(ViewRef::Named { name }) => {
            let BoardViews { loaded, supplying } =
                loaded_views(shared, &params.board, &root, &service)?;
            let found = find_view(&loaded, name, &supplying).map_err(|lookup| {
                let kind = match lookup {
                    Lookup::Missing { .. } => code::NOT_FOUND,
                    Lookup::Ambiguous { .. } => code::INVALID_PARAMS,
                };
                RpcError::new(kind, lookup.message())
            })?;
            // The report names the view found, so a bare name and the full one are answered alike.
            let name = &found.view.name;
            Some(match viewed(shared, &params.board, &service, found) {
                Ok(result) => (view_report(name, &result), result.members.clone()),
                Err(reason) => (ViewReport::failed(name, &reason), Vec::new()),
            })
        }
    };
    // The tasks come in the order of the view's members, which a rule (`bottlenecks`) may give an order of its own.
    let only: Option<BTreeSet<String>> = through
        .as_ref()
        .map(|(_, members)| members.iter().cloned().collect());
    let mut result = service.list(
        &ListFilter {
            ready: params.ready,
            kind: params.kind.as_deref(),
            checkout: params.checkout.as_deref(),
            only: only.as_ref(),
            order: through.as_ref().map(|(_, members)| members.as_slice()),
        },
        &Page {
            limit: params.limit.map(|limit| limit as usize),
            cursor: params.cursor.as_deref(),
        },
    );
    result.view = through.map(|(report, _)| report);
    // The run whose workspace a divergent copy's checkout is: only the workspace owner knows.
    for entry in &mut result.divergence {
        for copy in &mut entry.copies {
            copy.run = shared.file_runs.run_of_workspace(Path::new(&copy.checkout));
        }
    }
    Ok(serde_json::to_value(result).expect("a result is JSON"))
}

/// `task.views` (ts-0119-tj): the board's views with their counts, for the Field's view menu: the person's, the
/// repo's and the packs' ([`crate::views`]). One view's error is on that view and hides no other. A line that does not
/// load is `CONFLICT` with its problems.
fn task_views(shared: &Shared, params: &serde_json::Value) -> Result<serde_json::Value, RpcError> {
    let params: TaskViewsParams = read_params("task.views", params)?;
    let root = board_root(shared, &params.board)?;
    let service = corpus_of(shared, &params.board, &root)?;
    let views = loaded_views(shared, &params.board, &root, &service)?
        .loaded
        .iter()
        .map(|loaded| view_entry(shared, &params.board, &service, loaded))
        .collect();
    Ok(serde_json::to_value(TaskViewsResult { views }).expect("a result is JSON"))
}

/// The board's views in the order of precedence, compiled, and the packs that supply a source of its line.
struct BoardViews {
    loaded: Vec<Loaded>,
    supplying: Vec<String>,
}

/// The board's views, `ready` first, then the person's, the repo's and the packs' ([`crate::views`]).
fn loaded_views(
    shared: &Shared,
    board: &str,
    root: &str,
    service: &crate::corpus_service::BoardCorpus,
) -> Result<BoardViews, RpcError> {
    let lines = shared.config.lines();
    let line = lines
        .for_sources(
            BoardRef {
                id: board,
                root: Path::new(root),
            },
            &tools_path(shared),
        )
        .map_err(line_error)?;
    let mut candidates = builtin_candidates();
    if let Some(home) = lines.home() {
        candidates.extend(
            personal_candidates(home, board)
                .map_err(|message| RpcError::new(code::CONFLICT, message))?,
        );
    }
    candidates.extend(repo_candidates(&line));
    candidates.extend(pack_candidates(&line));
    let tasks = service.read(|corpus| corpus.rows().len());
    Ok(BoardViews {
        loaded: shared.views.assemble(candidates, tasks),
        supplying: plan(&line.line)
            .sources
            .into_iter()
            .map(|planned| planned.pack)
            .collect(),
    })
}

fn evaluated(
    shared: &Shared,
    board: &str,
    service: &crate::corpus_service::BoardCorpus,
    compiled: &collide_core::view_eval::CompiledView,
) -> Result<std::sync::Arc<collide_core::view_eval::ViewResult>, collide_core::view_eval::ViewFailure>
{
    let stamp = service.read(|corpus| corpus.stamp());
    shared.views.evaluate(board, compiled, stamp, || {
        // The copy has the counts of `unblocks` when the corpus has them, so each view of a version shares one pass.
        service.read(|corpus| {
            corpus.unblocks();
            corpus.clone()
        })
    })
}

/// The members of a loaded view, or the reason it has none.
fn viewed(
    shared: &Shared,
    board: &str,
    service: &crate::corpus_service::BoardCorpus,
    loaded: &Loaded,
) -> Result<std::sync::Arc<collide_core::view_eval::ViewResult>, String> {
    if let Some(error) = &loaded.error {
        return Err(error.clone());
    }
    let compiled = loaded.compiled().expect("a view with no error is compiled");
    evaluated(shared, board, service, compiled).map_err(|failure| failure.to_string())
}

/// One view of `task.views`: its counts come from the last evaluation, which is made again when the corpus changed.
/// `error_count` is the board's invalid items, which no view can match, and the tasks whose `where` raised an error or
/// ran out of steps. `problem_count` is the problems in the parts of the view's members, which are no error.
fn view_entry(
    shared: &Shared,
    board: &str,
    service: &crate::corpus_service::BoardCorpus,
    loaded: &Loaded,
) -> ViewEntry {
    let mut entry = ViewEntry {
        view: loaded.view.clone(),
        source: loaded.source.clone(),
        task_count: 0,
        error_count: 0,
        problem_count: 0,
        error: None,
    };
    match viewed(shared, board, service, loaded) {
        Ok(result) => {
            let counts = &result.counts;
            let errors =
                counts.invalid_items as u64 + counts.error_count() + counts.over_task_budget;
            entry.task_count = u32::try_from(result.members.len()).unwrap_or(u32::MAX);
            entry.error_count = u32::try_from(errors).unwrap_or(u32::MAX);
            entry.problem_count = u32::try_from(counts.part_problems).unwrap_or(u32::MAX);
        }
        Err(reason) => entry.error = Some(reason),
    }
    entry
}

/// `task.save_view` (ts-0119-tj): Save as View. The view is checked as a load checks it, and its name must be a safe
/// file name. The repo's file goes below the line's Collide folder, in the working tree, and the personal one in the
/// board's folder of the user's settings. An existing file is refused unless `overwrite`. Nothing is committed. A run's
/// agent may not save one.
fn task_save_view(
    shared: &Shared,
    session: &Session,
    params: &serde_json::Value,
) -> Result<serde_json::Value, RpcError> {
    refuse_run_agent(session, "save a view")?;
    let params: TaskSaveViewParams = read_params("task.save_view", params)?;
    let root = board_root(shared, &params.board)?;
    let service = corpus_of(shared, &params.board, &root)?;
    let tasks = service.read(|corpus| corpus.rows().len());
    shared
        .views
        .compile(&params.view, tasks)
        .map_err(|failure| RpcError::new(code::INVALID_PARAMS, failure.to_string()))?;
    let lines = shared.config.lines();
    let (base, folder, mode) = match params.target {
        ViewTarget::Personal => {
            let home = lines.home().ok_or_else(|| {
                RpcError::new(code::INTERNAL_ERROR, "the daemon knows no home directory")
            })?;
            let folder = personal_dir(home, &params.board)
                .map_err(|message| RpcError::new(code::CONFLICT, message))?;
            let base = folder
                .parent()
                .expect("a views folder has a parent")
                .to_path_buf();
            (base, folder, 0o600)
        }
        ViewTarget::Repo => {
            // Not queued (0.5, Root busy): a save is the person's own act, so a busy root says so and they retry.
            let busy = root_busy(&shared.git_env, Path::new(&root)).map_err(|error| {
                RpcError::new(
                    code::INTERNAL_ERROR,
                    format!("git could not say whether {root} is busy, so the view was not saved: {error}"),
                )
            })?;
            if let Some(what) = busy {
                return Err(RpcError::new(
                    code::CONFLICT,
                    format!(
                        "{what} is in progress in {root}, so the view was not saved there: save it as a personal \
                         view, or try again when it is done"
                    ),
                ));
            }
            let line = lines
                .for_sources(
                    BoardRef {
                        id: &params.board,
                        root: Path::new(&root),
                    },
                    &tools_path(shared),
                )
                .map_err(line_error)?;
            let dir = line.line.collide_dir.ok_or_else(|| {
                RpcError::new(
                    code::CONFLICT,
                    "the board's line has no Collide folder to keep a view in",
                )
            })?;
            let folder = repo_dir(Path::new(&root), &dir)
                .map_err(|message| RpcError::new(code::CONFLICT, message))?;
            (PathBuf::from(&root), folder, 0o644)
        }
    };
    let path = save_view(&params.view, &base, &folder, params.overwrite, mode).map_err(
        |error| match error {
            SaveError::Invalid(message) => RpcError::new(code::INVALID_PARAMS, message),
            SaveError::Exists(message) | SaveError::Blocked(message) => {
                RpcError::new(code::CONFLICT, message)
            }
            SaveError::Failed(message) => RpcError::new(code::INTERNAL_ERROR, message),
        },
    )?;
    let result = TaskSaveViewResult {
        path: path.display().to_string(),
    };
    Ok(serde_json::to_value(result).expect("a result is JSON"))
}

/// The board's corpus, started if it has not been.
fn corpus_of(
    shared: &Shared,
    board: &str,
    root: &str,
) -> Result<Arc<crate::corpus_service::BoardCorpus>, RpcError> {
    shared
        .corpora
        .get()
        .and_then(|corpora| corpora.ensure(board, Path::new(root)))
        .ok_or_else(|| RpcError::new(code::UNAVAILABLE, "the daemon is shutting down"))
}

/// `task.get` (ts-0075-hj, ts-0113-ck, ts-0116-wh): the task with its whole prompt or body, or no task when the board's
/// source has none with that id. An id with no source in front of it is a scratch task. A source the board's line does
/// not have answers no task. A source that cannot answer (an untrusted line, a provider that is gone) is an error that
/// says so. An id the corpus knows (a key, an old key, a short id) also brings the task's copies, the collision of a
/// key held at two paths, the keys a short id fits, its runs newest first (those of its old keys too) and its latest
/// finished run.
fn task_get(shared: &Shared, params: &serde_json::Value) -> Result<serde_json::Value, RpcError> {
    let params: TaskGetParams = read_params("task.get", params)?;
    let root = board_root(shared, &params.board)?;
    let (prefix, id) = scratch::split_id(&params.id);
    let known = match prefix {
        Some(_) => corpus_of(shared, &params.board, &root)?.read(|corpus| {
            let found = corpus.get(&params.id);
            let former = found
                .task
                .as_ref()
                .map(|task| corpus.former_keys(&task.key))
                .unwrap_or_default();
            (found, former)
        }),
        None => (empty_get(), Vec::new()),
    };
    let (mut result, former) = known;
    if result.task.is_some() {
        // What the corpus shows has no body: the source gives the whole task.
        let shown = result.task.clone().expect("checked");
        let (name, short) = shown
            .key
            .split_once(':')
            .unwrap_or((scratch::SOURCE, &shown.key));
        let checkout = params.checkout.clone().or_else(|| shown.checkout.clone());
        match source_of(shared, &params.board, &root, name) {
            Ok(source) => {
                let got = source
                    .get(&GetParams {
                        id: short.to_string(),
                        checkout,
                    })
                    .map_err(source_failed)?;
                if let Some(task) = got.task {
                    result.task = Some(task);
                }
            }
            Err(SourceError::NoSource(_)) => {}
            Err(error) => return Err(source_failed(error)),
        }
    } else if result.ambiguous.is_empty() {
        let source = prefix.unwrap_or(scratch::SOURCE);
        result.task = match source_of(shared, &params.board, &root, source) {
            Ok(source) => {
                source
                    .get(&GetParams {
                        id: id.to_string(),
                        checkout: params.checkout.clone(),
                    })
                    .map_err(source_failed)?
                    .task
            }
            Err(SourceError::NoSource(_)) => None,
            Err(error) => return Err(source_failed(error)),
        };
    }
    if let Some(task) = &mut result.task {
        // Core's count, as `task.list` gives it: whatever the source sent is replaced.
        let counts = corpus_of(shared, &params.board, &root)?.read(|corpus| corpus.unblocks());
        task.unblocks = counts.get(&task.key).copied();
    }
    if let Some(task) = &result.task {
        let mut keys = vec![task.key.clone()];
        keys.extend(former);
        let rows = runs_of(shared, &params.board, &keys)?;
        result.latest_run = rows.iter().find_map(latest_of);
        result.runs = rows.into_iter().map(|row| row.run).collect();
        result.ops = ops_of(shared, &params.board, &root, task);
        let key = task.key.clone();
        result.views = views_holding(shared, &params.board, &root, &key)?;
    }
    Ok(serde_json::to_value(result).expect("a result is JSON"))
}

/// The names of the board's views that hold the task `key`, in the order `task.views` lists them. Each view is
/// evaluated as `task.list` evaluates it, so it has the same budgets and the same cache; a view that failed to load or
/// to run holds nothing. A line that does not load has no views to name, and does not fail the task's own answer.
fn views_holding(
    shared: &Shared,
    board: &str,
    root: &str,
    key: &str,
) -> Result<Vec<String>, RpcError> {
    let service = corpus_of(shared, board, root)?;
    let Ok(BoardViews { loaded, .. }) = loaded_views(shared, board, root, &service) else {
        return Ok(Vec::new());
    };
    Ok(loaded
        .iter()
        .filter(|one| {
            viewed(shared, board, &service, one)
                .is_ok_and(|result| result.members.iter().any(|member| member == key))
        })
        .map(|one| one.view.name.clone())
        .collect())
}

/// The ops the task's source declares for its kind, or none when the source cannot say (a source that is gone is the
/// write's own error to give).
fn ops_of(
    shared: &Shared,
    board: &str,
    root: &str,
    task: &collide_core::provider::Task,
) -> Vec<collide_core::item::OpDecl> {
    let name = task
        .key
        .split_once(':')
        .map_or(scratch::SOURCE, |(name, _)| name);
    match source_of(shared, board, root, name).and_then(|source| source.declared()) {
        Ok(init) => crate::task_args::declared_ops(&init, task.kind.as_deref()),
        Err(_) => Vec::new(),
    }
}

fn empty_get() -> TaskGetResult {
    TaskGetResult {
        task: None,
        ambiguous: Vec::new(),
        copies: Vec::new(),
        collisions: Vec::new(),
        runs: Vec::new(),
        latest_run: None,
        links: Vec::new(),
        ops: Vec::new(),
        link_types: Vec::new(),
        views: Vec::new(),
    }
}

/// The runs of a task under any of `keys`, newest first.
fn runs_of(
    shared: &Shared,
    board: &str,
    keys: &[String],
) -> Result<Vec<crate::run_store::RunRow>, RpcError> {
    let mut rows = Vec::new();
    for key in keys {
        rows.extend(
            shared
                .store
                .runs(&RunFilter {
                    board: Some(board),
                    task: Some(key),
                    scope: RunScope::All,
                    before: None,
                    limit: RUN_PAGE_MAX,
                })
                .map_err(store_failed)?,
        );
    }
    rows.sort_by_key(|row| std::cmp::Reverse(row.opened_seq));
    rows.dedup_by(|a, b| a.run == b.run);
    Ok(rows)
}

/// A run that reached a final state, as the task's `latest_run` (0.5, `latestRun`): its outcome, with no state.
fn latest_of(row: &crate::run_store::RunRow) -> Option<collide_core::change::LatestRun> {
    use collide_core::state::RunState;
    let outcome = match &row.state {
        RunState::Ended { outcome } => outcome.as_str().to_string(),
        RunState::Failed {} => "failed".to_string(),
        RunState::Stopped {} => "stopped".to_string(),
        RunState::Running {} | RunState::Paused { .. } | RunState::Integrating {} => return None,
    };
    Some(collide_core::change::LatestRun {
        id: row.run.clone(),
        workflow: row.workflow.clone(),
        outcome,
        state: Default::default(),
    })
}

/// The source a write or a preview names, and the call it makes: `source` or the id's own prefix (which must agree),
/// and the id without it.
fn named_source(source: Option<&str>, apply: &mut ApplyParams) -> Result<String, RpcError> {
    let (from_id, bare_id) = match apply.id.as_deref().map(scratch::split_id) {
        Some((source, bare)) => (source, Some(bare.to_string())),
        None => (None, None),
    };
    let named = match (source, from_id) {
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
    let named = named.to_string();
    apply.id = bare_id;
    Ok(named)
}

/// `task.apply` on any source (ts-0075-hj, ts-0113-ck): the source's own `ok`, `conflict` or `refused` is the answer. A
/// source the line does not have, an untrusted line or a source that is gone is an error. With no `checkout` a
/// ref-scoped source writes the line's root, which the provider was told at `initialize`. `commit` is refused here:
/// a scratch task is never committed, and the CLI commits a write it made through the daemon, by its own `--commit`.
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
    let root = board_root(shared, &params.board)?;
    let mut apply = params.apply;
    // Who asked is the connection's, never the request's (0.10): whatever a client sent is dropped.
    apply.actor = None;
    let name = named_source(params.source.as_deref(), &mut apply)?;
    if params.commit {
        return Err(RpcError::new(
            code::INVALID_PARAMS,
            if name == scratch::SOURCE {
                "a scratch task is never committed: it lives in Collide's store, not in the repository".to_string()
            } else {
                "the daemon does not commit a write: `collide task <key> <op> --commit` commits the files it reported"
                    .to_string()
            },
        ));
    }
    let source = source_of(shared, &params.board, &root, &name).map_err(source_failed)?;
    let answer = judged_write(shared, session, &params.board, &root, &source, apply)?;
    Ok(serde_json::to_value(answer).expect("a result is JSON"))
}

/// The write `apply` asks of `source`, as the connection may make it: applied, queued while its checkout is busy, or
/// kept as a confirmation for a person ([`judge_write`]). Every method that writes an item goes through here.
fn judged_write(
    shared: &Shared,
    session: &Session,
    board: &str,
    root: &str,
    source: &Source,
    mut apply: ApplyParams,
) -> Result<TaskApplyResult, RpcError> {
    shape_run_agent_create(shared, session, source, &mut apply);
    let (mut result, mut queued, mut confirmation) = (None, None, None);
    match judge_write(shared, session, board, root, source, &apply) {
        Verdict::Allow => match write(shared, session.actor(), board, root, source, &apply)? {
            Written::Applied(done) => result = Some(*done),
            Written::Queued(waiting) => queued = Some(waiting),
        },
        Verdict::Refuse(error) => return Err(error),
        Verdict::Confirm(asked) => {
            let write = QueuedWrite {
                board: board.to_string(),
                source: source.name().to_string(),
                apply: apply.clone(),
                checkout: write_guard::canonical(Path::new(
                    apply.checkout.as_deref().unwrap_or(root),
                )),
                actor: Caller::App.actor(),
            };
            confirmation = Some(
                shared
                    .guard
                    .confirm(&write, &asked)
                    .map_err(|message| RpcError::new(code::INTERNAL_ERROR, message))?,
            );
        }
    }
    Ok(TaskApplyResult {
        result,
        queued,
        confirmation,
        commit: None,
    })
}

/// The create rule for a run's agent (ts-0139-wq; 0.10, Follow-ups): a `create` on the planning source by the agent of
/// a run is made private, from the main checkout, in the epic of the run's task, with `raised-by` to that task and the
/// run id in `run` ([`shape_create`]). The run's task is the first task the run's launch recorded, when it is an item
/// of the planning source. A run that has none, or whose task is not found, gets only the run id. Nothing else is
/// shaped: a person's create goes where it asks.
fn shape_run_agent_create(
    shared: &Shared,
    session: &Session,
    source: &Source,
    apply: &mut ApplyParams,
) {
    let Caller::RunAgent { run } = session.caller() else {
        return;
    };
    if apply.op != "create" || source.name() != PLANNING_SOURCE {
        return;
    }
    let task = run_task_of(shared, source, run);
    shape_create(apply, run, task.as_ref());
}

/// The planning-source task the run `run` works on, with its epic, or `None`.
fn run_task_of(shared: &Shared, source: &Source, run: &str) -> Option<RunTask> {
    let row = shared.store.run(run).ok().flatten()?;
    let key = &row.tasks.first()?.key;
    let (name, id) = key.split_once(':')?;
    if name != PLANNING_SOURCE {
        return None;
    }
    let found = source
        .get(&GetParams {
            id: id.to_string(),
            checkout: None,
        })
        .ok()?
        .task?;
    Some(RunTask {
        id: found.id.clone(),
        epic: found.epic().map(str::to_string),
    })
}

/// `task.create` (ts-0136-rr): the planning source's `create`, with the title, kind and epic the request names. It is a
/// write like `task.apply`'s: queued while the checkout is busy, and refused or answered by the provider in its own
/// words.
fn task_create(
    shared: &Shared,
    session: &Session,
    params: &serde_json::Value,
) -> Result<serde_json::Value, RpcError> {
    let params: TaskCreateParams = read_params("task.create", params)?;
    if shared.stopping() {
        return Err(RpcError::new(
            code::UNAVAILABLE,
            "the daemon is shutting down",
        ));
    }
    let root = board_root(shared, &params.board)?;
    let mut args = BTreeMap::from([("title".to_string(), serde_json::json!(params.title))]);
    for (name, value) in [
        ("kind", params.kind.map(serde_json::Value::from)),
        ("epic", params.epic.map(serde_json::Value::from)),
        ("seed", params.seed.map(serde_json::Value::from)),
        ("root", params.root.map(serde_json::Value::from)),
    ] {
        if let Some(value) = value {
            args.insert(name.to_string(), value);
        }
    }
    let source = source_of(shared, &params.board, &root, PLANNING_SOURCE).map_err(source_failed)?;
    let declared = source.declared().ok().and_then(|init| {
        crate::task_args::declared_ops(&init, None)
            .into_iter()
            .find(|op| op.name == "create")
    });
    crate::task_args::admit_create_args(declared.as_ref(), &mut args, params.args).map_err(
        |refused| {
            RpcError::new(code::INVALID_PARAMS, refused.message)
                .with_data(serde_json::json!({ "bad_argument": refused.name }))
        },
    )?;
    let apply = ApplyParams {
        actor: None,
        op: "create".to_string(),
        id: None,
        args,
        version: None,
        checkout: params.checkout,
    };
    let answer = judged_write(shared, session, &params.board, &root, &source, apply)?;
    Ok(serde_json::to_value(answer).expect("a result is JSON"))
}

/// `task.reid` (ts-0136-rr): the planning source's `reid` of the item `id` names (a key, or an id of the planning
/// source), with no version: the provider reads every file it rewrites under its lock. The renamed item is the
/// answer's `task`; `rewritten` names the other items it changed. Nothing is committed.
fn task_reid(
    shared: &Shared,
    session: &Session,
    params: &serde_json::Value,
) -> Result<serde_json::Value, RpcError> {
    let params: TaskReidParams = read_params("task.reid", params)?;
    if shared.stopping() {
        return Err(RpcError::new(
            code::UNAVAILABLE,
            "the daemon is shutting down",
        ));
    }
    let root = board_root(shared, &params.board)?;
    let (named, bare) = scratch::split_id(&params.id);
    let name = named.unwrap_or(PLANNING_SOURCE);
    let from = format!("{name}:{bare}");
    let args = params
        .seed
        .map(|seed| BTreeMap::from([("seed".to_string(), serde_json::json!(seed))]))
        .unwrap_or_default();
    let apply = ApplyParams {
        actor: None,
        op: "reid".to_string(),
        id: Some(bare.to_string()),
        args,
        version: None,
        checkout: params.checkout,
    };
    let source = source_of(shared, &params.board, &root, name).map_err(source_failed)?;
    let answer = judged_write(shared, session, &params.board, &root, &source, apply)?;
    let rewritten = match &answer.result {
        Some(ApplyResult::Ok { task, items, .. }) => items
            .iter()
            .map(|effect| effect.key.clone())
            .filter(|key| *key != task.key)
            .collect(),
        _ => Vec::new(),
    };
    Ok(serde_json::to_value(TaskReidResult {
        from,
        result: answer.result,
        queued: answer.queued,
        rewritten,
    })
    .expect("a result is JSON"))
}

/// `task.sections` (ts-0136-rr): the sections in use over the features of the proposals the board's corpus holds that
/// are not dropped (not rejected or superseded), and the near-duplicate groups among them ([`section_uses`]). A
/// proposal is the corpus's shown copy of its key, so a proposal that exists in several checkouts counts once.
fn task_sections(
    shared: &Shared,
    params: &serde_json::Value,
) -> Result<serde_json::Value, RpcError> {
    let params: TaskSectionsParams = read_params("task.sections", params)?;
    let root = board_root(shared, &params.board)?;
    let service = corpus_of(shared, &params.board, &root)?;
    let sections = service.read(|corpus| {
        let rows = corpus.rows();
        let proposals: Vec<ProposalFeatures<'_>> = rows
            .iter()
            .filter(|row| {
                let source = row
                    .task
                    .key
                    .split_once(':')
                    .map_or("", |(source, _)| source);
                corpus.kind_of(source, row.task) == "proposal"
                    && row.task.state.category != Category::Dropped
            })
            .map(|row| ProposalFeatures {
                proposal: &row.task.id,
                features: &row.task.features,
            })
            .collect();
        section_uses(&proposals)
    });
    Ok(serde_json::to_value(sections).expect("a result is JSON"))
}

/// `task.changes` (ts-0188-kb): for each run of the task, the files from where its first phase began to where its last
/// one ended (to the working tree while a phase is active), then the commits on the board's line that name the task,
/// outside every run's span. A task of the scratch source has no commits to find: its ids are short numbers.
fn task_changes(
    shared: &Shared,
    params: &serde_json::Value,
) -> Result<serde_json::Value, RpcError> {
    let params: TaskChangesParams = read_params("task.changes", params)?;
    let root = board_root(shared, &params.board)?;
    let root = Path::new(&root);
    let rows = task_runs(shared, &params.board, root, &params.task)?;
    let git = Git::with_env(shared.git_env.clone());
    let mut runs = Vec::new();
    let mut in_runs = HashSet::new();
    for row in &rows {
        let heads = shared.store.phase_heads(&row.run).map_err(store_failed)?;
        let workspace = shared.file_runs.workspace_path(&row.run);
        let input = task_changes::RunInput {
            run: &row.run,
            state: &row.state,
            heads: &heads,
            workspace: workspace.as_deref(),
            place: ChangesPlace::Worktree,
        };
        if let Some(found) = task_changes::run_span(&input, root) {
            in_runs.extend(commits_of_span(shared, &git, &found, &row.run));
        }
        runs.push(task_changes::changes_of_run(&git, root, &input));
    }
    let (commits, commits_words) =
        task_commits(shared, &git, root, &params.task, &in_runs, runs.len())?;
    let result = TaskChangesResult {
        runs,
        commits,
        commits_words,
    };
    Ok(serde_json::to_value(result).expect("a result is JSON"))
}

/// The commits a run's span holds. A span git cannot read is logged and holds none: the list of commits is then a
/// little longer, which is the better mistake.
fn commits_of_span(
    shared: &Shared,
    git: &Git,
    found: &task_changes::RunSpan,
    run: &str,
) -> HashSet<String> {
    let (base, head) = match &found.span {
        task_changes::Span::Between { base, head } => (base, head.as_str()),
        task_changes::Span::WorkingTree { base } => (base, "HEAD"),
        task_changes::Span::Commit { .. } => return HashSet::new(),
    };
    task_changes::commits_in(git, &found.cwd, base, head).unwrap_or_else(|error| {
        shared
            .log
            .line(&format!("run {run}: its commits were not listed: {error}"));
        HashSet::new()
    })
}

/// The commits that name `task` on the board's line, outside `in_runs`, and the words that say where they are.
fn task_commits(
    shared: &Shared,
    git: &Git,
    root: &Path,
    task: &str,
    in_runs: &HashSet<String>,
    runs: usize,
) -> Result<(Vec<CommitChanges>, ChangeWords), RpcError> {
    let (source, id) = task.split_once(':').unwrap_or((scratch::SOURCE, task));
    let line = boards::checkout_of(root, &shared.git_env)
        .map(|checkout| checkout.line_branch().to_string())
        .ok();
    let (Some(line), true) = (line, source != scratch::SOURCE) else {
        let line = "none";
        return Ok((Vec::new(), task_changes::commit_words(line, id, &[], runs)));
    };
    let found = task_changes::naming(git, root, &line, id).map_err(diff_failed)?;
    let found: Vec<_> = found
        .into_iter()
        .filter(|commit| !in_runs.contains(&commit.id))
        .collect();
    let mut commits = Vec::new();
    for commit in &found {
        let span = task_changes::Span::Commit {
            id: commit.id.clone(),
        };
        let files = task_changes::list(git, root, &span).map_err(diff_failed)?;
        commits.push(CommitChanges {
            id: commit.id.clone(),
            subject: commit.subject.clone(),
            date_ms: commit.date_ms,
            files: files.files,
            collapsed: files.collapsed,
            more: files.more,
        });
    }
    let words = task_changes::commit_words(&line, id, &found, runs);
    Ok((commits, words))
}

fn diff_failed(error: DiffError) -> RpcError {
    match error {
        DiffError::NoChange(_) | DiffError::BranchGone(_) => {
            RpcError::new(code::NOT_FOUND, error.to_string())
        }
        DiffError::Git(message) => RpcError::new(code::INTERNAL_ERROR, message),
    }
}

/// The runs of `task` under its key and the keys it had before, newest first.
fn task_runs(
    shared: &Shared,
    board: &str,
    root: &Path,
    task: &str,
) -> Result<Vec<RunRow>, RpcError> {
    let mut keys = vec![task.to_string()];
    if task.contains(':')
        && let Ok(corpus) = corpus_of(shared, board, &root.to_string_lossy())
    {
        keys.extend(corpus.read(|corpus| corpus.former_keys(task)));
    }
    runs_of(shared, board, &keys)
}

/// `task.diff` (ts-0188-kb): one file of a run's changes, or of a commit's, as git's patch or as the whole file after
/// the changes with each line numbered and marked. The patch is cut at the cap `run.diff`'s is.
fn task_diff(shared: &Shared, params: &serde_json::Value) -> Result<serde_json::Value, RpcError> {
    let params: TaskDiffParams = read_params("task.diff", params)?;
    let root = board_root(shared, &params.board)?;
    let root = Path::new(&root);
    let (span, cwd) = match (&params.run, &params.commit) {
        (Some(run), None) => {
            let rows = task_runs(shared, &params.board, root, &params.task)?;
            let row = rows.iter().find(|row| &row.run == run).ok_or_else(|| {
                RpcError::new(
                    code::NOT_FOUND,
                    format!("{} has no run {run:?}", params.task),
                )
            })?;
            let heads = shared.store.phase_heads(run).map_err(store_failed)?;
            let workspace = shared.file_runs.workspace_path(run);
            let input = task_changes::RunInput {
                run,
                state: &row.state,
                heads: &heads,
                workspace: workspace.as_deref(),
                place: ChangesPlace::Worktree,
            };
            let found = task_changes::run_span(&input, root)
                .ok_or_else(|| RpcError::new(code::NOT_FOUND, task_changes::NO_CHANGES_RECORDED))?;
            (found.span, found.cwd)
        }
        (None, Some(commit)) => {
            if commit.is_empty() || !commit.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(RpcError::new(
                    code::INVALID_PARAMS,
                    format!("{commit:?} is not a commit id"),
                ));
            }
            (
                task_changes::Span::Commit { id: commit.clone() },
                root.to_path_buf(),
            )
        }
        _ => {
            return Err(RpcError::new(
                code::INVALID_PARAMS,
                "task.diff takes a run or a commit, one of the two",
            ));
        }
    };
    let git = Git::with_env(shared.git_env.clone());
    let result = if params.whole {
        let (whole, note) =
            task_changes::whole(&git, &cwd, &span, &params.path).map_err(diff_failed)?;
        TaskDiffResult {
            path: params.path,
            patch: None,
            whole: Some(whole),
            note,
        }
    } else {
        let patch = task_changes::patch(&git, &cwd, &span, &params.path).map_err(diff_failed)?;
        let note = patch.truncated.then(|| {
            format!(
                "the patch is {} bytes and is cut at {}",
                patch.bytes,
                crate::run_diff::MAX_PATCH_BYTES
            )
        });
        TaskDiffResult {
            path: params.path,
            patch: Some(patch),
            whole: None,
            note,
        }
    };
    Ok(serde_json::to_value(result).expect("a result is JSON"))
}

/// Whether the write is one only a person makes (0.10, Planning sources), and what the connection may do about it.
/// The item's kind and status are the corpus's; an item it does not know yet is asked of the source, so a write is not
/// let through for the corpus being late. The kind is judged by its name (`proposal`, `question`). A write that names
/// no item (a create), a scratch task and every other kind are allowed to anyone.
fn judge_write(
    shared: &Shared,
    session: &Session,
    board: &str,
    root: &str,
    source: &Source,
    apply: &ApplyParams,
) -> Verdict {
    let Some(id) = apply.id.as_deref() else {
        return Verdict::Allow;
    };
    if source.name() == scratch::SOURCE {
        return Verdict::Allow;
    }
    // `move` is Share's op, and Keep's for one item: the app and the CLI may, and a run's agent may not, on any source,
    // since any provider may now declare it.
    if let (Caller::RunAgent { run }, "move") = (session.caller(), apply.op.as_str()) {
        return Verdict::Refuse(
            RpcError::new(
                code::NOT_PERMITTED,
                format!(
                    "run {run} may not keep {id}: only a person does that, in the app or the CLI"
                ),
            )
            .with_data(serde_json::json!({"run": run, "op": "move", "item": id})),
        );
    }
    // The app may do all of it, and most ops are never person-only: neither asks the corpus or the provider anything.
    let to_status = apply.args.get("status").and_then(serde_json::Value::as_str);
    let could_be =
        auth::person_only_request(&apply.op, "proposal", "", Some("deferred"), to_status);
    if matches!(session.caller(), Caller::App) || could_be.is_none() {
        return Verdict::Allow;
    }
    let key = format!("{}:{id}", source.name());
    let known = corpus_of(shared, board, root)
        .ok()
        .and_then(|corpus| {
            corpus.read(|corpus| {
                let task = corpus.get(&key).task?;
                Some((corpus.kind_of(source.name(), &task), task.state.status))
            })
        })
        .or_else(|| {
            let got = source
                .get(&GetParams {
                    id: id.to_string(),
                    checkout: apply.checkout.clone(),
                })
                .ok()?;
            let task = got.task?;
            Some((task.kind.unwrap_or_default(), task.state.status))
        });
    let Some((kind, status)) = known else {
        return Verdict::Allow;
    };
    match auth::person_only_request(&apply.op, &kind, &key, Some(&status), to_status) {
        Some(request) => session.judge(&request),
        None => Verdict::Allow,
    }
}

/// What [`write`] did with a write.
enum Written {
    Applied(Box<ApplyResult>),
    /// Stored, because the checkout is busy. The Queue item says why.
    Queued(WriteQueued),
}

/// The one function every source's write goes through (ts-0113-ck, ts-0114-hr). A scratch write is applied. A write
/// into a checkout holds the checkout's line lock, first applies the writes queued before it, and is queued itself
/// when the checkout is busy (a rebase, merge, cherry-pick, revert or bisect in progress, or a live phase): the
/// answer is `queued`, and the write is applied, in order, once the checkout is free. A checkout that is not a folder
/// is left to the provider to refuse in its own words.
fn write(
    shared: &Shared,
    actor: Actor,
    board: &str,
    root: &str,
    source: &Source,
    apply: &ApplyParams,
) -> Result<Written, RpcError> {
    if !source.writes_checkout() {
        let result =
            apply_and_record(shared, board, actor, source, apply).map_err(source_failed)?;
        return Ok(Written::Applied(Box::new(result)));
    }
    let checkout = write_guard::canonical(Path::new(apply.checkout.as_deref().unwrap_or(root)));
    if !checkout.is_dir() {
        let result =
            apply_and_record(shared, board, actor, source, apply).map_err(source_failed)?;
        return Ok(Written::Applied(Box::new(result)));
    }
    shared.guard.hold(&checkout, || {
        drain_checkout(shared, &checkout);
        match shared.guard.blocked(&checkout) {
            Err(message) => Err(RpcError::new(code::INTERNAL_ERROR, message)),
            Ok(Some(reason)) => {
                let write = QueuedWrite {
                    board: board.to_string(),
                    source: source.name().to_string(),
                    apply: apply.clone(),
                    checkout: checkout.clone(),
                    actor,
                };
                shared
                    .guard
                    .enqueue(&write, &reason)
                    .map(Written::Queued)
                    .map_err(|message| RpcError::new(code::INTERNAL_ERROR, message))
            }
            Ok(None) => apply_and_record(shared, board, actor, source, apply)
                .map(|result| Written::Applied(Box::new(result)))
                .map_err(source_failed),
        }
    })
}

/// `apply` with the actor the provider is told of (0.10, Planning sources): the connection's, and today's date in the
/// daemon's zone. A workflow is not a person, so it is told as an agent.
fn told_who_asked(shared: &Shared, actor: &Actor, apply: &ApplyParams) -> ApplyParams {
    let wall = shared.clocks.read(Clock::Wall).nanos;
    let offset = shared
        .utc_offset_secs
        .unwrap_or_else(|| local_utc_offset(wall));
    let mut told = apply.clone();
    told.actor = Some(ApplyActor {
        kind: match actor.kind {
            ActorKind::Person => ApplyActorKind::Person,
            ActorKind::Workflow | ActorKind::Agent => ApplyActorKind::Agent,
        },
        at: civil_date(wall, offset),
        run: actor.run.clone(),
    });
    told
}

/// The source's `apply`, then one `task.changed` event for each item the write changed, with `actor`. The board's
/// corpus is told of the write under its gate, so the watcher's notice of the same write is dropped and the write is
/// one event ([`crate::corpus_service`]).
fn apply_and_record(
    shared: &Shared,
    board: &str,
    actor: Actor,
    source: &Source,
    apply: &ApplyParams,
) -> Result<ApplyResult, SourceError> {
    let corpus = shared.corpora.get().and_then(|corpora| corpora.get(board));
    let _gate = corpus.as_ref().map(|corpus| corpus.gate());
    let result = source.apply(&told_who_asked(shared, &actor, apply))?;
    if let ApplyResult::Ok {
        task, items, files, ..
    } = &result
    {
        for effect in items {
            let event = changed_event(
                source.name(),
                effect,
                source.write_cause(),
                task.checkout.clone(),
            );
            if let Err(error) = shared.phases.append_event(actor.clone(), event) {
                shared.log.line(&format!(
                    "the change of {} was written but its event was not: {error}",
                    effect.key
                ));
            }
        }
        if let Some(corpus) = &corpus {
            corpus.note_write(source.name(), task, items, files, apply.checkout.as_deref());
        }
    }
    Ok(result)
}

/// `task.preview` (ts-0113-ck): what `task.apply` would do with the same params, with nothing written and no event. A
/// source that does not declare `preview` is refused, naming it. The provider is told the connection's actor, as for
/// `apply`, never the one the request carries.
fn task_preview(
    shared: &Shared,
    session: &Session,
    params: &serde_json::Value,
) -> Result<serde_json::Value, RpcError> {
    let params: TaskPreviewParams = read_params("task.preview", params)?;
    if shared.stopping() {
        return Err(RpcError::new(
            code::UNAVAILABLE,
            "the daemon is shutting down",
        ));
    }
    let root = board_root(shared, &params.board)?;
    let mut apply = params.apply;
    let name = named_source(params.source.as_deref(), &mut apply)?;
    let source = source_of(shared, &params.board, &root, &name).map_err(source_failed)?;
    let result = source
        .preview(&told_who_asked(shared, &session.actor(), &apply))
        .map_err(source_failed)?;
    Ok(serde_json::to_value(result).expect("a result is JSON"))
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
    fn a_home_that_is_unset_empty_or_a_folder_passes_the_home_check() {
        assert!(check_home(None).is_ok());
        assert!(check_home(Some("")).is_ok());
        assert!(check_home(Some(std::env::temp_dir().to_str().unwrap())).is_ok());
    }

    #[test]
    fn a_home_that_names_no_folder_is_refused_with_its_path() {
        let message = check_home(Some("/no/such/collide-home")).unwrap_err();
        assert!(message.contains("/no/such/collide-home"), "{message}");
        assert!(message.contains("HOME"), "{message}");
    }

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

    /// A log whose lines a test reads back.
    #[derive(Clone, Default)]
    struct Lines(Arc<Mutex<Vec<u8>>>);

    impl Write for Lines {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// What `log_refusal` wrote for `refused`.
    fn logged_for(refused: SocketError) -> String {
        let lines = Lines::default();
        let log = DaemonLog {
            sink: Mutex::new(Box::new(lines.clone())),
            failures: AtomicU64::new(0),
            clocks: Arc::new(SystemClocks),
        };
        log_refusal(&log, &refused);
        let bytes = lines.0.lock().unwrap().clone();
        String::from_utf8(bytes).unwrap()
    }

    #[test]
    fn a_peer_that_closed_before_it_was_read_writes_no_line() {
        assert_eq!(logged_for(SocketError::PeerClosed), "");
    }

    #[test]
    fn a_foreign_uid_still_writes_its_refused_line() {
        let text = logged_for(SocketError::ForeignUid {
            uid: 502,
            daemon_uid: 501,
        });
        assert!(text.contains("a connection was refused"), "{text}");
        assert!(text.contains("502"), "{text}");
    }

    #[test]
    fn a_pid_that_could_not_be_read_for_another_reason_still_writes_its_refused_line() {
        let text = logged_for(SocketError::Io {
            context: "could not read the pid of the connecting peer (LOCAL_PEERPID)".to_string(),
            source: io::Error::from(io::ErrorKind::PermissionDenied),
        });
        assert!(text.contains("a connection was refused"), "{text}");
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
