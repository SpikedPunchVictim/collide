//! The daemon's side of the shim link (0.2, The owner is `collide-daemon`, with a per-child shim; Stage 3a;
//! `ts-0057-ck`): the phases it started or adopted, each with the shim that owns its child, the connection to that
//! shim, and the state the daemon holds for it. The messages are `core`'s ([`collide_core::shim`]), the framing and
//! the handshake are [`crate::shim_link`]'s, and the shim itself is [`crate::shim`].
//!
//! ## Threads, and what is locked
//!
//! `std` threads, no async runtime: a daemon has a handful of phases, and everything here waits on one socket or one
//! process at a time. For each phase there is one **watcher** thread, which owns the connection's reading, and for a
//! shim the daemon started one **reaper** thread blocked in `Child::wait`. A call from outside ([`Phases::write`])
//! sends on the phase's own connection and waits for the answer, so a shim that is stopped holds the callers of that
//! phase and nobody else. Three kinds of lock exist and none is held across a read or a write to a shim, except the
//! one connection's own writer lock:
//!
//! - the table of phases, held to look one up or add one;
//! - one lock for each phase's state, held to read or change it (and for the event that records the change);
//! - one lock for each connection's writer, held while one frame is written to that shim, and a second, short one for
//!   the requests waiting for an answer.
//!
//! ## A phase with an agent
//!
//! [`Phases::start_agent_phase`] gives a phase an [`crate::agent_feed::AgentFeed`]. The phase's watcher tails the
//! journal on every poll and feeds each stdout line to the session with its `wall_ns`; what the session writes goes
//! to the shim, and the journal has it before the child does. The phase settles when the shim reports the exit, which
//! the shim does only after it has drained the child's stdout, so the journal is whole and the order of the exit and
//! the end of stdout does not matter. The phase's last usage and its failure cause are stored in the phase's record.
//!
//! ## How the daemon finds the shims it left behind
//!
//! [`crate::daemon::adopt_shims`] scans `runs_dir` for `<run>/<phase>` directories that hold a `spec.json`, which is
//! what a shim is started from. For each it reads, in this order:
//!
//! 1. `exit.json`: a record means the shim's child ended ([`AdoptedState::Exited`]); a file that cannot be read as a
//!    record is [`AdoptedState::Damaged`];
//! 2. `shim.json`, the shim's own pid and start time: the shim is alive when a process with that pid started at that
//!    time and is not a zombie ([`AdoptedState::Alive`]); a shim that left no `shim.json` is alive when its socket
//!    accepts a connection;
//! 3. otherwise the shim is gone and left no record: [`AdoptedState::Lost`].
//!
//! [`Phases::adopt`] applies that, before the daemon binds its socket: the state held in the event log is replayed
//! first ([`Phases::new`]), so a phase the daemon had already settled is settled again by nothing, and a change is
//! recorded only when there is one. A live shim gets a watcher, which connects on its own thread, so a shim that is
//! stopped never delays the bind. A phase whose shim is gone with no record is `lost`, whatever the agent's stream
//! said (0.2), and its child, if it still runs, is stopped ([`Phases::stop_run`]'s order, below).
//!
//! ## States
//!
//! A phase goes `pending` when its directory is made, `running` when its shim has started, and ends by
//! [`collide_core::transition::phase_ending`]: `ended`, `failed` or `lost`. Until the stream parser exists (Stage 3b)
//! the fact "the agent's stream reached a `result`" is read as "the child exited with code 0", which is what a
//! `claude -p` that finished its work does. Each change is one `phase.transition` event in the store, written before
//! the shim is told the record is held ([`DaemonMessage::AckExit`]), so a daemon that dies between the two repeats
//! nothing: it reads the event, and the shim sends the record again to the next one.
//!
//! ## Stopping
//!
//! [`Phases::stop_run`] stops a run: it marks each phase of the run that is not final, finds the tree (each phase's
//! child by `child.json`, and the processes carrying `COLLIDE_RUN=<run>`), and stops it in the order of
//! [`crate::stop::ORDER`]. A marked phase whose exit arrives is `stopped`, not `ended` or `failed`. The shim is never
//! part of the tree: it carries `COLLIDE_RUNTIME=1` from the moment the daemon starts it, so it outlives the stop,
//! reaps its child and writes `exit.json`.
//!
//! The child of a phase that turns `lost` goes through the same order, on a thread of its own, with the grace of
//! [`PhaseOptions::lost_grace`]: a lost phase's child used to get an immediate SIGKILL, which gave a child that traps
//! SIGTERM no chance to clean up and reached nothing below it. The roots are the child by `child.json`; the marker scan
//! is added only when no other phase of the run is live, because it would stop that phase too. A double-forked
//! orphan of a lost phase whose run has another live phase is therefore not found (it has no marker the scan may
//! use), which is the price of not stopping the live phase.
//!
//! ## An unresponsive shim
//!
//! A shim is reported unresponsive ([`Health::Unresponsive`]) when its process is stopped
//! ([`crate::sysproc::is_stopped`]) for [`PhaseOptions::unresponsive_after`], when it does not say `Hello` in that
//! time, or when a request has had no answer for [`PhaseOptions::reply_wait`]. It is reported responsive again when
//! none of those holds. The protocol has no ping, and none is needed: the process's own status says what silence
//! cannot. Unresponsive is not lost: the phase stays `running`.
//!
//! | tag | function | what it does |
//! |---|---|---|
//! | `daemon: adopt shims` | [`crate::daemon::adopt_shims`], [`Phases::adopt`] | scan, classify, reattach, settle, before the bind |

use crate::agent_feed::AgentFeed;
use crate::auth::check_socket_path;
use crate::clock::{Clock, Clocks};
use crate::daemon::{AdoptedShim, AdoptedState};
use crate::disk::{DiskGuard, Floor, FreeSpace};
use crate::shim::{
    ChildIdentity, ChildStdio, DaemonStart, PROBLEM_PREFIX, ShimSpec, files as shim_files,
};
use crate::shim_link::{Next, ShimConnection, connect_shim, send};
use crate::stop::{
    Cleanup, Processes, RUNTIME_MARKER, Root, StopReport, StopRequest, read_detached, stop_tree,
};
use crate::store::{PhaseRecord, Store, StoreError};
use crate::sysproc;
use collide_core::adapter::{PhaseFailure, PhaseOutcome};
use collide_core::change::{Actor, ActorKind};
use collide_core::events::{Event, EventBody};
use collide_core::non_empty::NonEmpty;
use collide_core::shim::{DaemonMessage, ExitEnd, ExitRecord, ShimMessage};
use collide_core::state::{PhaseState, RunState};
use collide_core::transition::{PhaseEnding, TransitionError, phase_ending, phase_transition};
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// How long a request to a shim may go unanswered before the shim is reported unresponsive, and before the caller is
/// told so. The plan sets none: a shim answers a write in microseconds, so this is long for a loaded machine and short
/// for a person who is waiting.
pub const REPLY_WAIT: Duration = Duration::from_secs(5);

/// How long a stopped shim, or one that says nothing, goes before it is reported unresponsive. The plan sets none.
pub const UNRESPONSIVE_AFTER: Duration = Duration::from_secs(5);

/// How long a shim that is not known to be running may have no socket before its phase is settled as gone. A shim
/// binds its socket within milliseconds of starting.
pub const BIND_WAIT: Duration = Duration::from_secs(5);

/// How long the child of a lost phase gets between SIGTERM and SIGKILL (the stop of [`crate::stop`]). The shim that
/// would pass an interrupt is gone, and nobody waits, so this is shorter than the 60 s a hook gets.
pub const LOST_GRACE: Duration = Duration::from_secs(10);

/// How long [`Phases::stop_run`] waits, after the processes are gone, for each phase to settle: the shim reports the
/// child's exit and the watcher records it.
pub const STOP_SETTLE_WAIT: Duration = Duration::from_secs(10);

/// The longest a run's or a phase's name may be, in bytes. Names make directory names, and the socket path has to fit
/// the limit of a macOS socket path.
pub const MAX_NAME_BYTES: usize = 64;

/// The waits of a [`Phases`]. [`PhaseOptions::default`] is what a running daemon uses; a test passes short ones.
#[derive(Debug, Clone, Copy)]
pub struct PhaseOptions {
    /// How long a request waits for the shim's answer ([`REPLY_WAIT`]).
    pub reply_wait: Duration,
    /// How long a stopped or silent shim goes before it is reported unresponsive ([`UNRESPONSIVE_AFTER`]).
    pub unresponsive_after: Duration,
    /// How long a shim not known to be running may have no socket ([`BIND_WAIT`]).
    pub bind_wait: Duration,
    /// How often a watcher looks at its shim and at whether the daemon is stopping.
    pub poll: Duration,
    /// The grace of the stop of a lost phase's child ([`LOST_GRACE`]).
    pub lost_grace: Duration,
}

impl Default for PhaseOptions {
    fn default() -> Self {
        PhaseOptions {
            reply_wait: REPLY_WAIT,
            unresponsive_after: UNRESPONSIVE_AFTER,
            bind_wait: BIND_WAIT,
            poll: Duration::from_millis(50),
            lost_grace: LOST_GRACE,
        }
    }
}

/// One phase of one run, which names its directory under `runs_dir`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PhaseKey {
    pub run: String,
    pub phase: String,
}

impl PhaseKey {
    /// A key whose names are safe as directory names: letters, digits, `.`, `_` and `-`, not starting with a dot, at
    /// most [`MAX_NAME_BYTES`] bytes.
    pub fn new(run: &str, phase: &str) -> Result<PhaseKey, PhaseError> {
        check_name("run", run)?;
        check_name("phase", phase)?;
        Ok(PhaseKey {
            run: run.to_string(),
            phase: phase.to_string(),
        })
    }
}

impl fmt::Display for PhaseKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.run, self.phase)
    }
}

fn check_name(what: &'static str, name: &str) -> Result<(), PhaseError> {
    let reason = if name.is_empty() {
        Some("it is empty")
    } else if name.len() > MAX_NAME_BYTES {
        Some("it is too long")
    } else if name.starts_with('.') {
        Some("it starts with a dot")
    } else if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    {
        Some("it has a character other than letters, digits, `.`, `_` and `-`")
    } else {
        None
    };
    match reason {
        None => Ok(()),
        Some(reason) => Err(PhaseError::BadName {
            what,
            name: name.to_string(),
            reason,
        }),
    }
}

/// What starts a phase's child: the command, its complete environment, its directory and how it is connected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseSpec {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub env: std::collections::BTreeMap<String, String>,
    pub cwd: PathBuf,
    pub stdio: ChildStdio,
}

/// A phase that reached a final state in this daemon, as the end hook is told.
#[derive(Debug, Clone, PartialEq)]
pub struct PhaseEnd {
    pub key: PhaseKey,
    /// `ended`, `failed` or `lost`.
    pub state: PhaseState,
    /// The shim's record. `None` for a phase that is `lost` or that never started.
    pub exit: Option<ExitRecord>,
}

/// Called when a phase reaches a final state, with no lock held, so it may start the next phase. It runs on the
/// thread that noticed: a watcher, or the thread that adopts shims at start.
pub type PhaseEndHook = Box<dyn Fn(&Phases, &PhaseEnd) + Send + Sync>;

/// Whether a phase's shim answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Health {
    Responsive,
    /// `since_ms` is the wall clock, in milliseconds since the epoch, when it was first noticed.
    Unresponsive {
        since_ms: u64,
    },
}

/// Why a request to a phase's shim failed.
#[derive(Debug)]
pub enum LinkError {
    /// There is no connection to the phase's shim: the phase is unknown, final, or not connected yet.
    NoLink(PhaseKey),
    /// The shim did not answer within [`PhaseOptions::reply_wait`]. The request may still be carried out when it
    /// does.
    Unresponsive,
    /// The shim answered that it could not do it.
    Shim(String),
    Io(io::Error),
}

impl fmt::Display for LinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LinkError::NoLink(key) => write!(f, "phase {key} has no connection to its shim"),
            LinkError::Unresponsive => write!(f, "the shim did not answer in time"),
            LinkError::Shim(message) => write!(f, "the shim could not do it: {message}"),
            LinkError::Io(error) => write!(f, "writing to the shim: {error}"),
        }
    }
}

impl std::error::Error for LinkError {}

/// Why a phase could not be started or recorded.
#[derive(Debug)]
pub enum PhaseError {
    BadName {
        what: &'static str,
        name: String,
        reason: &'static str,
    },
    /// The phase's name is taken: it was started, or adopted, before.
    Exists(PhaseKey),
    /// The socket's path is over the limit, so no phase is started: the message names the path and the limit.
    SocketPath(String),
    /// The daemon was not given the `collide` binary that runs a shim.
    NoShimExe,
    Io {
        context: String,
        source: io::Error,
    },
    Store(StoreError),
    Transition(TransitionError),
}

impl fmt::Display for PhaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PhaseError::BadName { what, name, reason } => {
                write!(f, "the {what} name {name:?} is not usable: {reason}")
            }
            PhaseError::Exists(key) => write!(f, "the phase {key} is already there"),
            PhaseError::SocketPath(message) => write!(f, "{message}"),
            PhaseError::NoShimExe => write!(
                f,
                "this daemon was not given the collide binary that runs a shim, so it cannot start a phase"
            ),
            PhaseError::Io { context, source } => write!(f, "{context}: {source}"),
            PhaseError::Store(error) => write!(f, "recording the phase: {error}"),
            PhaseError::Transition(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for PhaseError {}

/// Why [`Phases::stop_run`] stopped nothing.
#[derive(Debug)]
pub enum StopRunError {
    /// The run's name is not a usable one.
    Name(PhaseError),
    /// The daemon has no phase of that run.
    Unknown(String),
    /// The stop refused the request or could not read the process table.
    Refused(io::Error),
}

impl fmt::Display for StopRunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StopRunError::Name(error) => write!(f, "{error}"),
            StopRunError::Unknown(run) => write!(f, "the daemon has no run {run:?}"),
            StopRunError::Refused(error) => write!(f, "the stop did not run: {error}"),
        }
    }
}

impl std::error::Error for StopRunError {}

/// What a [`Phases`] is made of: where the runs are, how a shim is started, and how the daemon reports.
pub struct PhasesConfig {
    pub runs_dir: PathBuf,
    /// The `collide` binary that runs `collide shim <run-dir>`.
    pub shim_exe: Option<PathBuf>,
    /// What a shim needs to start the daemon again, written into each `spec.json`; and the environment the shim
    /// process itself has.
    pub daemon_start: Option<DaemonStart>,
    pub options: PhaseOptions,
    pub on_phase_end: Option<PhaseEndHook>,
    pub store: Arc<Store>,
    pub clocks: Arc<dyn Clocks + Send + Sync>,
    pub log: Arc<dyn Fn(&str) + Send + Sync>,
    /// The process table a stop reads and signals ([`crate::stop::SystemProcesses`] in a running daemon).
    pub processes: Arc<dyn Processes + Send + Sync>,
    /// The workspace implementation's `cleanup` hook, which a stop runs after SIGKILL. `None` until the hook executor
    /// exists (Stage 7).
    pub cleanup: Option<Arc<dyn Cleanup + Send + Sync>>,
    /// Where free space is read from and the floor, for a phase that ends with no `result`: on a full volume it fails
    /// with cause `disk`, not `lost` or `failed` (0.2, Durability). `None` never calls a volume full.
    pub disk: Option<(Arc<dyn FreeSpace + Send + Sync>, Floor)>,
}

/// A lock that a panicking thread poisoned is still a lock.
fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The answer to a request waiting on the connection.
enum Reply {
    Wrote(u64),
    Failed(String),
}

struct Pending {
    sent: Instant,
    reply: mpsc::Sender<Reply>,
}

/// One connection to one shim. The watcher reads; callers write under `writer`, which is held for one frame.
struct Link {
    writer: Mutex<UnixStream>,
    pending: Mutex<VecDeque<Pending>>,
}

impl Link {
    fn send(&self, message: &DaemonMessage) -> io::Result<()> {
        send(&mut locked(&self.writer), message)
    }

    fn oldest_wait(&self) -> Option<Duration> {
        locked(&self.pending).front().map(|p| p.sent.elapsed())
    }
}

/// A shim's process, as the daemon knows it.
#[derive(Debug, Clone, Copy)]
struct ShimId {
    pid: u32,
    started_us: Option<u64>,
}

struct EntryState {
    state: PhaseState,
    exit: Option<ExitRecord>,
    shim: Option<ShimId>,
    /// The status of a shim this daemon started, once its reaper has it.
    shim_exit: Option<ExitStatus>,
    /// Set by the reaper: the shim this daemon started is gone.
    shim_gone: bool,
    link: Option<Arc<Link>>,
    health: Health,
    /// The record was acknowledged to the shim.
    acknowledged: bool,
    /// A watcher runs for this phase.
    watching: bool,
    /// A stop of the run asked for this phase to end: its exit settles it `stopped` and not `ended` or `failed`.
    stop_requested: bool,
}

struct Entry {
    key: PhaseKey,
    dir: PathBuf,
    inner: Mutex<EntryState>,
    /// The agent session of an agent phase and what it has read of the journal. A phase with none (a plain command,
    /// an adopted one) settles by its exit code. Held for one read or one settle, never while a request waits.
    feed: Option<Mutex<AgentFeed>>,
}

impl Entry {
    fn socket(&self) -> PathBuf {
        self.dir
            .join(shim_files::SOCKET_DIR)
            .join(shim_files::SOCKET)
    }
}

fn is_final(state: &PhaseState) -> bool {
    matches!(
        state,
        PhaseState::Ended {}
            | PhaseState::Failed { .. }
            | PhaseState::Stopped {}
            | PhaseState::Lost {}
    )
}

fn state_name(state: &PhaseState) -> &'static str {
    match state {
        PhaseState::Pending {} => "pending",
        PhaseState::Running {} => "running",
        PhaseState::Paused { .. } => "paused",
        PhaseState::Ended {} => "ended",
        PhaseState::Failed { .. } => "failed",
        PhaseState::Stopped {} => "stopped",
        PhaseState::Lost {} => "lost",
    }
}

fn exit_text(exit: &ExitRecord) -> String {
    match exit.end {
        ExitEnd::Exited { code } => format!("exited with code {code}"),
        ExitEnd::Signaled { signal } => format!("ended by signal {signal}"),
    }
}

/// Whether the process `pid` is running, and is the one that started at `started_us` when that is known.
pub(crate) fn process_is_running(pid: u32, started_us: Option<u64>) -> bool {
    match sysproc::start_time_us(pid) {
        Some(started) => {
            started_us.is_none_or(|expected| expected == started)
                && sysproc::is_zombie(pid) != Some(true)
        }
        None => false,
    }
}

/// What every thread of the phases shares.
struct PhasesShared {
    runs_dir: PathBuf,
    shim_exe: Option<PathBuf>,
    daemon_start: Option<DaemonStart>,
    options: PhaseOptions,
    on_phase_end: Option<PhaseEndHook>,
    store: Arc<Store>,
    clocks: Arc<dyn Clocks + Send + Sync>,
    log: Arc<dyn Fn(&str) + Send + Sync>,
    processes: Arc<dyn Processes + Send + Sync>,
    cleanup: Option<Arc<dyn Cleanup + Send + Sync>>,
    disk: Option<(Arc<dyn FreeSpace + Send + Sync>, Floor)>,
    table: Mutex<HashMap<PhaseKey, Arc<Entry>>>,
    /// Held to take the next event number and write the event, so two threads never take the same one.
    events: Mutex<()>,
    watchers: Mutex<Vec<JoinHandle<()>>>,
    /// The threads that stop the children of lost phases; [`Phases::stop`] waits for them.
    stoppers: Mutex<Vec<JoinHandle<()>>>,
    stopping: AtomicBool,
}

/// The phases of a daemon. Cloning it shares them.
#[derive(Clone)]
pub struct Phases {
    shared: Arc<PhasesShared>,
}

/// The text stored as a phase's failure when it ended with no `result` on a full volume.
const DISK_FAILURE: &str = "disk";

/// What an agent phase's session settled on, and whether the volume was full when it ended with no `result`.
struct Settled {
    outcome: PhaseOutcome,
    volume_full: bool,
}

/// What a change of state did.
struct Advanced {
    changed: bool,
    /// The event is in the store.
    durable: bool,
}

impl Phases {
    /// Makes the phases of a daemon and restores what the event log says of them: the last state of each phase. The
    /// shims themselves are found by [`Phases::adopt`].
    pub fn new(config: PhasesConfig) -> Result<Phases, StoreError> {
        let PhasesConfig {
            runs_dir,
            shim_exe,
            daemon_start,
            options,
            on_phase_end,
            store,
            clocks,
            log,
            processes,
            cleanup,
            disk,
        } = config;
        let mut last: HashMap<PhaseKey, PhaseState> = HashMap::new();
        for event in store.events_since(0)? {
            if let EventBody::PhaseTransition { run, phase, to, .. } = event.body
                && let Ok(key) = PhaseKey::new(&run, &phase)
            {
                last.insert(key, to);
            }
        }
        let table = last
            .into_iter()
            .map(|(key, state)| {
                let dir = runs_dir.join(&key.run).join(&key.phase);
                let entry = Arc::new(Entry {
                    key: key.clone(),
                    dir,
                    inner: Mutex::new(EntryState::new(state)),
                    feed: None,
                });
                (key, entry)
            })
            .collect();
        Ok(Phases {
            shared: Arc::new(PhasesShared {
                runs_dir,
                shim_exe,
                daemon_start,
                options,
                on_phase_end,
                store,
                clocks,
                log,
                processes,
                cleanup,
                disk,
                table: Mutex::new(table),
                events: Mutex::new(()),
                watchers: Mutex::new(Vec::new()),
                stoppers: Mutex::new(Vec::new()),
                stopping: AtomicBool::new(false),
            }),
        })
    }

    /// Stops the watchers, which end within a poll, and waits for them, and for any stop of a lost phase's child that
    /// is running (it ends within [`PhaseOptions::lost_grace`] and the settle of [`crate::stop`]). The shims go on:
    /// they belong to no daemon.
    pub fn stop(&self) {
        self.shared.stopping.store(true, Ordering::SeqCst);
        let handles = std::mem::take(&mut *locked(&self.shared.watchers));
        for handle in handles {
            let _ = handle.join();
        }
        let stoppers = std::mem::take(&mut *locked(&self.shared.stoppers));
        for handle in stoppers {
            let _ = handle.join();
        }
    }

    fn stopping(&self) -> bool {
        self.shared.stopping.load(Ordering::SeqCst)
    }

    fn log(&self, text: &str) {
        (self.shared.log)(text);
    }

    fn now_ms(&self) -> u64 {
        self.shared.clocks.read(Clock::Wall).nanos / 1_000_000
    }

    fn entry(&self, key: &PhaseKey) -> Option<Arc<Entry>> {
        locked(&self.shared.table).get(key).cloned()
    }

    // ---- what a caller asks ----------------------------------------------------------------------------------

    /// The phase's state, or `None` for a phase the daemon does not know.
    pub fn state(&self, key: &PhaseKey) -> Option<PhaseState> {
        self.entry(key).map(|e| locked(&e.inner).state.clone())
    }

    /// Whether the phase's shim answers; `None` for an unknown phase.
    pub fn health(&self, key: &PhaseKey) -> Option<Health> {
        self.entry(key).map(|e| locked(&e.inner).health.clone())
    }

    /// The exit record the daemon holds for the phase: the shim's, once the phase ended through it.
    pub fn exit_record(&self, key: &PhaseKey) -> Option<ExitRecord> {
        self.entry(key).and_then(|e| locked(&e.inner).exit.clone())
    }

    /// The pid of the phase's shim, once known.
    pub fn shim_pid(&self, key: &PhaseKey) -> Option<u32> {
        self.entry(key)
            .and_then(|e| locked(&e.inner).shim.map(|s| s.pid))
    }

    /// How the shim ended, for a shim this daemon started and whose reaper has taken it.
    pub fn shim_exit(&self, key: &PhaseKey) -> Option<ExitStatus> {
        self.entry(key).and_then(|e| locked(&e.inner).shim_exit)
    }

    /// How many phases have not reached a final state.
    pub fn live(&self) -> usize {
        locked(&self.shared.table)
            .values()
            .filter(|e| !is_final(&locked(&e.inner).state))
            .count()
    }

    /// The phases whose shim is unresponsive, as `run/phase`, in order.
    pub fn unresponsive(&self) -> Vec<String> {
        let mut names: Vec<String> = locked(&self.shared.table)
            .values()
            .filter(|e| matches!(locked(&e.inner).health, Health::Unresponsive { .. }))
            .map(|e| e.key.to_string())
            .collect();
        names.sort();
        names
    }

    /// Writes `data` to the phase's child through its shim and returns the journal's sequence for it. Waits for the
    /// shim's answer for [`PhaseOptions::reply_wait`]; a shim that does not answer is reported unresponsive, and the
    /// request is not withdrawn: it is carried out when the shim runs again.
    pub fn write(&self, key: &PhaseKey, data: &str) -> Result<u64, LinkError> {
        let entry = self
            .entry(key)
            .ok_or_else(|| LinkError::NoLink(key.clone()))?;
        let link = self.link_of(&entry)?;
        let (reply, answer) = mpsc::channel();
        {
            // One frame at a time on this connection; other phases have their own.
            let mut writer = locked(&link.writer);
            locked(&link.pending).push_back(Pending {
                sent: Instant::now(),
                reply,
            });
            let message = DaemonMessage::Write {
                data: data.to_string(),
            };
            if let Err(error) = send(&mut writer, &message) {
                locked(&link.pending).pop_back();
                return Err(LinkError::Io(error));
            }
        }
        match answer.recv_timeout(self.shared.options.reply_wait) {
            Ok(Reply::Wrote(seq)) => Ok(seq),
            Ok(Reply::Failed(message)) => Err(LinkError::Shim(message)),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                self.mark_unresponsive(&entry);
                Err(LinkError::Unresponsive)
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(LinkError::NoLink(key.clone())),
        }
    }

    /// Closes the phase's child's stdin through its shim. The shim sends no answer to it; a failure it reports is
    /// logged.
    pub fn close_stdin(&self, key: &PhaseKey) -> Result<(), LinkError> {
        let entry = self
            .entry(key)
            .ok_or_else(|| LinkError::NoLink(key.clone()))?;
        let link = self.link_of(&entry)?;
        link.send(&DaemonMessage::CloseStdin {})
            .map_err(LinkError::Io)
    }

    /// The phase's connection. A phase that has just started, or whose daemon has just restarted, has none until its
    /// watcher has done the handshake, so this waits for it for up to [`PhaseOptions::reply_wait`] while the phase has
    /// not ended. It never waits on a shim: only on the watcher's connect, which is a thread of its own.
    fn link_of(&self, entry: &Arc<Entry>) -> Result<Arc<Link>, LinkError> {
        let until = Instant::now() + self.shared.options.reply_wait;
        loop {
            {
                let state = locked(&entry.inner);
                if let Some(link) = &state.link {
                    return Ok(link.clone());
                }
                if is_final(&state.state) {
                    return Err(LinkError::NoLink(entry.key.clone()));
                }
                if Instant::now() >= until {
                    return Err(match state.health {
                        Health::Unresponsive { .. } => LinkError::Unresponsive,
                        Health::Responsive => LinkError::NoLink(entry.key.clone()),
                    });
                }
            }
            if self.stopping() {
                return Err(LinkError::NoLink(entry.key.clone()));
            }
            thread::sleep(Duration::from_millis(5));
        }
    }

    // ---- starting a phase ------------------------------------------------------------------------------------

    /// Starts a phase: makes its directory and `spec.json`, records it `pending`, starts its shim, and records it
    /// `running`. Nothing is made for a name that is bad or taken, or whose socket path is over the limit. A shim that
    /// cannot be started makes the phase `failed`, and the error says why.
    pub fn start_phase(&self, key: &PhaseKey, spec: &PhaseSpec) -> Result<(), PhaseError> {
        self.start_phase_with(key, spec, None)
    }

    /// Starts an agent phase: [`Phases::start_phase`], with the session whose stream the phase's watcher feeds from
    /// the journal ([`AgentFeed`]) and whose outcome settles the phase. The caller sends the first turn with
    /// [`Phases::send_turn`].
    pub fn start_agent_phase(
        &self,
        key: &PhaseKey,
        spec: &PhaseSpec,
        feed: AgentFeed,
    ) -> Result<(), PhaseError> {
        self.start_phase_with(key, spec, Some(feed))
    }

    fn start_phase_with(
        &self,
        key: &PhaseKey,
        spec: &PhaseSpec,
        feed: Option<AgentFeed>,
    ) -> Result<(), PhaseError> {
        let shim_exe = self.shared.shim_exe.clone().ok_or(PhaseError::NoShimExe)?;
        let dir = self.shared.runs_dir.join(&key.run).join(&key.phase);
        let socket = dir.join(shim_files::SOCKET_DIR).join(shim_files::SOCKET);
        check_socket_path(&socket).map_err(|e| PhaseError::SocketPath(e.to_string()))?;
        let entry = {
            let mut table = locked(&self.shared.table);
            if table.contains_key(key) {
                return Err(PhaseError::Exists(key.clone()));
            }
            let entry = Arc::new(Entry {
                key: key.clone(),
                dir: dir.clone(),
                inner: Mutex::new(EntryState::new(PhaseState::Pending {})),
                feed: feed.map(Mutex::new),
            });
            table.insert(key.clone(), entry.clone());
            entry
        };
        let made = self.make_dir_and_spec(&entry, spec);
        if let Err(error) = made {
            // The name is taken by a directory that was already there, or by one that could not be made.
            self.fail_to_start(&entry, &error);
            return Err(error);
        }
        if let Err(error) = self
            .record(&entry.key, None, PhaseState::Pending {})
            .map_err(PhaseError::Store)
        {
            self.fail_to_start(&entry, &error);
            return Err(error);
        }
        match self.spawn_shim(&shim_exe, &entry) {
            Ok(()) => {}
            Err(error) => {
                self.fail_to_start(&entry, &error);
                return Err(error);
            }
        }
        match self.advance(&entry, PhaseState::Running {}, None) {
            Ok(_) => {}
            Err(error) => {
                self.log(&format!(
                    "phase {key}: could not be recorded running: {error}"
                ));
            }
        }
        self.log(&format!("phase {key}: started its shim"));
        self.watch(&entry);
        Ok(())
    }

    fn make_dir_and_spec(&self, entry: &Arc<Entry>, spec: &PhaseSpec) -> Result<(), PhaseError> {
        let io_error = |context: String| move |source| PhaseError::Io { context, source };
        let run_dir = self.shared.runs_dir.join(&entry.key.run);
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&run_dir)
            .map_err(io_error(format!("making {}", run_dir.display())))?;
        match fs::DirBuilder::new().mode(0o700).create(&entry.dir) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                return Err(PhaseError::Exists(entry.key.clone()));
            }
            Err(source) => {
                return Err(PhaseError::Io {
                    context: format!("making {}", entry.dir.display()),
                    source,
                });
            }
        }
        let shim_spec = ShimSpec {
            program: spec.program.clone(),
            args: spec.args.clone(),
            env: spec.env.clone(),
            cwd: spec.cwd.clone(),
            run_dir: entry.dir.clone(),
            stdio: spec.stdio,
            daemon: self.shared.daemon_start.clone(),
        };
        let path = entry.dir.join(shim_files::SPEC);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .map_err(io_error(format!("making {}", path.display())))?;
        file.write_all(shim_spec.to_json().to_string().as_bytes())
            .and_then(|()| file.sync_all())
            .map_err(io_error(format!("writing {}", path.display())))
    }

    /// Starts `collide shim <dir>` in a session of its own, so that neither a signal to the daemon's group nor a job
    /// that ends with the daemon reaches it, with a cleared environment (the spec's `daemon.env`, if there is one) and
    /// its output in `shim.log`. A reaper thread takes its exit.
    fn spawn_shim(&self, exe: &Path, entry: &Arc<Entry>) -> Result<(), PhaseError> {
        let io_error = |context: String| move |source| PhaseError::Io { context, source };
        let log_path = entry.dir.join(shim_files::SHIM_LOG);
        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o600)
            .open(&log_path)
            .map_err(io_error(format!("making {}", log_path.display())))?;
        let log_for_errors = log
            .try_clone()
            .map_err(io_error(format!("making {}", log_path.display())))?;
        let mut command = Command::new(exe);
        command
            .arg("shim")
            .arg(&entry.dir)
            .env_clear()
            .stdin(Stdio::null())
            .stdout(log)
            .stderr(log_for_errors);
        if let Some(start) = &self.shared.daemon_start {
            command.envs(&start.env);
        }
        // A shim is runtime: a stop never sweeps it, because it must outlive the stop to reap its child and write
        // `exit.json`. The marker is in the environment at exec, which is what a scan reads.
        command.env(RUNTIME_MARKER, "1");
        // SAFETY: the closure runs between fork and exec and calls only `setsid`, which is async-signal-safe and
        // allocates nothing.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let mut child = command
            .spawn()
            .map_err(io_error(format!("starting {} shim", exe.display())))?;
        let pid = child.id();
        {
            let mut state = locked(&entry.inner);
            state.shim = Some(ShimId {
                pid,
                started_us: sysproc::start_time_us(pid),
            });
        }
        let reaped = entry.clone();
        let reaper = thread::Builder::new()
            .name("collide-shim-reaper".to_string())
            .spawn(move || {
                // The daemon's own child: its exit is taken here, and nowhere else takes it.
                let status = child.wait().ok();
                let mut state = locked(&reaped.inner);
                state.shim_exit = status;
                state.shim_gone = true;
            });
        match reaper {
            Ok(_) => Ok(()),
            Err(source) => Err(PhaseError::Io {
                context: "starting the shim's reaper thread".to_string(),
                source,
            }),
        }
    }

    /// A phase that could not be started is `failed`, and says so in the log.
    fn fail_to_start(&self, entry: &Arc<Entry>, error: &PhaseError) {
        self.log(&format!("phase {}: did not start: {error}", entry.key));
        // `None` for the first event of a phase that was never recorded; `advance` records from the held state.
        let pending_recorded = self
            .shared
            .store
            .events_since(0)
            .map(|events| {
                events.iter().any(|e| {
                    matches!(&e.body, EventBody::PhaseTransition { run, phase, .. }
                        if *run == entry.key.run && *phase == entry.key.phase)
                })
            })
            .unwrap_or(false);
        if !pending_recorded {
            let _ = self.record(&entry.key, None, PhaseState::Pending {});
        }
        match self.advance(entry, PhaseState::Failed { cause: None }, None) {
            Ok(advanced) if advanced.changed => self.ended(entry),
            Ok(_) => {}
            Err(e) => self.log(&format!(
                "phase {}: could not be recorded failed: {e}",
                entry.key
            )),
        }
    }

    // ---- recording -------------------------------------------------------------------------------------------

    /// Writes one `phase.transition` event. The caller holds nothing of the shims.
    fn record(
        &self,
        key: &PhaseKey,
        from: Option<PhaseState>,
        to: PhaseState,
    ) -> Result<(), StoreError> {
        self.append_event(
            Actor {
                kind: ActorKind::Workflow,
                run: Some(key.run.clone()),
                phase: Some(key.phase.clone()),
            },
            EventBody::PhaseTransition {
                run: key.run.clone(),
                phase: key.phase.clone(),
                round: 1,
                from,
                to,
                log: None,
                transcript: None,
            },
        )
    }

    /// Appends one event, numbered and stamped here. Every writer of the event log goes through this, so two threads
    /// never take the same number: the phases' own transitions and the run and workspace events of a file-task run
    /// ([`crate::file_run`]).
    pub fn append_event(&self, actor: Actor, body: EventBody) -> Result<(), StoreError> {
        let _events = locked(&self.shared.events);
        let seq = self.shared.store.last_seq()? + 1;
        let event = Event {
            seq,
            wall_ms: self.now_ms(),
            actor,
            experiment: None,
            body,
        };
        self.shared.store.append_event(&event)
    }

    /// Moves a phase to `to` if the state machine allows it: the event first, then the state. A move to the state the
    /// phase is in changes nothing and records nothing. A store that fails is logged, and the state still moves, so
    /// what the daemon knows of a shim is not lost with a disk problem; the move is then reported not durable.
    fn advance(
        &self,
        entry: &Arc<Entry>,
        to: PhaseState,
        exit: Option<ExitRecord>,
    ) -> Result<Advanced, PhaseError> {
        let mut state = locked(&entry.inner);
        if state.state == to {
            if state.exit.is_none() {
                state.exit = exit;
            }
            return Ok(Advanced {
                changed: false,
                durable: true,
            });
        }
        phase_transition(&state.state, &to).map_err(PhaseError::Transition)?;
        let recorded = self.record(&entry.key, Some(state.state.clone()), to.clone());
        let durable = recorded.is_ok();
        if let Err(error) = recorded {
            self.log(&format!(
                "phase {}: the event for {} -> {} was not stored: {error}",
                entry.key,
                state_name(&state.state),
                state_name(&to)
            ));
        }
        let from = std::mem::replace(&mut state.state, to);
        if exit.is_some() {
            state.exit = exit;
        }
        self.log(&format!(
            "phase {}: {} -> {}{}",
            entry.key,
            state_name(&from),
            state_name(&state.state),
            state
                .exit
                .as_ref()
                .map(|e| format!(" ({})", exit_text(e)))
                .unwrap_or_default()
        ));
        Ok(Advanced {
            changed: true,
            durable,
        })
    }

    /// Tells the end hook that the phase reached a final state. No lock is held while it runs.
    fn ended(&self, entry: &Arc<Entry>) {
        let end = {
            let state = locked(&entry.inner);
            PhaseEnd {
                key: entry.key.clone(),
                state: state.state.clone(),
                exit: state.exit.clone(),
            }
        };
        if let Some(hook) = &self.shared.on_phase_end {
            hook(self, &end);
        }
    }

    /// The first event of a phase the daemon did not start and has no event for: it is recorded `running`, so the
    /// events of its end are moves the machine allows.
    fn entry_for_adoption(&self, shim: &AdoptedShim) -> Arc<Entry> {
        if let Some(entry) = self.entry(&shim.key) {
            return entry;
        }
        let entry = Arc::new(Entry {
            key: shim.key.clone(),
            dir: shim.dir.clone(),
            inner: Mutex::new(EntryState::new(PhaseState::Running {})),
            feed: None,
        });
        locked(&self.shared.table).insert(shim.key.clone(), entry.clone());
        if let Err(error) = self.record(&shim.key, None, PhaseState::Running {}) {
            self.log(&format!(
                "phase {}: the event that records it running was not stored: {error}",
                shim.key
            ));
        }
        entry
    }

    // ---- adoption --------------------------------------------------------------------------------------------

    /// Applies what [`crate::daemon::adopt_shims`] found, before the daemon binds its socket. Everything here is
    /// quick and none of it talks to a shim: a live shim gets a watcher thread, which connects on its own.
    pub fn adopt(&self, found: Vec<AdoptedShim>) {
        let mut ended = Vec::new();
        for shim in found {
            let entry = self.entry_for_adoption(&shim);
            if let Some(identity) = shim.identity {
                locked(&entry.inner).shim = Some(ShimId {
                    pid: identity.pid,
                    started_us: Some(identity.started_unix_us),
                });
            }
            if matches!(locked(&entry.inner).state, PhaseState::Pending {})
                && let Err(error) = self.advance(&entry, PhaseState::Running {}, None)
            {
                self.log(&format!("phase {}: {error}", shim.key));
            }
            match shim.state {
                AdoptedState::Alive => {
                    self.log(&format!(
                        "adopted phase {}: its shim {} is running",
                        shim.key,
                        shim.identity
                            .map_or("?".to_string(), |id| id.pid.to_string())
                    ));
                    self.watch(&entry);
                }
                AdoptedState::Exited(record) => {
                    if self.settle_with_exit(&entry, record).changed {
                        ended.push(entry.clone());
                    }
                    // A shim that is still there waits for the acknowledgement.
                    if entry.socket().exists() {
                        self.watch(&entry);
                    }
                }
                AdoptedState::Damaged { reason } => {
                    let moved = move_damaged_exit(&entry.dir, self.now_ms());
                    self.log(&format!(
                        "phase {}: exit.json is damaged ({reason}); {}",
                        shim.key,
                        match moved {
                            Ok(path) => format!("it was moved to {}", path.display()),
                            Err(error) => format!("it could not be moved aside: {error}"),
                        }
                    ));
                    if self.settle_gone(&entry) {
                        ended.push(entry.clone());
                    }
                }
                AdoptedState::Lost => {
                    if self.settle_gone(&entry) {
                        ended.push(entry.clone());
                    }
                }
            }
        }
        for entry in ended {
            self.ended(&entry);
        }
    }

    /// Ends the phase with the shim's record: `ended` for a clean exit, `failed` otherwise (see the module doc for the
    /// `result`). A phase that is final already keeps its state and takes the record if it had none.
    fn settle_with_exit(&self, entry: &Arc<Entry>, exit: ExitRecord) -> Advanced {
        if matches!(locked(&entry.inner).state, PhaseState::Pending {})
            && let Err(error) = self.advance(entry, PhaseState::Running {}, None)
        {
            self.log(&format!("phase {}: {error}", entry.key));
        }
        // An agent phase's outcome is its session's: the stream's `result` (and not the exit code) says whether the
        // agent finished. The journal is read to its end first, which is complete once the shim reports the exit.
        let outcome = self.settle_feed(entry, &exit);
        // A phase a stop was asked to end is `stopped`, whatever its exit was: the person asked for it.
        let to = if locked(&entry.inner).stop_requested {
            PhaseState::Stopped {}
        } else {
            match outcome {
                Some(Settled {
                    outcome: PhaseOutcome::Stopped,
                    ..
                }) => PhaseState::Stopped {},
                Some(Settled {
                    outcome: PhaseOutcome::Lost,
                    ..
                }) => PhaseState::Lost {},
                Some(Settled {
                    outcome: PhaseOutcome::Ended,
                    ..
                }) => phase_ending(&PhaseEnding {
                    exit_recorded: true,
                    result_seen: true,
                    volume_full: false,
                }),
                Some(Settled { volume_full, .. }) => phase_ending(&PhaseEnding {
                    exit_recorded: true,
                    result_seen: false,
                    volume_full,
                }),
                None => {
                    let result_seen = exit.end == ExitEnd::Exited { code: 0 };
                    phase_ending(&PhaseEnding {
                        exit_recorded: true,
                        result_seen,
                        volume_full: !result_seen && self.volume_full(entry),
                    })
                }
            }
        };
        match self.advance(entry, to, Some(exit)) {
            Ok(advanced) => advanced,
            Err(error) => {
                self.log(&format!("phase {}: {error}", entry.key));
                Advanced {
                    changed: false,
                    durable: false,
                }
            }
        }
    }

    /// Settles an agent phase's session with the shim's record: the journal is read to its end, the session says how
    /// the phase ended, and the phase's record (its last usage, its report, why it failed) is stored before the phase
    /// moves, so a reader who sees the phase final sees the record. `None` for a phase with no session and for one that
    /// is final already (its session settled once).
    fn settle_feed(&self, entry: &Arc<Entry>, exit: &ExitRecord) -> Option<Settled> {
        let feed = entry.feed.as_ref()?;
        if is_final(&locked(&entry.inner).state) {
            return None;
        }
        let (outcome, mut record, problem) = locked(feed).settle(Some(exit.clone()));
        if let Some(problem) = problem {
            self.log(&format!("phase {}: {problem}", entry.key));
        }
        // Only an ending with no `result` can be a full volume: any other failure had its say in the stream.
        let volume_full = matches!(
            outcome,
            PhaseOutcome::Failed {
                cause: PhaseFailure::NoResult
            }
        ) && self.volume_full(entry);
        if volume_full {
            record.failure = Some(DISK_FAILURE.to_string());
        }
        self.store_record(entry, &record);
        Some(Settled {
            outcome,
            volume_full,
        })
    }

    /// Whether the volume that holds the phase's journal is under the free-space floor now. A volume that cannot be
    /// read counts as not full, so the phase keeps its other cause.
    fn volume_full(&self, entry: &Arc<Entry>) -> bool {
        let Some((free, floor)) = &self.shared.disk else {
            return false;
        };
        match DiskGuard::new(free.as_ref(), *floor).volume_full(&entry.dir) {
            Ok(full) => full,
            Err(error) => {
                self.log(&format!("phase {}: {error}", entry.key));
                false
            }
        }
    }

    fn store_record(&self, entry: &Arc<Entry>, record: &PhaseRecord) {
        if let Err(error) =
            self.shared
                .store
                .set_phase_record(&entry.key.run, &entry.key.phase, record)
        {
            self.log(&format!(
                "phase {}: its record was not stored: {error}",
                entry.key
            ));
        }
    }

    /// Reads what the shim has journaled since the last read, feeds it to the phase's session and acts on what the
    /// session asks: bytes to write (sent without waiting, because the watcher is the thread that reads the answer),
    /// stdin to close, and a record to store.
    fn pump_feed(&self, entry: &Arc<Entry>, link: &Arc<Link>) {
        let Some(feed) = &entry.feed else { return };
        if is_final(&locked(&entry.inner).state) {
            return;
        }
        let (pumped, record) = {
            let mut feed = locked(feed);
            let pumped = feed.pump();
            let record = pumped.record_changed.then(|| feed.record(None));
            (pumped, record)
        };
        if let Some(problem) = &pumped.problem {
            self.log(&format!("phase {}: {problem}", entry.key));
        }
        for bytes in &pumped.writes {
            let data = String::from_utf8_lossy(bytes).into_owned();
            // A request nobody waits on still takes the shim's answer, so the answers stay in step.
            let (reply, _unwanted) = mpsc::channel();
            let mut writer = locked(&link.writer);
            locked(&link.pending).push_back(Pending {
                sent: Instant::now(),
                reply,
            });
            if let Err(error) = send(&mut writer, &DaemonMessage::Write { data }) {
                locked(&link.pending).pop_back();
                self.log(&format!(
                    "phase {}: a write of the session was not sent: {error}",
                    entry.key
                ));
            }
        }
        if pumped.close_stdin
            && let Err(error) = link.send(&DaemonMessage::CloseStdin {})
        {
            self.log(&format!(
                "phase {}: stdin was not closed after the turn: {error}",
                entry.key
            ));
        }
        if let Some(record) = record {
            self.store_record(entry, &record);
        }
    }

    /// Sends the phase's session a user turn and writes what it answers to the child, waiting for the shim's answer to
    /// each write. The first turn of an agent phase is sent this way, once the phase is started.
    pub fn send_turn(&self, key: &PhaseKey, text: &str) -> Result<(), LinkError> {
        let entry = self
            .entry(key)
            .ok_or_else(|| LinkError::NoLink(key.clone()))?;
        let feed = entry
            .feed
            .as_ref()
            .ok_or_else(|| LinkError::NoLink(key.clone()))?;
        let step = locked(feed).send_turn(text);
        for bytes in step.writes {
            self.write(key, &String::from_utf8_lossy(&bytes))?;
        }
        Ok(())
    }

    /// The phase's shim is gone. A record on disk ends the phase as it says; none makes it `lost`, whatever the agent
    /// reported (0.2), and a child that still runs is stopped. A phase that never ran fails. Returns whether the
    /// phase reached a final state now.
    fn settle_gone(&self, entry: &Arc<Entry>) -> bool {
        if is_final(&locked(&entry.inner).state) {
            return false;
        }
        match read_exit_json(&entry.dir) {
            ExitJson::Record(record) => return self.settle_with_exit(entry, record).changed,
            ExitJson::Damaged(reason) => {
                let moved = move_damaged_exit(&entry.dir, self.now_ms());
                self.log(&format!(
                    "phase {}: exit.json is damaged ({reason}); {}",
                    entry.key,
                    match moved {
                        Ok(path) => format!("it was moved to {}", path.display()),
                        Err(error) => format!("it could not be moved aside: {error}"),
                    }
                ));
            }
            ExitJson::Missing => {}
        }
        let to = if matches!(locked(&entry.inner).state, PhaseState::Pending {}) {
            PhaseState::Failed { cause: None }
        } else {
            // No record and no `result` on a full volume is `failed` with cause `disk`: the record could not be
            // written. Anything else is lost (0.2, Durability).
            phase_ending(&PhaseEnding {
                exit_recorded: false,
                result_seen: false,
                volume_full: self.volume_full(entry),
            })
        };
        let lost = matches!(to, PhaseState::Lost {});
        let changed = match self.advance(entry, to, None) {
            Ok(advanced) => advanced.changed,
            Err(error) => {
                self.log(&format!("phase {}: {error}", entry.key));
                false
            }
        };
        if changed && lost {
            self.log(&format!(
                "phase {} is lost: its shim ended without writing exit.json",
                entry.key
            ));
            self.stop_lost_child(entry);
        }
        changed
    }

    /// Stops what a lost phase's child left running, in the order of [`crate::stop::ORDER`] and over the child's whole
    /// tree, on a thread of its own: the grace is seconds, and the watcher or the adoption that noticed must go on.
    /// The roots are the child by `child.json` (its pid and start time, so a reused pid is never signalled). The
    /// marker scan runs only when no other phase of the run is live, because it would reach that phase too.
    fn stop_lost_child(&self, entry: &Arc<Entry>) {
        let Some(identity) = read_identity(&entry.dir.join(shim_files::CHILD)) else {
            return;
        };
        let other_phase_live = locked(&self.shared.table).values().any(|other| {
            other.key.run == entry.key.run
                && other.key != entry.key
                && !is_final(&locked(&other.inner).state)
        });
        let mut request = StopRequest::new(entry.key.run.clone());
        request.roots = vec![Root {
            pid: identity.pid,
            started_us: identity.started_unix_us,
        }];
        request.by_marker = !other_phase_live;
        request.run_start_us = self.run_start_us(&entry.key.run);
        request.grace = self.shared.options.lost_grace;
        request.poll = self.shared.options.poll;
        request.detached = self.detached_of(&entry.key.run).unwrap_or_default();
        let phases = self.clone();
        let key = entry.key.clone();
        let spawned = thread::Builder::new()
            .name(format!("collide-stop-{key}"))
            .spawn(move || {
                match stop_tree(
                    phases.shared.processes.as_ref(),
                    &request,
                    phases.cleanup_hook(),
                ) {
                    Ok(report) => phases.log(&format!(
                        "phase {key}: the stop of its lost child signalled {} process(es), killed {}, and left {} alive",
                        report.signalled.len(),
                        report.killed.len(),
                        report.survivors.len()
                    )),
                    Err(error) => phases.log(&format!(
                        "phase {key}: its lost child could not be stopped: {error}"
                    )),
                }
            });
        match spawned {
            Ok(handle) => locked(&self.shared.stoppers).push(handle),
            Err(error) => self.log(&format!(
                "phase {}: the thread that stops its lost child could not start: {error}",
                entry.key
            )),
        }
    }

    fn cleanup_hook(&self) -> Option<&dyn Cleanup> {
        self.shared
            .cleanup
            .as_ref()
            .map(|hook| hook.as_ref() as &dyn Cleanup)
    }

    /// The records the run's hooks made of what they detached.
    fn detached_of(&self, run: &str) -> io::Result<Vec<crate::stop::Detached>> {
        read_detached(&self.shared.runs_dir.join(run))
    }

    /// When the run began, for the scan, in microseconds since the epoch: the time of the run's first `run.transition`
    /// event, which is the run's record (ts-0059 decision 5). A run with no such event (one started with
    /// [`Phases::start_phase`] alone) falls back to the earliest `spec.json` of its phases. A process older than this
    /// cannot be the run's.
    pub fn run_start_us(&self, run: &str) -> Option<u64> {
        if let Ok(events) = self.shared.store.events_since(0) {
            let first = events.iter().find_map(|event| match &event.body {
                EventBody::RunTransition { run: named, .. } if named == run => Some(event.wall_ms),
                _ => None,
            });
            if let Some(wall_ms) = first {
                return wall_ms.checked_mul(1000);
            }
        }
        let entries: Vec<Arc<Entry>> = locked(&self.shared.table)
            .values()
            .filter(|e| e.key.run == run)
            .cloned()
            .collect();
        entries
            .iter()
            .filter_map(|e| {
                let modified = fs::metadata(e.dir.join(shim_files::SPEC))
                    .ok()?
                    .modified()
                    .ok()?;
                let since = modified.duration_since(std::time::UNIX_EPOCH).ok()?;
                u64::try_from(since.as_micros()).ok()
            })
            .min()
    }

    // ---- stopping a run --------------------------------------------------------------------------------------

    /// Stops a run: every phase of it that is not final is marked, so that its exit settles it `stopped`; then the
    /// tree is found and stopped in the order of [`crate::stop::ORDER`] ([`stop_tree`]), with the run's id as the
    /// marker and each phase's child (by `child.json`) as a root; and each marked phase is waited for until it
    /// settles. `grace` is the time between SIGTERM and SIGKILL ([`crate::stop::DEFAULT_GRACE`] when `None`). It blocks
    /// for as long as that, so a caller serves it on a thread of its own.
    ///
    /// A run the daemon does not know is an error and nothing is signalled. A run whose phases are all final is
    /// swept too: what a finished run left running is what a stop is for.
    pub fn stop_run(&self, run: &str, grace: Option<Duration>) -> Result<StopReport, StopRunError> {
        check_name("run", run).map_err(StopRunError::Name)?;
        let entries: Vec<Arc<Entry>> = locked(&self.shared.table)
            .values()
            .filter(|e| e.key.run == run)
            .cloned()
            .collect();
        if entries.is_empty() {
            return Err(StopRunError::Unknown(run.to_string()));
        }
        let mut request = StopRequest::new(run);
        request.run_start_us = self.run_start_us(run);
        request.poll = self.shared.options.poll;
        if let Some(grace) = grace {
            request.grace = grace;
        }
        let mut problems = Vec::new();
        match self.detached_of(run) {
            Ok(detached) => request.detached = detached,
            Err(error) => problems.push(format!("the record of detached processes: {error}")),
        }
        let mut marked = Vec::new();
        for entry in &entries {
            {
                let mut state = locked(&entry.inner);
                if !is_final(&state.state) {
                    state.stop_requested = true;
                    marked.push(entry.clone());
                }
            }
            if let Some(identity) = read_identity(&entry.dir.join(shim_files::CHILD)) {
                request.roots.push(Root {
                    pid: identity.pid,
                    started_us: identity.started_unix_us,
                });
            }
        }
        self.log(&format!(
            "run {run}: stopping {} phase(s), grace {} s",
            marked.len(),
            request.grace.as_secs_f64()
        ));
        let mut report = stop_tree(
            self.shared.processes.as_ref(),
            &request,
            self.cleanup_hook(),
        )
        .map_err(StopRunError::Refused)?;
        let until = Instant::now() + STOP_SETTLE_WAIT;
        for entry in &marked {
            while !is_final(&locked(&entry.inner).state) && Instant::now() < until {
                thread::sleep(self.shared.options.poll);
            }
            if !is_final(&locked(&entry.inner).state) {
                problems.push(format!("phase {} did not settle", entry.key));
            }
        }
        report.problems.extend(problems);
        self.log(&format!(
            "run {run}: stopped; {} signalled, {} killed, {} left alive",
            report.signalled.len(),
            report.killed.len(),
            report.survivors.len()
        ));
        Ok(report)
    }

    /// The run's state as its phases give it, until the run machine is the daemon's (Stage 4): `running` while a
    /// phase is not final; else `stopped` if a phase was stopped, `failed` if one failed or was lost, and `ended`
    /// otherwise. `None` for a run with no phases.
    pub fn run_state(&self, run: &str) -> Option<RunState> {
        let states: Vec<PhaseState> = locked(&self.shared.table)
            .values()
            .filter(|e| e.key.run == run)
            .map(|e| locked(&e.inner).state.clone())
            .collect();
        if states.is_empty() {
            // A run that has not started a phase (paused for disk before it did) has only its own record.
            return self.recorded_run_state(run);
        }
        Some(if states.iter().any(|s| !is_final(s)) {
            RunState::Running {}
        } else if states.iter().any(|s| matches!(s, PhaseState::Stopped {})) {
            RunState::Stopped {}
        } else if states
            .iter()
            .any(|s| matches!(s, PhaseState::Failed { .. } | PhaseState::Lost {}))
        {
            RunState::Failed {}
        } else {
            RunState::Ended {
                outcome: NonEmpty::new("every phase ended").expect("the text is not empty"),
            }
        })
    }

    /// The `to` of the run's last `run.transition` event.
    fn recorded_run_state(&self, run: &str) -> Option<RunState> {
        let events = self.shared.store.events_since(0).ok()?;
        events.into_iter().rev().find_map(|event| match event.body {
            EventBody::RunTransition { run: named, to, .. } if named == run => Some(to),
            _ => None,
        })
    }

    // ---- health ----------------------------------------------------------------------------------------------

    fn mark_unresponsive(&self, entry: &Arc<Entry>) {
        let mut state = locked(&entry.inner);
        if state.health == Health::Responsive {
            state.health = Health::Unresponsive {
                since_ms: self.now_ms(),
            };
            self.log(&format!("the shim of phase {} is not answering", entry.key));
        }
    }

    fn mark_responsive(&self, entry: &Arc<Entry>) {
        let mut state = locked(&entry.inner);
        if state.health != Health::Responsive {
            state.health = Health::Responsive;
            self.log(&format!("the shim of phase {} answers again", entry.key));
        }
    }

    /// Whether the phase's shim is known to be gone: the reaper has it, or its pid and start time name no running
    /// process. A shim whose identity is not known is not called gone here.
    fn shim_is_gone(&self, entry: &Arc<Entry>) -> bool {
        let known = {
            let state = locked(&entry.inner);
            if state.shim_gone {
                return true;
            }
            state.shim
        };
        let id = known.or_else(|| {
            let identity = read_identity(&entry.dir.join(shim_files::SHIM))?;
            let id = ShimId {
                pid: identity.pid,
                started_us: Some(identity.started_unix_us),
            };
            locked(&entry.inner).shim = Some(id);
            Some(id)
        });
        id.is_some_and(|id| !process_is_running(id.pid, id.started_us))
    }

    // ---- the watcher -----------------------------------------------------------------------------------------

    /// Starts the phase's watcher, if one is not running.
    fn watch(&self, entry: &Arc<Entry>) {
        {
            let mut state = locked(&entry.inner);
            if state.watching || self.stopping() {
                return;
            }
            state.watching = true;
        }
        let phases = self.clone();
        let watched = entry.clone();
        let started = thread::Builder::new()
            .name(format!("collide-watch-{}", entry.key))
            .spawn(move || {
                phases.watch_loop(&watched);
                locked(&watched.inner).watching = false;
            });
        match started {
            Ok(handle) => locked(&self.shared.watchers).push(handle),
            Err(error) => {
                locked(&entry.inner).watching = false;
                self.log(&format!(
                    "phase {}: its watcher thread could not start: {error}",
                    entry.key
                ));
            }
        }
    }

    /// Connects to the shim, reads what it says, and when the link goes down decides whether the shim is gone. It ends
    /// when the phase is final and the record is acknowledged, when the shim is gone, or when the daemon stops.
    fn watch_loop(&self, entry: &Arc<Entry>) {
        let options = self.shared.options;
        let socket = entry.socket();
        let mut unreachable_since: Option<Instant> = None;
        while !self.stopping() {
            let attempt = connect_shim(&socket, options.poll, options.reply_wait, |waited| {
                if self.stopping() {
                    return false;
                }
                if waited >= options.unresponsive_after {
                    self.mark_unresponsive(entry);
                }
                !self.shim_is_gone(entry)
            });
            match attempt {
                Ok(connection) => {
                    unreachable_since = None;
                    self.serve_link(entry, connection);
                    let state = locked(&entry.inner);
                    if is_final(&state.state) && state.acknowledged {
                        return;
                    }
                }
                Err(failure) => {
                    if let Some(pid) = failure.shim_pid {
                        let mut state = locked(&entry.inner);
                        if state.shim.is_none() {
                            state.shim = Some(ShimId {
                                pid,
                                started_us: sysproc::start_time_us(pid),
                            });
                        }
                    }
                    let since = *unreachable_since.get_or_insert_with(Instant::now);
                    if since.elapsed() >= options.unresponsive_after && !self.shim_is_gone(entry) {
                        // Alive and not reachable: it is not answering.
                        if !is_final(&locked(&entry.inner).state) {
                            self.mark_unresponsive(entry);
                        }
                    }
                }
            }
            if self.stopping() {
                return;
            }
            // The link is down, or never came up. A shim that is gone settles the phase; one that is not is tried
            // again, and one that is not known to be there for `bind_wait` is called gone.
            let gone = self.shim_is_gone(entry)
                || unreachable_since.is_some_and(|since| {
                    since.elapsed() >= options.bind_wait && locked(&entry.inner).shim.is_none()
                });
            if gone {
                if self.settle_gone(entry) {
                    self.ended(entry);
                }
                return;
            }
            if is_final(&locked(&entry.inner).state) && !entry.socket().exists() {
                return;
            }
            thread::sleep(options.poll);
        }
    }

    /// Serves one connection after the handshake: the answers to requests, the notices, and the exit record.
    fn serve_link(&self, entry: &Arc<Entry>, connection: ShimConnection) {
        let ShimConnection {
            writer,
            mut reader,
            shim_pid,
            ..
        } = connection;
        let link = Arc::new(Link {
            writer: Mutex::new(writer),
            pending: Mutex::new(VecDeque::new()),
        });
        {
            let mut state = locked(&entry.inner);
            if let Some(pid) = shim_pid
                && state.shim.is_none()
            {
                state.shim = Some(ShimId {
                    pid,
                    started_us: sysproc::start_time_us(pid),
                });
            }
            state.link = Some(link.clone());
        }
        self.mark_responsive(entry);
        let mut stopped_since: Option<Instant> = None;
        loop {
            if self.stopping() {
                break;
            }
            match reader.read_next::<ShimMessage>() {
                Ok(Next::Message(message)) => {
                    if !self.take_message(entry, &link, message) {
                        break;
                    }
                }
                Ok(Next::Idle) => {}
                Ok(Next::Closed { .. }) => break,
                Err(error) => {
                    self.log(&format!(
                        "phase {}: the shim's link was closed: {error}",
                        entry.key
                    ));
                    break;
                }
            }
            self.pump_feed(entry, &link);
            self.refresh_health(entry, &link, &mut stopped_since);
        }
        locked(&entry.inner).link = None;
        // Whoever is still waiting for an answer is told there is none: their senders drop with the queue.
        locked(&link.pending).clear();
    }

    /// Handles one message. `false` ends the connection.
    fn take_message(&self, entry: &Arc<Entry>, link: &Arc<Link>, message: ShimMessage) -> bool {
        match message {
            ShimMessage::Hello { .. } => {}
            ShimMessage::Wrote { seq } => {
                if let Some(waiting) = locked(&link.pending).pop_front() {
                    let _ = waiting.reply.send(Reply::Wrote(seq));
                }
            }
            ShimMessage::Failed { message } => {
                if let Some(notice) = message.strip_prefix(PROBLEM_PREFIX) {
                    self.log(&format!("phase {}: the shim reports: {notice}", entry.key));
                } else if let Some(waiting) = locked(&link.pending).pop_front() {
                    let _ = waiting.reply.send(Reply::Failed(message));
                } else {
                    self.log(&format!(
                        "phase {}: the shim could not do a request: {message}",
                        entry.key
                    ));
                }
            }
            ShimMessage::Exited { exit } => {
                let advanced = self.settle_with_exit(entry, exit);
                if advanced.changed {
                    self.ended(entry);
                }
                let recorded = is_final(&locked(&entry.inner).state)
                    && (advanced.durable || !advanced.changed);
                if recorded {
                    // The event is stored: the shim may stop serving the record.
                    match link.send(&DaemonMessage::AckExit {}) {
                        Ok(()) => locked(&entry.inner).acknowledged = true,
                        Err(error) => self.log(&format!(
                            "phase {}: the acknowledgement was not sent: {error}",
                            entry.key
                        )),
                    }
                }
            }
        }
        true
    }

    /// Reports the shim unresponsive or responsive from what is known: its process stopped for long enough, or a
    /// request without an answer for longer than the reply wait.
    fn refresh_health(&self, entry: &Arc<Entry>, link: &Link, stopped_since: &mut Option<Instant>) {
        let options = self.shared.options;
        let pid = locked(&entry.inner).shim.map(|s| s.pid);
        let stopped = pid.is_some_and(|pid| sysproc::is_stopped(pid) == Some(true));
        if stopped {
            stopped_since.get_or_insert_with(Instant::now);
        } else {
            *stopped_since = None;
        }
        let silent =
            stopped_since.is_some_and(|since| since.elapsed() >= options.unresponsive_after);
        let overdue = link
            .oldest_wait()
            .is_some_and(|waited| waited >= options.reply_wait);
        if silent || overdue {
            self.mark_unresponsive(entry);
        } else if !stopped {
            self.mark_responsive(entry);
        }
    }
}

impl EntryState {
    fn new(state: PhaseState) -> EntryState {
        EntryState {
            state,
            exit: None,
            shim: None,
            shim_exit: None,
            shim_gone: false,
            link: None,
            health: Health::Responsive,
            acknowledged: false,
            watching: false,
            stop_requested: false,
        }
    }
}

// ---- reading what a shim left ---------------------------------------------------------------------------------

/// What `exit.json` holds.
pub enum ExitJson {
    Record(ExitRecord),
    /// The file is there and is not a record: the reason, as `core` words it.
    Damaged(String),
    Missing,
}

/// Reads `dir/exit.json`.
pub fn read_exit_json(dir: &Path) -> ExitJson {
    match fs::read(dir.join(shim_files::EXIT_RECORD)) {
        Ok(bytes) => match collide_core::shim::parse_exit_record(&bytes) {
            Ok(record) => ExitJson::Record(record),
            Err(error) => ExitJson::Damaged(error.to_string()),
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => ExitJson::Missing,
        Err(error) => ExitJson::Damaged(format!("it could not be read: {error}")),
    }
}

/// Moves a damaged `exit.json` aside, to a name that is not taken, so it is never read twice and never written over.
pub fn move_damaged_exit(dir: &Path, wall_ms: u64) -> io::Result<PathBuf> {
    let from = dir.join(shim_files::EXIT_RECORD);
    for attempt in 0u32.. {
        let name = if attempt == 0 {
            format!("{}.damaged-{wall_ms}", shim_files::EXIT_RECORD)
        } else {
            format!("{}.damaged-{wall_ms}-{attempt}", shim_files::EXIT_RECORD)
        };
        let to = dir.join(name);
        if !to.exists() {
            fs::rename(&from, &to)?;
            return Ok(to);
        }
    }
    unreachable!("the loop ends by returning")
}

/// Reads a pid-and-start-time file ([`ChildIdentity`]'s shape): `child.json` or `shim.json`.
pub fn read_identity(path: &Path) -> Option<ChildIdentity> {
    let text = fs::read_to_string(path).ok()?;
    ChildIdentity::from_json(&serde_json::from_str(&text).ok()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_a_safe_directory_name() {
        for bad in [
            "",
            ".",
            "..",
            "a/b",
            "a\\b",
            ".hidden",
            "with space",
            "nul\0",
        ] {
            assert!(PhaseKey::new(bad, "p").is_err(), "{bad:?}");
        }
        assert!(PhaseKey::new(&"x".repeat(MAX_NAME_BYTES + 1), "p").is_err());
        assert!(PhaseKey::new(&"x".repeat(MAX_NAME_BYTES), "p").is_ok());
        assert_eq!(
            PhaseKey::new("run-1", "p.2").unwrap().to_string(),
            "run-1/p.2"
        );
    }

    #[test]
    fn a_pid_that_is_gone_is_not_running() {
        // A pid far above any a test machine hands out.
        assert!(!process_is_running(4_000_000, None));
    }

    #[test]
    fn this_process_is_running_with_its_own_start_time() {
        let me = std::process::id();
        let started = sysproc::start_time_us(me);
        assert!(process_is_running(me, started));
        assert!(!process_is_running(me, started.map(|t| t + 1)));
    }

    #[test]
    fn a_damaged_record_is_moved_to_a_name_that_is_not_taken() {
        let dir = std::env::temp_dir().join(format!("collide-phases-unit-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let record = dir.join(shim_files::EXIT_RECORD);
        fs::write(&record, "{").unwrap();
        let first = move_damaged_exit(&dir, 7).unwrap();
        fs::write(&record, "{").unwrap();
        let second = move_damaged_exit(&dir, 7).unwrap();
        assert_ne!(first, second);
        assert!(!record.exists());
        for path in [&first, &second] {
            fs::remove_file(path).unwrap();
        }
        fs::remove_dir(&dir).unwrap();
    }

    #[test]
    fn an_exit_is_worded_by_its_code_or_its_signal() {
        let mut record = ExitRecord {
            protocol: 1,
            end: ExitEnd::Exited { code: 7 },
            cpu_user_us: 0,
            cpu_system_us: 0,
            max_rss_bytes: 0,
        };
        assert_eq!(exit_text(&record), "exited with code 7");
        record.end = ExitEnd::Signaled { signal: 9 };
        assert_eq!(exit_text(&record), "ended by signal 9");
    }
}
