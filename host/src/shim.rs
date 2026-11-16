//! The `collide shim` mode (0.2, The owner is `collide-daemon`, with a per-child shim; Stage 3a; spike S3). One
//! shim runs for one child, so the child's owner is a small process that a daemon restart does not touch. The
//! protocol it speaks is `core`'s ([`collide_core::shim`]), the framing is [`collide_core::frame`], and the journal's
//! record is [`collide_core::journal`]; this module is the process that uses them.
//!
//! The shim is synchronous: threads and blocking calls, no async runtime. It:
//!
//! 1. calls `setsid`, with the child started by `std::process::Command` and `pre_exec(setsid)` ([`spawn_child`]);
//! 2. holds the child's stdin pipe, or for an interactive session the PTY master, across a daemon restart
//!    ([`ChildStdio`], [`Shim`]). The child's stdout and stderr are pipes the shim reads and writes to the log files
//!    ([`files`]), which is what lets it cap a line and a log ([`Limits`]);
//! 3. records the child's pid and start time in the run directory ([`ChildIdentity`]), so the daemon can stop that
//!    child when it marks the phase `lost`;
//! 4. journals the traffic in one ordered file, each record written before the child sees the bytes
//!    ([`JournalWriter`], [`Shim::write_to_child`]);
//! 5. for a PTY keeps the last [`PTY_RING_BYTES`] of output and replays it on attach, atomically with subscribing
//!    ([`PtyRing`]);
//! 6. reaps the child with `wait4` ([`reap`]). An agent's exit is **never** taken from std's or tokio's own wait
//!    calls, and a tokio `Child` is never dropped (measured S3). A test reads this file's source to keep that true;
//! 7. writes `exit.json` atomically ([`write_exit_record`]): temp file, `fsync`, rename, `fsync` of the directory.
//!    On a full disk it keeps the record in memory and serves it until the daemon acknowledges it ([`ExitKeeper`]);
//! 8. serves the daemon on its own socket while the child runs and until the exit record is acknowledged
//!    ([`ShimLink`]), and when the child ends with no daemon connected, starts one ([`ShimSpec::daemon`]).
//!
//! A shim that dies without writing `exit.json` makes its phase `lost`, whatever the agent's stream said. That is the
//! daemon's rule, so nothing here writes a record for a shim that did not finish.
//!
//! ## What the journal holds
//!
//! One `in` record for each [`Shim::write_to_child`] (split at [`MAX_RECORD_BYTES`]), and for a pipe child one `out`
//! record for each line of its stdout, whole, with the newline, so replay hands a session a line as the live session
//! had it. A line over [`Limits::line_bytes`] is cut and marked ([`line_cut_marker`]); the cut line is what the
//! journal and `stdout.log` both hold. A PTY's output is not journaled: it is terminal bytes with no lines, and the
//! ring keeps the end of it. Stderr is a log only. The journal itself has no cap: cutting it would break replay,
//! and its growth is bounded by what the child writes, line by line.
//!
//! ## The daemon link
//!
//! The shim owns a socket, `shim/s` in a `0700` directory under its run directory, which follows the daemon's own rules
//! (0.2, The socket): the directory's owner and mode are checked, the peer's uid must be the shim's, and there is no
//! token, because only the daemon talks to it. The daemon connects and the shim speaks first (`Hello`); a connection
//! is served on a thread of its own, so a daemon that says nothing, or is stopped, holds only its own thread.
//!
//! - `Write` is journaled and written to the child ([`Shim::write_to_child`]), and answered with `Wrote`. `CloseStdin`
//!   and `Signal` have no success answer in the protocol; a failure is a `Failed`.
//! - A first failure of a sink ([`Shim::problems`]) is sent once on each connection as `Failed`.
//! - **A message that has not all arrived is not read.** The framing is [`crate::shim_link::FrameReader`]'s: a peer
//!   that sends half a message and goes away is dropped with the half, and a peer that sends the rest later is read
//!   whole. A damaged frame closes that connection and no other.
//! - When the child is reaped the record is published ([`ShimLink::publish_exit`]): each connection is sent
//!   `Exited`, a connection that comes later is sent it on arrival, and the shim serves it until a daemon sends
//!   `AckExit` ([`ShimLink::wait_acknowledged`]). `exit.json` stays on disk for the daemon either way.
//! - When the child ends and no daemon is connected, and the spec says how ([`DaemonStart`]), the shim starts the
//!   daemon ([`crate::launch::ensure_daemon`]) so that a phase boundary is not missed while the app is closed
//!   (0.2, Starting the daemon). It then waits for the acknowledgement for [`ACK_WAIT`]; when `exit.json` could not be
//!   written it waits until it is acknowledged, because the record exists nowhere else.
//!
//! ## A failing disk
//!
//! A write that fails (`ENOSPC` is the case that is tested) never panics and never stops the child. The pumps keep
//! reading the pipes so the child does not block, drop what they cannot write, and note the first failure of each
//! sink in [`Shim::problems`], which the daemon link reports with [`ShimMessage::Failed`]. A journal that failed
//! stops writing for good, so it never holds a record after a cut one; an `in` write then fails and the child gets
//! nothing, because the journal comes first.
//!
//! | tag | function | what it does |
//! |---|---|---|
//! | `shim: entry` | [`run`] | the shim's `main`: arguments, the steps above in order, the exit code |
//! | `shim: spawn child` | [`spawn_child`] | `setsid`, the pipes or the PTY, and the child's identity file |
//! | `shim: reap` | [`reap`] | `wait4`, and the exit record built from its status and `rusage` |
//! | `shim: journal writer` | [`JournalWriter`] | one ordered file of records, journal first |
//! | `shim: exit record` | [`write_exit_record`], [`ExitKeeper`] | the atomic `exit.json`, and the in-memory copy on a full disk |
//! | `shim: pty ring` | [`PtyRing`] | the last 1 MiB of a PTY's output, and the replay on attach |
//! | `shim: daemon link` | [`ShimLink`] | the shim's socket: handshake, `Write`, `CloseStdin`, `Signal`, `AckExit` |

use crate::atomic_write::write_atomically;
use crate::auth::{self, StdFs, UnixPeer};
use crate::clock::{Clock, Clocks, SystemClocks};
use crate::daemon::{DaemonPaths, ROOT_ENV};
use crate::launch::{LAUNCHCTL_ENV, StartRequest, ensure_daemon, launchctl_from_env};
use crate::shim_link::{FrameReader, Next, send};
use crate::sysproc;
use collide_core::journal::{Direction, JournalRecord, MAX_RECORD_BYTES, encode_record};
use collide_core::shim::{
    DaemonMessage, ExitEnd, ExitRecord, SHIM_PROTOCOL, ShimMessage, StopSignal, exit_record_bytes,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{ChildStderr, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// The output a PTY's ring keeps (0.2, measured S2): the last 1 MiB.
pub const PTY_RING_BYTES: usize = 1024 * 1024;

/// The most bytes of one stdout or stderr line the shim keeps (0.2, Durability). The plan gives no number: 8 MiB holds
/// the biggest line a stream-json agent writes in practice (a tool result with a file in it) and stays under a
/// journal record's own limit, [`MAX_RECORD_BYTES`], with room for the marker.
pub const LINE_CAP_BYTES: usize = 8 * 1024 * 1024;

/// The most bytes one log file (`stdout.log`, `stderr.log`) grows to (0.2, Durability). The plan gives no number:
/// 256 MiB is far more than a long phase writes and still stops a runaway tool well before it fills a disk.
pub const LOG_CAP_BYTES: u64 = 256 * 1024 * 1024;

/// How long the shim waits, after the child is reaped, for its pipes to run dry before it writes `exit.json`. A
/// grandchild that kept a pipe open would otherwise hold the record back for ever.
pub const DRAIN_AFTER_EXIT: Duration = Duration::from_secs(2);

/// How long the shim waits, after its child ended and `exit.json` is on disk, for a daemon to acknowledge the record.
/// The plan sets none: the record is safe on disk, so the wait is only to give a daemon that is being started the
/// time to take it (a start waits up to 15 s, and adoption follows).
pub const ACK_WAIT: Duration = Duration::from_secs(60);

/// How long a daemon has, after it connects, to answer the shim's `Hello` with `Welcome`. The plan sets none: a daemon
/// that has connected answers at once, so this is for a peer that connects and says nothing.
pub const HANDSHAKE_WAIT: Duration = Duration::from_secs(10);

/// How often a link's threads look at whether the shim is ending, and so how long a read waits.
const LINK_POLL: Duration = Duration::from_millis(50);

/// How long a write to one daemon connection may block. A daemon that does not read is dropped, not waited for.
const LINK_WRITE_WAIT: Duration = Duration::from_secs(5);

/// How many daemon connections a shim serves at once: one daemon talks to it, and one that is about to take over.
const MAX_LINK_CONNECTIONS: usize = 8;

/// What a [`ShimMessage::Failed`] that reports a sink's failure starts with, as against one that answers a request. The
/// protocol has one `Failed` for both, so the text tells them apart and the daemon does not pair a notice with a
/// request.
pub const PROBLEM_PREFIX: &str = "sink problem: ";

/// The bytes a pump reads at once.
const READ_CHUNK_BYTES: usize = 64 * 1024;

/// The exit codes of the shim process itself, which are not the child's.
pub mod exit_code {
    /// The child ran, was reaped, and `exit.json` is on disk.
    pub const OK: i32 = 0;
    /// The shim could not start the child, or could not reap it.
    pub const FAILED: i32 = 1;
    /// The arguments or the spec were not understood.
    pub const USAGE: i32 = 2;
    /// The child was reaped but `exit.json` could not be written, and no daemon acknowledged the record from memory.
    pub const RECORD_NOT_WRITTEN: i32 = 3;
}

/// The files of a run directory, by name. The plan names `exit.json` and the journal and leaves the rest unnamed, so
/// these are this outline's choice.
pub mod files {
    pub const EXIT_RECORD: &str = "exit.json";
    pub const JOURNAL: &str = "journal";
    pub const STDOUT_LOG: &str = "stdout.log";
    pub const STDERR_LOG: &str = "stderr.log";
    /// The child's pid and start time ([`super::ChildIdentity`]).
    pub const CHILD: &str = "child.json";
    /// What the daemon tells the shim to start ([`super::ShimSpec`]), as JSON.
    pub const SPEC: &str = "spec.json";
    /// The shim's socket directory, mode `0700` (0.2).
    pub const SOCKET_DIR: &str = "shim";
    /// The shim's socket, inside [`SOCKET_DIR`]. Short, because the whole path is limited to 103 bytes (0.2).
    pub const SOCKET: &str = "s";
    /// What the shim process itself writes to its standard output and error, when the daemon starts it.
    pub const SHIM_LOG: &str = "shim.log";
    /// The shim process's own pid and start time ([`super::ChildIdentity`] shaped), so a daemon that restarts finds the
    /// shim by pid and start time and can tell a shim that ended from one that is only quiet.
    pub const SHIM: &str = "shim.json";
}

/// The caps and waits of a shim. [`Limits::default`] is what a running shim uses; a test passes small ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// A line of stdout or stderr over this many bytes (not counting its newline) is cut and marked.
    pub line_bytes: usize,
    /// A log that reaches this many bytes stops growing and ends with a marker.
    pub log_bytes: u64,
    /// How long to wait for the pipes after the child is reaped.
    pub drain_after_exit: Duration,
    /// How long the shim serves the exit record for a daemon to acknowledge it, once `exit.json` is on disk
    /// ([`ACK_WAIT`]). A record that is only in memory is served for as long as it takes.
    pub ack_wait: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            line_bytes: LINE_CAP_BYTES,
            log_bytes: LOG_CAP_BYTES,
            drain_after_exit: DRAIN_AFTER_EXIT,
            ack_wait: ACK_WAIT,
        }
    }
}

/// The text a cut line ends with, before its newline: how many bytes were dropped.
pub fn line_cut_marker(dropped: usize) -> String {
    format!("[collide: line cut, {dropped} bytes dropped]")
}

/// The line a log ends with when it reaches its cap. Nothing follows it.
pub fn log_full_marker(cap: u64) -> String {
    format!("[collide: log reached {cap} bytes, later output is not kept]\n")
}

/// How the child's stdin and output are connected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildStdio {
    /// A pipe for stdin, and pipes for stdout and stderr that the shim copies to the logs. A `claude -p` phase.
    Pipes,
    /// A PTY, whose master the shim holds. An interactive session (a take-over, a new session).
    Pty { rows: u16, cols: u16 },
}

/// How a shim starts the daemon when its child ends and no daemon is there to be told (0.2, Starting the daemon): the
/// `collide-daemon` binary and the environment it is started with. The environment carries `COLLIDE_DAEMON_ROOT` or
/// `HOME`, so the shim finds the same files the daemon that started it uses. A spec without it starts no daemon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonStart {
    pub exe: PathBuf,
    pub env: BTreeMap<String, String>,
}

/// What the daemon tells a shim to start: the child's command, environment and directory, and where the shim writes.
/// The environment is complete: the shim adds nothing and removes nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShimSpec {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub cwd: PathBuf,
    /// The run directory the files of [`files`] are in.
    pub run_dir: PathBuf,
    pub stdio: ChildStdio,
    /// How to start the daemon when the child ends with none connected. `None` starts none.
    pub daemon: Option<DaemonStart>,
}

impl ShimSpec {
    /// The JSON of `spec.json`.
    pub fn to_json(&self) -> Value {
        let stdio = match self.stdio {
            ChildStdio::Pipes => json!({ "type": "pipes" }),
            ChildStdio::Pty { rows, cols } => json!({ "type": "pty", "rows": rows, "cols": cols }),
        };
        let mut spec = json!({
            "program": self.program,
            "args": self.args,
            "env": self.env,
            "cwd": self.cwd,
            "run_dir": self.run_dir,
            "stdio": stdio,
        });
        if let (Some(daemon), Some(object)) = (&self.daemon, spec.as_object_mut()) {
            object.insert(
                "daemon".to_string(),
                json!({ "exe": daemon.exe, "env": daemon.env }),
            );
        }
        spec
    }

    /// Reads what [`ShimSpec::to_json`] wrote. The error names the field that is missing or of the wrong kind.
    pub fn from_json(value: &Value) -> Result<ShimSpec, ShimError> {
        let spec_error = |message: String| ShimError::Spec { message };
        let text = |name: &str| {
            value
                .get(name)
                .and_then(Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| spec_error(format!("`{name}` is missing or is not a string")))
        };
        let args = value
            .get("args")
            .and_then(Value::as_array)
            .ok_or_else(|| spec_error("`args` is missing or is not a list".into()))?
            .iter()
            .map(|arg| {
                arg.as_str()
                    .map(str::to_string)
                    .ok_or_else(|| spec_error("`args` holds something that is not a string".into()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let env = value
            .get("env")
            .and_then(Value::as_object)
            .ok_or_else(|| spec_error("`env` is missing or is not an object".into()))?
            .iter()
            .map(|(name, value)| {
                value
                    .as_str()
                    .map(|value| (name.clone(), value.to_string()))
                    .ok_or_else(|| spec_error(format!("`env` value of {name} is not a string")))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        let stdio = value
            .get("stdio")
            .ok_or_else(|| spec_error("`stdio` is missing".into()))?;
        let stdio = match stdio.get("type").and_then(Value::as_str) {
            Some("pipes") => ChildStdio::Pipes,
            Some("pty") => {
                let size = |name: &str| {
                    stdio
                        .get(name)
                        .and_then(Value::as_u64)
                        .and_then(|n| u16::try_from(n).ok())
                        .ok_or_else(|| spec_error(format!("`stdio.{name}` is not a u16")))
                };
                ChildStdio::Pty {
                    rows: size("rows")?,
                    cols: size("cols")?,
                }
            }
            _ => return Err(spec_error("`stdio.type` is not `pipes` or `pty`".into())),
        };
        let daemon = match value.get("daemon") {
            None | Some(Value::Null) => None,
            Some(daemon) => {
                let exe = daemon.get("exe").and_then(Value::as_str).ok_or_else(|| {
                    spec_error("`daemon.exe` is missing or is not a string".into())
                })?;
                let env = daemon
                    .get("env")
                    .and_then(Value::as_object)
                    .ok_or_else(|| {
                        spec_error("`daemon.env` is missing or is not an object".into())
                    })?
                    .iter()
                    .map(|(name, value)| {
                        value
                            .as_str()
                            .map(|value| (name.clone(), value.to_string()))
                            .ok_or_else(|| {
                                spec_error(format!("`daemon.env` value of {name} is not a string"))
                            })
                    })
                    .collect::<Result<BTreeMap<_, _>, _>>()?;
                Some(DaemonStart {
                    exe: exe.into(),
                    env,
                })
            }
        };
        Ok(ShimSpec {
            program: text("program")?.into(),
            args,
            env,
            cwd: text("cwd")?.into(),
            run_dir: text("run_dir")?.into(),
            stdio,
            daemon,
        })
    }
}

/// The child's pid and the time it started, as `p_starttime` gives it, recorded in the run directory. The daemon
/// reattaches by both, because a pid alone can be reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChildIdentity {
    pub pid: u32,
    /// Microseconds since the Unix epoch. Where the system cannot say (off macOS), it is the wall clock when the
    /// shim started the child.
    pub started_unix_us: u64,
}

impl ChildIdentity {
    /// The JSON of `child.json`.
    pub fn to_json(&self) -> Value {
        json!({ "pid": self.pid, "started_unix_us": self.started_unix_us })
    }

    /// Reads what [`ChildIdentity::to_json`] wrote; `None` when it is anything else.
    pub fn from_json(value: &Value) -> Option<ChildIdentity> {
        Some(ChildIdentity {
            pid: u32::try_from(value.get("pid")?.as_u64()?).ok()?,
            started_unix_us: value.get("started_unix_us")?.as_u64()?,
        })
    }
}

/// Why the shim could not do a step. Each message names the file or the call.
#[derive(Debug)]
pub enum ShimError {
    /// The child could not be started. The message names the program and the directory.
    Spawn { context: String, source: io::Error },
    /// `wait4` failed for a reason other than being interrupted.
    Reap { pid: u32, source: io::Error },
    /// A journal, log or record write failed, for example with `ENOSPC`.
    Write { context: String, source: io::Error },
    /// The spec the shim was given is not one.
    Spec { message: String },
}

impl fmt::Display for ShimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShimError::Spawn { context, source } => {
                write!(f, "could not start the child: {context}: {source}")
            }
            ShimError::Reap { pid, source } => write!(f, "wait4 on {pid} failed: {source}"),
            ShimError::Write { context, source } => write!(f, "{context}: {source}"),
            ShimError::Spec { message } => write!(f, "the shim's spec is not usable: {message}"),
        }
    }
}

impl std::error::Error for ShimError {}

fn write_error(context: impl Into<String>, source: io::Error) -> ShimError {
    ShimError::Write {
        context: context.into(),
        source,
    }
}

/// A lock that a panicking thread poisoned is still a lock: the shim keeps going and never panics because another
/// thread did.
fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The shim's `main`, for `collide shim <run-dir>`: reads `spec.json` from the run directory, starts the child, serves
/// the daemon on the shim's socket, reaps the child, writes `exit.json` and serves the record until a daemon
/// acknowledges it. Returns the process exit code of the shim itself ([`exit_code`]), which is not the child's.
pub fn run(args: &[String]) -> i32 {
    run_with(args, Limits::default())
}

/// [`run`] with the caps and waits chosen by the caller, which is how a test keeps the wait for an acknowledgement
/// short.
pub fn run_with(args: &[String], limits: Limits) -> i32 {
    let [run_dir] = args else {
        eprintln!("usage: collide shim <run-dir>");
        return exit_code::USAGE;
    };
    let spec_path = Path::new(run_dir).join(files::SPEC);
    let spec = match fs::read(&spec_path)
        .map_err(|source| write_error(format!("reading {}", spec_path.display()), source))
        .and_then(|bytes| {
            serde_json::from_slice::<Value>(&bytes).map_err(|error| ShimError::Spec {
                message: format!("{} is not JSON: {error}", spec_path.display()),
            })
        })
        .and_then(|value| ShimSpec::from_json(&value))
    {
        Ok(spec) => spec,
        Err(error) => {
            eprintln!("collide shim: {error}");
            return exit_code::USAGE;
        }
    };
    run_spec(&spec, limits)
}

/// Writes the shim's own identity ([`files::SHIM`]). A shim that cannot is still a shim: the daemon then finds it by
/// its socket alone.
fn write_own_identity(run_dir: &Path) {
    let pid = std::process::id();
    let started_unix_us =
        sysproc::start_time_us(pid).unwrap_or_else(|| SystemClocks.read(Clock::Wall).nanos / 1000);
    let identity = ChildIdentity {
        pid,
        started_unix_us,
    };
    if let Err(error) = write_file_atomically(
        run_dir,
        files::SHIM,
        identity.to_json().to_string().as_bytes(),
        |file| file,
    ) {
        eprintln!("collide shim: its own identity was not written: {error}");
    }
}

/// Starts the daemon the way a start does ([`ensure_daemon`]), with the files the spec's environment names. A failure
/// is reported on the shim's own stderr and nothing more: the record is safe on disk, and the next daemon adopts it.
fn start_daemon(start: &DaemonStart) {
    let paths = match DaemonPaths::from_env(
        start.env.get(ROOT_ENV).map(String::as_str),
        start.env.get("HOME").map(String::as_str),
    ) {
        Ok(paths) => paths,
        Err(message) => {
            eprintln!("collide shim: the daemon was not started: {message}");
            return;
        }
    };
    let mut request = StartRequest::new(start.exe.clone(), &paths);
    request.env = start.env.clone();
    request.launchctl = launchctl_from_env(
        start.env.get(ROOT_ENV).map(String::as_str),
        start.env.get(LAUNCHCTL_ENV).map(String::as_str),
    );
    match ensure_daemon(&request) {
        Ok(_) => {}
        Err(error) => eprintln!("collide shim: the daemon was not started: {error}"),
    }
}

fn run_spec(spec: &ShimSpec, limits: Limits) -> i32 {
    write_own_identity(&spec.run_dir);
    let shim = match Shim::start(spec, limits) {
        Ok(shim) => Arc::new(shim),
        Err(error) => {
            eprintln!("collide shim: {error}");
            return exit_code::FAILED;
        }
    };
    // A link that cannot be made leaves the shim with no way to be reached, and still a shim: the child runs, the
    // record is written, and the next daemon takes it from `exit.json`.
    let link = match ShimLink::start(Arc::clone(&shim), &spec.run_dir, LinkOptions::default()) {
        Ok(link) => Some(link),
        Err(error) => {
            eprintln!("collide shim: no daemon link: {error}");
            None
        }
    };
    let outcome = match shim.finish() {
        Ok(outcome) => outcome,
        Err(error) => {
            eprintln!("collide shim: {error}");
            return exit_code::FAILED;
        }
    };
    for problem in &outcome.problems {
        eprintln!("collide shim: {problem}");
    }
    let ShimOutcome {
        keeper,
        write_error,
        ..
    } = outcome;
    if let Some(error) = &write_error {
        eprintln!("collide shim: exit.json was not written: {error}");
    }
    let Some(link) = link else {
        return if write_error.is_none() {
            exit_code::OK
        } else {
            exit_code::RECORD_NOT_WRITTEN
        };
    };
    link.publish_exit(keeper);
    // A phase boundary must not wait for somebody to open the app: with no daemon connected, start one.
    if link.connections() == 0
        && let Some(start) = &spec.daemon
    {
        start_daemon(start);
    }
    // A record on disk is waited for a while; one in memory only is waited for as long as it takes.
    let wait = write_error.is_none().then_some(limits.ack_wait);
    let acknowledged = link.wait_acknowledged(wait);
    drop(link);
    if write_error.is_some() && !acknowledged {
        exit_code::RECORD_NOT_WRITTEN
    } else {
        exit_code::OK
    }
}

/// How the child's input and output are held after it started.
pub enum ChildIo {
    Pipes {
        stdin: ChildStdin,
        stdout: ChildStdout,
        stderr: ChildStderr,
    },
    Pty {
        master: File,
    },
}

/// A child that is running, and what the shim holds of it.
pub struct SpawnedChild {
    pub identity: ChildIdentity,
    pub io: ChildIo,
}

/// Starts the child with `pre_exec(setsid)`, stdin as [`ChildStdio`] says, stdout and stderr as pipes (or all three
/// the PTY's slave), and writes its [`ChildIdentity`] to the run directory. The environment is exactly `spec.env`.
///
/// The `std` handle for the process is dropped once the pipes are taken from it. That is safe: dropping it neither
/// waits nor kills, and [`reap`] is the one place the exit is taken.
pub fn spawn_child(spec: &ShimSpec) -> Result<SpawnedChild, ShimError> {
    let spawn_error = |source| ShimError::Spawn {
        context: format!("{} in {}", spec.program.display(), spec.cwd.display()),
        source,
    };
    let mut command = Command::new(&spec.program);
    command
        .args(&spec.args)
        .env_clear()
        .envs(&spec.env)
        .current_dir(&spec.cwd);
    let mut master = None;
    match spec.stdio {
        ChildStdio::Pipes => {
            command
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
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
        }
        ChildStdio::Pty { rows, cols } => {
            let (parent, slave) = open_pty(rows, cols).map_err(spawn_error)?;
            let slave_for = |fd: &OwnedFd| fd.try_clone().map(Stdio::from);
            command
                .stdin(slave_for(&slave).map_err(spawn_error)?)
                .stdout(slave_for(&slave).map_err(spawn_error)?)
                .stderr(Stdio::from(slave));
            // SAFETY: as above, with `ioctl` on descriptor 0, the slave that `Command` made stdin: it makes the PTY
            // the new session's controlling terminal, as a login shell's has.
            unsafe {
                command.pre_exec(|| {
                    if libc::setsid() == -1 {
                        return Err(io::Error::last_os_error());
                    }
                    if libc::ioctl(0, libc::TIOCSCTTY as _, 0) == -1 {
                        return Err(io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
            master = Some(parent);
        }
    }
    let mut child = command.spawn().map_err(spawn_error)?;
    // The `Command` holds the slave in the parent; the child has its own. Dropping it closes the parent's copy, so
    // the master sees the child's hangup.
    drop(command);
    let pid = child.id();
    let io = match master {
        Some(master) => ChildIo::Pty {
            master: File::from(master),
        },
        None => match (child.stdin.take(), child.stdout.take(), child.stderr.take()) {
            (Some(stdin), Some(stdout), Some(stderr)) => ChildIo::Pipes {
                stdin,
                stdout,
                stderr,
            },
            _ => {
                kill_and_reap(pid);
                return Err(spawn_error(io::Error::other(
                    "a pipe of the child is missing",
                )));
            }
        },
    };
    drop(child);
    let started_unix_us =
        sysproc::start_time_us(pid).unwrap_or_else(|| SystemClocks.read(Clock::Wall).nanos / 1000);
    let identity = ChildIdentity {
        pid,
        started_unix_us,
    };
    // A child nobody can find is worse than no child: the daemon stops it through this file.
    if let Err(error) = write_file_atomically(
        &spec.run_dir,
        files::CHILD,
        identity.to_json().to_string().as_bytes(),
        |file| file,
    ) {
        kill_and_reap(pid);
        return Err(ShimError::Spawn {
            context: "the child's identity file".to_string(),
            source: io::Error::other(error.to_string()),
        });
    }
    Ok(SpawnedChild { identity, io })
}

/// A PTY of `rows` by `cols`: the master the shim holds and the slave the child gets. Neither is inherited by what
/// the child starts, except as `Command` makes the slave descriptors 0, 1 and 2.
fn open_pty(rows: u16, cols: u16) -> io::Result<(OwnedFd, OwnedFd)> {
    let mut size = libc::winsize {
        ws_row: rows,
        ws_col: cols,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    let (mut master, mut slave) = (-1, -1);
    // SAFETY: `master` and `slave` are live `c_int`s for the call, the name and termios arguments are null (the
    // defaults), and `size` is a live `winsize`.
    let status = unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut size,
        )
    };
    if status == -1 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `openpty` succeeded, so both are open descriptors that nothing else owns.
    let (master, slave) = unsafe { (OwnedFd::from_raw_fd(master), OwnedFd::from_raw_fd(slave)) };
    for fd in [&master, &slave] {
        // SAFETY: `fd` is open for the call.
        if unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) } == -1 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok((master, slave))
}

/// Ends a child the shim cannot use and takes its exit, so nothing is left running or unreaped.
fn kill_and_reap(pid: u32) {
    if let Ok(pid) = libc::pid_t::try_from(pid) {
        // SAFETY: `kill` takes two integers. The pid is the shim's own child, not yet reaped, so it names that child.
        unsafe { libc::kill(pid, libc::SIGKILL) };
    }
    let _ = reap(pid);
}

/// Waits for `pid` with `wait4` and returns what `exit.json` holds: how it ended, and the CPU time and largest
/// resident set of it and its waited-for descendants.
pub fn reap(pid: u32) -> Result<ExitRecord, ShimError> {
    let reap_error = |source| ShimError::Reap { pid, source };
    let raw_pid = libc::pid_t::try_from(pid)
        .map_err(|_| reap_error(io::Error::new(io::ErrorKind::InvalidInput, "not a pid")))?;
    loop {
        let mut status: libc::c_int = 0;
        // SAFETY: an all-zero `rusage` is a valid one (integers only), and it is filled by the call.
        let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
        // SAFETY: `status` and `usage` are live and writable for the call; `raw_pid` names one child, so no other
        // child's exit is taken.
        let waited = unsafe { libc::wait4(raw_pid, &mut status, 0, &mut usage) };
        if waited == -1 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(reap_error(error));
        }
        let end = if libc::WIFEXITED(status) {
            ExitEnd::Exited {
                code: libc::WEXITSTATUS(status),
            }
        } else if libc::WIFSIGNALED(status) {
            ExitEnd::Signaled {
                signal: libc::WTERMSIG(status),
            }
        } else {
            // Neither happens without `WUNTRACED`, which is not asked for. If it did, the child has not ended.
            return Err(reap_error(io::Error::other(format!(
                "wait4 returned status {status:#x}, which is neither an exit nor a signal"
            ))));
        };
        return Ok(ExitRecord {
            protocol: SHIM_PROTOCOL,
            end,
            cpu_user_us: micros(usage.ru_utime),
            cpu_system_us: micros(usage.ru_stime),
            max_rss_bytes: max_rss_bytes(usage.ru_maxrss),
        });
    }
}

fn micros(time: libc::timeval) -> u64 {
    let seconds = u64::try_from(time.tv_sec).unwrap_or(0);
    let micros = u64::try_from(time.tv_usec).unwrap_or(0);
    seconds.saturating_mul(1_000_000).saturating_add(micros)
}

/// `ru_maxrss` is in bytes on macOS and in KiB on Linux.
fn max_rss_bytes(ru_maxrss: libc::c_long) -> u64 {
    let raw = u64::try_from(ru_maxrss).unwrap_or(0);
    if cfg!(target_os = "macos") {
        raw
    } else {
        raw.saturating_mul(1024)
    }
}

/// The journal of one child (0.2): one ordered file of `{seq, wall_ns, dir, bytes}` records, so replay has one order
/// that two logs could not give. It writes to any [`Write`], so a test passes a writer that fails with `ENOSPC` on
/// demand and the failing write is a chosen one (0.2, Durability).
///
/// A write that fails may leave a record cut short. The writer then refuses every later record, so the file never
/// holds a whole record after a cut one (which [`collide_core::journal::read_journal`] would call damage).
pub struct JournalWriter<W: Write> {
    sink: W,
    next_seq: u64,
    failed: bool,
}

impl<W: Write> JournalWriter<W> {
    /// A writer that starts at `seq` 0 on an empty sink.
    pub fn new(sink: W) -> Self {
        JournalWriter {
            sink,
            next_seq: 0,
            failed: false,
        }
    }

    /// The `seq` the next record gets.
    pub fn next_seq(&self) -> u64 {
        self.next_seq
    }

    /// Writes one record and flushes it, and returns its `seq`. The caller passes the bytes to the child only after
    /// this returns, so an `in` record never goes missing. `wall_ns` is a reading of the wall clock
    /// ([`crate::clock::Clock::Wall`]).
    pub fn append(&mut self, dir: Direction, wall_ns: u64, bytes: &[u8]) -> Result<u64, ShimError> {
        if self.failed {
            return Err(write_error(
                "writing the journal",
                io::Error::other("an earlier write failed, so the journal takes no more records"),
            ));
        }
        let seq = self.next_seq;
        let record = JournalRecord {
            seq,
            wall_ns,
            direction: dir,
            bytes: bytes.to_vec(),
        };
        let encoded = encode_record(&record).map_err(|too_large| {
            write_error(
                "writing the journal",
                io::Error::new(io::ErrorKind::InvalidInput, too_large.to_string()),
            )
        })?;
        if let Err(source) = self
            .sink
            .write_all(&encoded)
            .and_then(|()| self.sink.flush())
        {
            self.failed = true;
            return Err(write_error("writing the journal", source));
        }
        self.next_seq += 1;
        Ok(seq)
    }
}

/// Writes `bytes` to `dir/name` atomically ([`write_atomically`]): a temp file beside it, `fsync`, rename, and `fsync`
/// of `dir`. A write that fails removes the temp file and leaves what was at `name` whole. `wrap` is the seam a test
/// uses to make the write fail; the shim passes the file itself.
fn write_file_atomically<W: Write>(
    dir: &Path,
    name: &str,
    bytes: &[u8],
    wrap: impl FnOnce(File) -> W,
) -> Result<(), ShimError> {
    write_atomically(dir, name, bytes, 0o600, wrap).map_err(|error| {
        write_error(
            format!("{} {}", error.verb(), error.path.display()),
            error.source,
        )
    })
}

/// Writes `record` to `dir/exit.json` atomically: a temp file, `fsync`, rename, and `fsync` of `dir`. A file that
/// cannot be written leaves the old one whole, and no partial file.
pub fn write_exit_record(dir: &Path, record: &ExitRecord) -> Result<(), ShimError> {
    write_exit_record_through(dir, record, |file| file)
}

fn write_exit_record_through<W: Write>(
    dir: &Path,
    record: &ExitRecord,
    wrap: impl FnOnce(File) -> W,
) -> Result<(), ShimError> {
    write_file_atomically(dir, files::EXIT_RECORD, &exit_record_bytes(record), wrap)
}

/// What the shim has of its child's exit: the record, held in memory until the daemon acknowledges it, and the error
/// if `exit.json` could not be written.
#[derive(Debug)]
pub struct ExitOutcome {
    pub keeper: ExitKeeper,
    pub write_error: Option<ShimError>,
}

/// Takes the record into an [`ExitKeeper`] and writes `exit.json`. A write that fails (a full disk) does not lose the
/// record: the keeper still has it.
pub fn record_exit(dir: &Path, record: ExitRecord) -> ExitOutcome {
    let write_error = write_exit_record(dir, &record).err();
    ExitOutcome {
        keeper: ExitKeeper::new(record),
        write_error,
    }
}

/// The exit record held in memory until the daemon acknowledges it. When [`write_exit_record`] fails on a full disk,
/// the shim keeps serving the record from here, and sends it again until [`collide_core::shim::DaemonMessage::AckExit`] arrives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitKeeper {
    record: ExitRecord,
    acknowledged: bool,
}

impl ExitKeeper {
    pub fn new(record: ExitRecord) -> Self {
        ExitKeeper {
            record,
            acknowledged: false,
        }
    }

    /// The record the shim sends: [`ShimMessage::Exited`], until the daemon has acknowledged it.
    pub fn pending_message(&self) -> Option<ShimMessage> {
        (!self.acknowledged).then(|| ShimMessage::Exited {
            exit: self.record.clone(),
        })
    }

    /// The daemon has the record: the shim may stop serving it and exit.
    pub fn acknowledge(&mut self) {
        self.acknowledged = true;
    }

    /// The record, whether or not it was acknowledged.
    pub fn record(&self) -> &ExitRecord {
        &self.record
    }
}

/// The last [`PTY_RING_BYTES`] of a PTY's output. On attach the shim replays it and subscribes to new output in one
/// step, so a view opened after an app restart shows the session so far with no gap and no repeat (measured S2).
/// The ring is the replay; the subscription is the daemon link's, which holds the ring's lock across both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PtyRing {
    capacity: usize,
    bytes: Vec<u8>,
}

impl PtyRing {
    /// An empty ring that keeps `capacity` bytes. The shim uses [`PTY_RING_BYTES`].
    pub fn new(capacity: usize) -> Self {
        PtyRing {
            capacity,
            bytes: Vec::new(),
        }
    }

    /// Adds output, dropping the oldest bytes past the capacity.
    pub fn push(&mut self, output: &[u8]) {
        if output.len() >= self.capacity {
            self.bytes.clear();
            self.bytes
                .extend_from_slice(&output[output.len() - self.capacity..]);
            return;
        }
        self.bytes.extend_from_slice(output);
        if self.bytes.len() > self.capacity {
            let excess = self.bytes.len() - self.capacity;
            self.bytes.drain(..excess);
        }
    }

    /// What the ring holds, oldest first, for the replay on attach.
    pub fn snapshot(&self) -> Vec<u8> {
        self.bytes.clone()
    }
}

/// Cuts a byte stream into lines of at most `cap` bytes of content. A longer line is cut and marked
/// ([`line_cut_marker`]); the rest of it is counted and dropped as it arrives, so one runaway line costs `cap` bytes
/// of memory and no more. Each line it returns ends in its newline, except a last line the stream ended without one.
#[derive(Debug)]
struct LineSplitter {
    cap: usize,
    line: Vec<u8>,
    dropped: usize,
}

impl LineSplitter {
    fn new(cap: usize) -> Self {
        LineSplitter {
            cap,
            line: Vec::new(),
            dropped: 0,
        }
    }

    /// Takes the bytes of one read and adds the lines it completed to `out`.
    fn push(&mut self, chunk: &[u8], out: &mut Vec<Vec<u8>>) {
        let mut rest = chunk;
        while let Some(newline) = rest.iter().position(|&byte| byte == b'\n') {
            self.add(&rest[..newline]);
            let mut line = self.take();
            line.push(b'\n');
            out.push(line);
            rest = &rest[newline + 1..];
        }
        self.add(rest);
    }

    /// The line the stream ended in the middle of, if there is one.
    fn finish(&mut self) -> Option<Vec<u8>> {
        (!self.line.is_empty() || self.dropped > 0).then(|| self.take())
    }

    fn add(&mut self, content: &[u8]) {
        let room = self.cap.saturating_sub(self.line.len());
        let kept = content.len().min(room);
        self.line.extend_from_slice(&content[..kept]);
        self.dropped += content.len() - kept;
    }

    /// The line so far, with the marker if it was cut, and the splitter ready for the next one.
    fn take(&mut self) -> Vec<u8> {
        let mut line = std::mem::take(&mut self.line);
        if self.dropped > 0 {
            line.extend_from_slice(line_cut_marker(self.dropped).as_bytes());
            self.dropped = 0;
        }
        line
    }
}

/// Where a [`CappedLog`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LogState {
    Open,
    /// The cap was reached and the marker written. Later output is dropped.
    Full,
    /// A write failed. Later output is dropped, so the file never holds a record after a cut one.
    Failed,
}

/// A log file with a cap. At the cap it writes [`log_full_marker`] and stops growing; after a failed write it stops
/// too. Output it drops is dropped quietly: the first failure is the one that is reported.
struct CappedLog<W: Write> {
    sink: W,
    cap: u64,
    written: u64,
    state: LogState,
}

impl<W: Write> CappedLog<W> {
    fn new(sink: W, cap: u64) -> Self {
        CappedLog {
            sink,
            cap,
            written: 0,
            state: LogState::Open,
        }
    }

    /// Writes `bytes`, which are a line (or a chunk of PTY output). One that would pass the cap is not written, and
    /// the marker is written in its place.
    fn write(&mut self, bytes: &[u8]) -> Result<(), ShimError> {
        if self.state != LogState::Open {
            return Ok(());
        }
        let len = bytes.len() as u64;
        let (to_write, next_state) = if self.written.saturating_add(len) > self.cap {
            (log_full_marker(self.cap).into_bytes(), LogState::Full)
        } else {
            (bytes.to_vec(), LogState::Open)
        };
        if let Err(source) = self.sink.write_all(&to_write) {
            self.state = LogState::Failed;
            return Err(write_error("writing a log", source));
        }
        self.written += to_write.len() as u64;
        self.state = next_state;
        Ok(())
    }

    fn flush(&mut self) -> Result<(), ShimError> {
        if self.state == LogState::Failed {
            return Ok(());
        }
        self.sink.flush().map_err(|source| {
            self.state = LogState::Failed;
            write_error("writing a log", source)
        })
    }
}

/// The first failure of each sink, by name, for [`Shim::problems`].
type Problems = Arc<Mutex<BTreeMap<&'static str, String>>>;

fn note_problem(problems: &Problems, source: &'static str, error: &ShimError) {
    locked(problems)
        .entry(source)
        .or_insert_with(|| error.to_string());
}

/// What the live session is told of each line of the child's stdout, after the line is in the journal: the line, with
/// its newline, and the wall clock the journal stored for it.
pub type LineHook = Box<dyn FnMut(&[u8], u64) + Send>;

/// The writers a shim writes to. [`Sinks::files`] opens the run directory's; a test passes a writer that fails with
/// `ENOSPC` on demand in place of any of them.
pub struct Sinks {
    pub journal: Box<dyn Write + Send>,
    pub stdout_log: Box<dyn Write + Send>,
    pub stderr_log: Box<dyn Write + Send>,
    /// Called for each stdout line after it is journaled. The daemon link feeds the live session from it.
    pub on_line: Option<LineHook>,
}

impl Sinks {
    /// The journal and the two logs of `run_dir`, each made new with mode `0600`: a file that is already there is an
    /// error and is never written over.
    pub fn files(run_dir: &Path) -> Result<Sinks, ShimError> {
        let create = |name: &str| {
            let path = run_dir.join(name);
            OpenOptions::new()
                .append(true)
                .create_new(true)
                .mode(0o600)
                .open(&path)
                .map_err(|source| write_error(format!("creating {}", path.display()), source))
        };
        Ok(Sinks {
            journal: Box::new(create(files::JOURNAL)?),
            stdout_log: Box::new(BufWriter::new(create(files::STDOUT_LOG)?)),
            stderr_log: Box::new(BufWriter::new(create(files::STDERR_LOG)?)),
            on_line: None,
        })
    }
}

type SharedJournal = Arc<Mutex<JournalWriter<Box<dyn Write + Send>>>>;

/// Reads a pipe, cuts it into lines and sends each to the journal (stdout only), the hook (stdout only) and a log.
struct LinePump<R: Read> {
    reader: R,
    splitter: LineSplitter,
    log: CappedLog<Box<dyn Write + Send>>,
    /// The journal and the hook, for stdout. Stderr has neither.
    journal: Option<SharedJournal>,
    hook: Option<LineHook>,
    log_name: &'static str,
    problems: Problems,
}

impl<R: Read> LinePump<R> {
    fn run(mut self) {
        let mut buffer = vec![0u8; READ_CHUNK_BYTES];
        let mut lines = Vec::new();
        loop {
            let read = match self.reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => read,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(source) => {
                    let error = write_error("reading the child's output", source);
                    note_problem(&self.problems, self.log_name, &error);
                    break;
                }
            };
            let wall_ns = SystemClocks.read(Clock::Wall).nanos;
            self.splitter.push(&buffer[..read], &mut lines);
            for line in lines.drain(..) {
                self.line(&line, wall_ns);
            }
            self.flush();
        }
        if let Some(last) = self.splitter.finish() {
            self.line(&last, SystemClocks.read(Clock::Wall).nanos);
        }
        self.flush();
    }

    fn line(&mut self, line: &[u8], wall_ns: u64) {
        if let Some(journal) = &self.journal {
            let appended = locked(journal).append(Direction::Out, wall_ns, line);
            if let Err(error) = appended {
                note_problem(&self.problems, "journal", &error);
            }
        }
        if let Some(hook) = &mut self.hook {
            hook(line, wall_ns);
        }
        if let Err(error) = self.log.write(line) {
            note_problem(&self.problems, self.log_name, &error);
        }
    }

    fn flush(&mut self) {
        if let Err(error) = self.log.flush() {
            note_problem(&self.problems, self.log_name, &error);
        }
    }
}

/// Reads a PTY's master into the ring and the log.
fn pump_pty(
    mut master: File,
    ring: Arc<Mutex<PtyRing>>,
    mut log: CappedLog<Box<dyn Write + Send>>,
    problems: Problems,
) {
    let mut buffer = vec![0u8; READ_CHUNK_BYTES];
    loop {
        let read = match master.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            // Once the child and everything holding the slave have gone, the master reads `EIO`: the end.
            Err(error) if error.raw_os_error() == Some(libc::EIO) => break,
            Err(source) => {
                let error = write_error("reading the PTY", source);
                note_problem(&problems, "pty", &error);
                break;
            }
        };
        locked(&ring).push(&buffer[..read]);
        if let Err(error) = log.write(&buffer[..read]).and_then(|()| log.flush()) {
            note_problem(&problems, "stdout log", &error);
        }
    }
}

/// A thread that copies one stream, and the means to wait for it with a timeout.
struct Pump {
    done: mpsc::Receiver<()>,
    _handle: JoinHandle<()>,
}

fn start_pump(name: &str, body: impl FnOnce() + Send + 'static) -> Result<Pump, ShimError> {
    let (sender, done) = mpsc::channel();
    let handle = thread::Builder::new()
        .name(format!("shim-{name}"))
        .spawn(move || {
            body();
            let _ = sender.send(());
        })
        .map_err(|source| ShimError::Spawn {
            context: format!("the {name} thread"),
            source,
        })?;
    Ok(Pump {
        done,
        _handle: handle,
    })
}

/// How the shim writes to the child.
enum ChildInput {
    /// `None` once stdin is closed.
    Pipe(Option<ChildStdin>),
    Pty(File),
}

/// A running shim: the child, and what the shim does with its traffic until [`Shim::finish`].
///
/// Everything here is `&self` and thread-safe, so the daemon link can serve a socket and the pumps run beside it.
pub struct Shim {
    identity: ChildIdentity,
    run_dir: PathBuf,
    drain_after_exit: Duration,
    /// Held across a write to the child, so records and bytes reach it in one order.
    input: Mutex<ChildInput>,
    journal: SharedJournal,
    ring: Arc<Mutex<PtyRing>>,
    problems: Problems,
    /// Behind a lock because a pump's channel is not `Sync`, and the shim is shared with the link's threads.
    pumps: Mutex<Vec<Pump>>,
    /// Set once [`reap`] has taken the child's exit, so a signal is not sent for a child that is gone.
    reaped: AtomicBool,
}

impl Shim {
    /// Starts the child of `spec` with the run directory's files ([`Sinks::files`]). The files are made before the
    /// child, so a directory that cannot take them starts nothing.
    pub fn start(spec: &ShimSpec, limits: Limits) -> Result<Shim, ShimError> {
        let sinks = Sinks::files(&spec.run_dir)?;
        Shim::start_with(spec, limits, sinks)
    }

    /// [`Shim::start`] with the writers chosen by the caller.
    pub fn start_with(spec: &ShimSpec, limits: Limits, sinks: Sinks) -> Result<Shim, ShimError> {
        let Sinks {
            journal,
            stdout_log,
            stderr_log,
            on_line,
        } = sinks;
        let spawned = spawn_child(spec)?;
        let pid = spawned.identity.pid;
        let journal: SharedJournal = Arc::new(Mutex::new(JournalWriter::new(journal)));
        let ring = Arc::new(Mutex::new(PtyRing::new(PTY_RING_BYTES)));
        let problems = Problems::default();
        let stdout_log = CappedLog::new(stdout_log, limits.log_bytes);
        let started = match spawned.io {
            ChildIo::Pipes {
                stdin,
                stdout,
                stderr,
            } => {
                let stdout_pump = LinePump {
                    reader: stdout,
                    splitter: LineSplitter::new(limits.line_bytes),
                    log: stdout_log,
                    journal: Some(Arc::clone(&journal)),
                    hook: on_line,
                    log_name: "stdout log",
                    problems: Arc::clone(&problems),
                };
                let stderr_pump = LinePump {
                    reader: stderr,
                    splitter: LineSplitter::new(limits.line_bytes),
                    log: CappedLog::new(stderr_log, limits.log_bytes),
                    journal: None,
                    hook: None,
                    log_name: "stderr log",
                    problems: Arc::clone(&problems),
                };
                start_pump("stdout", move || stdout_pump.run())
                    .and_then(|first| {
                        start_pump("stderr", move || stderr_pump.run())
                            .map(|second| vec![first, second])
                    })
                    .map(|pumps| (ChildInput::Pipe(Some(stdin)), pumps))
            }
            ChildIo::Pty { master } => master
                .try_clone()
                .map_err(|source| ShimError::Spawn {
                    context: "the PTY master".to_string(),
                    source,
                })
                .and_then(|reader| {
                    let ring = Arc::clone(&ring);
                    let problems = Arc::clone(&problems);
                    start_pump("pty", move || pump_pty(reader, ring, stdout_log, problems))
                })
                .map(|pump| (ChildInput::Pty(master), vec![pump])),
        };
        match started {
            Ok((input, pumps)) => Ok(Shim {
                identity: spawned.identity,
                run_dir: spec.run_dir.clone(),
                drain_after_exit: limits.drain_after_exit,
                input: Mutex::new(input),
                journal,
                ring,
                problems,
                pumps: Mutex::new(pumps),
                reaped: AtomicBool::new(false),
            }),
            Err(error) => {
                kill_and_reap(pid);
                Err(error)
            }
        }
    }

    /// The child's pid and start time, as `child.json` holds them.
    pub fn identity(&self) -> ChildIdentity {
        self.identity
    }

    /// Writes `bytes` to the child's stdin (or the PTY): the journal record first, the child second, so an `in`
    /// record never goes missing. Returns the `seq` of the last record, which is what [`ShimMessage::Wrote`] carries.
    /// A journal that cannot take the record fails the write and the child gets nothing.
    pub fn write_to_child(&self, bytes: &[u8]) -> Result<u64, ShimError> {
        if bytes.is_empty() {
            return Err(write_error(
                "writing to the child",
                io::Error::new(io::ErrorKind::InvalidInput, "there are no bytes to write"),
            ));
        }
        let mut input = locked(&self.input);
        let mut last_seq = 0;
        for part in bytes.chunks(MAX_RECORD_BYTES) {
            let wall_ns = SystemClocks.read(Clock::Wall).nanos;
            let appended = locked(&self.journal).append(Direction::In, wall_ns, part);
            last_seq = match appended {
                Ok(seq) => seq,
                Err(error) => {
                    note_problem(&self.problems, "journal", &error);
                    return Err(error);
                }
            };
            input
                .write_all(part)
                .map_err(|source| write_error("writing to the child", source))?;
        }
        Ok(last_seq)
    }

    /// Closes the child's stdin pipe. A PTY has no stdin to close, and that is an error.
    pub fn close_stdin(&self) -> Result<(), ShimError> {
        match &mut *locked(&self.input) {
            ChildInput::Pipe(stdin) => {
                *stdin = None;
                Ok(())
            }
            ChildInput::Pty(_) => Err(write_error(
                "closing the child's stdin",
                io::Error::new(io::ErrorKind::Unsupported, "a PTY has no stdin to close"),
            )),
        }
    }

    /// Sends `signal` to the child's process group. The child is a session leader (the shim made it one), so its group
    /// is its pid, and the group is what is signalled: a pid alone could name something else once the child is gone,
    /// and a group with no member left cannot. A child that has been reaped is refused.
    pub fn signal_child(&self, signal: StopSignal) -> Result<(), ShimError> {
        let fail = |source| write_error("signalling the child", source);
        if self.reaped.load(Ordering::SeqCst) {
            return Err(fail(io::Error::other("the child has ended")));
        }
        let number = match signal {
            StopSignal::Hup => libc::SIGHUP,
            StopSignal::Term => libc::SIGTERM,
            StopSignal::Kill => libc::SIGKILL,
        };
        let group = libc::pid_t::try_from(self.identity.pid)
            .map_err(|_| fail(io::Error::new(io::ErrorKind::InvalidInput, "not a pid")))?;
        // SAFETY: `kill` takes two integers. A negative pid names the process group with that id, which is the
        // child's own group.
        if unsafe { libc::kill(-group, number) } == -1 {
            return Err(fail(io::Error::last_os_error()));
        }
        Ok(())
    }

    /// What the PTY ring holds, for the replay on attach. Empty for a pipe child.
    pub fn ring_snapshot(&self) -> Vec<u8> {
        locked(&self.ring).snapshot()
    }

    /// The first failure of each sink so far, as `name: message`. The daemon link reports them as
    /// [`ShimMessage::Failed`].
    pub fn problems(&self) -> Vec<String> {
        locked(&self.problems)
            .iter()
            .map(|(name, message)| format!("{name}: {message}"))
            .collect()
    }

    /// Waits for the child with [`reap`], lets the pumps drain what is left in the pipes (for at most
    /// [`Limits::drain_after_exit`]), and writes `exit.json`, so the journal and the logs are complete before the
    /// record that says the child is done. An error is a `wait4` that failed; a failed `exit.json` is in the outcome.
    pub fn finish(&self) -> Result<ShimOutcome, ShimError> {
        let record = reap(self.identity.pid)?;
        self.reaped.store(true, Ordering::SeqCst);
        let deadline = Instant::now() + self.drain_after_exit;
        for pump in locked(&self.pumps).iter() {
            let wait = deadline.saturating_duration_since(Instant::now());
            // A pump that ended and one that panicked both disconnect the channel; only a timeout means it is still
            // reading, and then the record does not wait for it.
            let _ = pump.done.recv_timeout(wait);
        }
        let problems = self.problems();
        let ExitOutcome {
            keeper,
            write_error,
        } = record_exit(&self.run_dir, record);
        Ok(ShimOutcome {
            keeper,
            write_error,
            problems,
        })
    }
}

/// What a shim has when its child is done.
#[derive(Debug)]
pub struct ShimOutcome {
    /// The exit record, served until the daemon acknowledges it.
    pub keeper: ExitKeeper,
    /// Why `exit.json` is not on disk, if it is not. The keeper still has the record.
    pub write_error: Option<ShimError>,
    /// The first failure of each sink ([`Shim::problems`]).
    pub problems: Vec<String>,
}

impl ChildInput {
    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        match self {
            ChildInput::Pipe(Some(stdin)) => stdin.write_all(bytes).and_then(|()| stdin.flush()),
            ChildInput::Pipe(None) => Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "the child's stdin is closed",
            )),
            ChildInput::Pty(master) => master.write_all(bytes),
        }
    }
}

// ---- the daemon link --------------------------------------------------------------------------------------------

/// What a [`ShimLink`] is made with. [`LinkOptions::default`] is what a running shim uses; a test passes its own.
#[derive(Debug, Clone, Copy)]
pub struct LinkOptions {
    /// The uid a connecting peer must have: the shim's own, because the daemon is the same user's.
    pub uid: u32,
    /// How long a read on a connection waits before the thread looks at whether it should end.
    pub poll: Duration,
    /// How long a connection has, after the `Hello`, to answer with `Welcome`.
    pub handshake_wait: Duration,
    /// How long a write to a connection may block.
    pub write_wait: Duration,
}

impl Default for LinkOptions {
    fn default() -> Self {
        LinkOptions {
            uid: sysproc::daemon_uid(),
            poll: LINK_POLL,
            handshake_wait: HANDSHAKE_WAIT,
            write_wait: LINK_WRITE_WAIT,
        }
    }
}

/// What the link's threads share.
struct LinkShared {
    shim: Arc<Shim>,
    options: LinkOptions,
    stopping: AtomicBool,
    /// Threads serving a connection.
    open: AtomicUsize,
    /// Connections that have passed the handshake.
    welcomed: AtomicUsize,
    /// The record, once the child is reaped, until a daemon acknowledges it.
    exit: Mutex<Option<ExitKeeper>>,
    acknowledged: Condvar,
}

impl LinkShared {
    fn pending_exit(&self) -> Option<ShimMessage> {
        locked(&self.exit)
            .as_ref()
            .and_then(ExitKeeper::pending_message)
    }

    /// A daemon has the record. Only a record that was published can be acknowledged.
    fn acknowledge(&self) -> bool {
        let mut exit = locked(&self.exit);
        match exit.as_mut() {
            Some(keeper) => {
                keeper.acknowledge();
                self.acknowledged.notify_all();
                true
            }
            None => false,
        }
    }
}

/// Counts a thread in `counter` for as long as the guard lives.
struct Counted<'a>(&'a AtomicUsize);

impl<'a> Counted<'a> {
    fn new(counter: &'a AtomicUsize) -> Counted<'a> {
        counter.fetch_add(1, Ordering::SeqCst);
        Counted(counter)
    }
}

impl Drop for Counted<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// The shim's socket and the threads that serve it (`shim: daemon link`): one accepts, and one serves each
/// connection, so a daemon that says nothing or does not read holds only its own thread. The socket is `shim/s` in a
/// directory of mode `0700` under the run directory, checked as the daemon's own is ([`auth::prepare_shim_socket`]),
/// and a peer whose uid is not [`LinkOptions::uid`] is closed before anything is said. Dropping the link stops the
/// threads and removes the socket and its directory.
pub struct ShimLink {
    shared: Arc<LinkShared>,
    accept: Option<JoinHandle<()>>,
    socket: PathBuf,
}

impl ShimLink {
    /// Checks the socket's path and directory, binds, and starts accepting.
    pub fn start(
        shim: Arc<Shim>,
        run_dir: &Path,
        options: LinkOptions,
    ) -> Result<ShimLink, ShimError> {
        let socket =
            auth::prepare_shim_socket(&StdFs, sysproc::daemon_uid(), run_dir).map_err(|error| {
                write_error(
                    format!("the shim's socket in {}", run_dir.display()),
                    io::Error::other(error.to_string()),
                )
            })?;
        let bind = |what: &str, source| write_error(format!("{what} {}", socket.display()), source);
        // A socket file left by a shim that died is this directory's own: nobody else serves it.
        match fs::remove_file(&socket) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(source) => return Err(bind("removing the old socket", source)),
        }
        let listener =
            UnixListener::bind(&socket).map_err(|source| bind("binding the socket", source))?;
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o600))
            .map_err(|source| bind("setting the mode of the socket", source))?;
        listener
            .set_nonblocking(true)
            .map_err(|source| bind("making the listener non-blocking", source))?;
        let shared = Arc::new(LinkShared {
            shim,
            options,
            stopping: AtomicBool::new(false),
            open: AtomicUsize::new(0),
            welcomed: AtomicUsize::new(0),
            exit: Mutex::new(None),
            acknowledged: Condvar::new(),
        });
        let serving = Arc::clone(&shared);
        let accept = thread::Builder::new()
            .name("shim-link-accept".to_string())
            .spawn(move || accept_connections(&serving, &listener))
            .map_err(|source| bind("starting the accept thread for", source))?;
        Ok(ShimLink {
            shared,
            accept: Some(accept),
            socket,
        })
    }

    /// The socket's path.
    pub fn socket(&self) -> &Path {
        &self.socket
    }

    /// How many daemon connections have passed the handshake and are open.
    pub fn connections(&self) -> usize {
        self.shared.welcomed.load(Ordering::SeqCst)
    }

    /// Makes the exit record available: each connection is sent it, one that comes later is sent it on arrival, and
    /// every one is sent it again until a daemon acknowledges it.
    pub fn publish_exit(&self, keeper: ExitKeeper) {
        *locked(&self.shared.exit) = Some(keeper);
    }

    /// Waits until a daemon has acknowledged the published record. `None` waits for as long as it takes. Returns
    /// whether it was acknowledged.
    pub fn wait_acknowledged(&self, within: Option<Duration>) -> bool {
        let until = within.map(|within| Instant::now() + within);
        let mut exit = locked(&self.shared.exit);
        loop {
            if exit
                .as_ref()
                .is_some_and(|keeper| keeper.pending_message().is_none())
            {
                return true;
            }
            let slice = match until {
                Some(until) => {
                    let left = until.saturating_duration_since(Instant::now());
                    if left.is_zero() {
                        return false;
                    }
                    left.min(LINK_POLL)
                }
                None => LINK_POLL,
            };
            exit = self
                .shared
                .acknowledged
                .wait_timeout(exit, slice)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }
}

impl Drop for ShimLink {
    fn drop(&mut self) {
        self.shared.stopping.store(true, Ordering::SeqCst);
        if let Some(accept) = self.accept.take() {
            let _ = accept.join();
        }
        let _ = fs::remove_file(&self.socket);
        if let Some(dir) = self.socket.parent() {
            let _ = fs::remove_dir(dir);
        }
    }
}

/// Accepts until the link stops, a thread for each connection.
fn accept_connections(shared: &Arc<LinkShared>, listener: &UnixListener) {
    while !shared.stopping.load(Ordering::SeqCst) {
        let mut poll = libc::pollfd {
            fd: listener.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let wait_ms = i32::try_from(shared.options.poll.as_millis()).unwrap_or(i32::MAX);
        // SAFETY: `poll` points at one `pollfd` that lives for the call, and the count says so.
        let ready = unsafe { libc::poll(&mut poll, 1, wait_ms) };
        if ready <= 0 {
            continue;
        }
        match listener.accept() {
            Ok((stream, _)) => {
                if shared.open.load(Ordering::SeqCst) >= MAX_LINK_CONNECTIONS {
                    eprintln!(
                        "collide shim: a connection was refused: {MAX_LINK_CONNECTIONS} are open"
                    );
                    continue;
                }
                let serving = Arc::clone(shared);
                let started = thread::Builder::new()
                    .name("shim-link-connection".to_string())
                    .spawn(move || serve_connection(&serving, stream));
                if let Err(error) = started {
                    eprintln!("collide shim: could not start a connection's thread: {error}");
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => {
                eprintln!("collide shim: accept failed: {error}");
                thread::sleep(shared.options.poll);
            }
        }
    }
}

/// One daemon connection: the uid check, the shim's `Hello`, the daemon's `Welcome`, then each message until the
/// connection ends or the link stops. Everything is written by this thread, so a frame is never interleaved with
/// another.
fn serve_connection(shared: &LinkShared, stream: UnixStream) {
    let _open = Counted::new(&shared.open);
    if let Err(refused) = auth::admit_shim(shared.options.uid, &UnixPeer { stream: &stream }) {
        eprintln!("collide shim: a connection was refused: {refused}");
        return;
    }
    let setup = stream
        .set_nonblocking(false)
        .and_then(|()| stream.set_write_timeout(Some(shared.options.write_wait)))
        .and_then(|()| stream.try_clone());
    let mut writer = match setup {
        Ok(writer) => writer,
        Err(error) => {
            eprintln!("collide shim: a connection could not be set up: {error}");
            return;
        }
    };
    let mut reader = FrameReader::new(stream, shared.options.poll);
    if send(
        &mut writer,
        &ShimMessage::Hello {
            protocol: SHIM_PROTOCOL,
        },
    )
    .is_err()
    {
        return;
    }
    if !await_welcome(shared, &mut reader) {
        return;
    }
    let _welcomed = Counted::new(&shared.welcomed);
    let mut exit_sent = false;
    let mut problems_sent: Vec<String> = Vec::new();
    loop {
        if !exit_sent && let Some(message) = shared.pending_exit() {
            if send(&mut writer, &message).is_err() {
                return;
            }
            exit_sent = true;
        }
        for problem in shared.shim.problems() {
            if !problems_sent.contains(&problem) {
                let notice = ShimMessage::Failed {
                    message: format!("{PROBLEM_PREFIX}{problem}"),
                };
                if send(&mut writer, &notice).is_err() {
                    return;
                }
                problems_sent.push(problem);
            }
        }
        match reader.read_next::<DaemonMessage>() {
            Ok(Next::Message(message)) => {
                if !serve_message(shared, &mut writer, message) {
                    return;
                }
            }
            Ok(Next::Idle) => {
                if shared.stopping.load(Ordering::SeqCst) {
                    return;
                }
            }
            Ok(Next::Closed { dropped }) => {
                if dropped > 0 {
                    eprintln!(
                        "collide shim: a daemon went away in the middle of a message; its {dropped} bytes were dropped"
                    );
                }
                return;
            }
            Err(error) => {
                eprintln!("collide shim: a connection was closed: {error}");
                return;
            }
        }
    }
}

/// Waits for the daemon's `Welcome`. Anything else, a close, or no answer in [`LinkOptions::handshake_wait`] ends the
/// connection.
fn await_welcome(shared: &LinkShared, reader: &mut FrameReader) -> bool {
    let until = Instant::now() + shared.options.handshake_wait;
    loop {
        match reader.read_next::<DaemonMessage>() {
            Ok(Next::Message(DaemonMessage::Welcome { protocol })) if protocol == SHIM_PROTOCOL => {
                return true;
            }
            Ok(Next::Message(DaemonMessage::Refused { message })) => {
                eprintln!("collide shim: a daemon refused this shim: {message}");
                return false;
            }
            Ok(Next::Message(other)) => {
                eprintln!(
                    "collide shim: a connection's first message was {other:?}, not a welcome"
                );
                return false;
            }
            Ok(Next::Idle) => {
                if shared.stopping.load(Ordering::SeqCst) || Instant::now() >= until {
                    return false;
                }
            }
            Ok(Next::Closed { .. }) | Err(_) => return false,
        }
    }
}

/// Does what one message asks. `false` ends the connection.
fn serve_message(shared: &LinkShared, writer: &mut UnixStream, message: DaemonMessage) -> bool {
    let failed = |error: ShimError| ShimMessage::Failed {
        message: error.to_string(),
    };
    let answer = match message {
        DaemonMessage::Write { data } => Some(match shared.shim.write_to_child(data.as_bytes()) {
            Ok(seq) => ShimMessage::Wrote { seq },
            Err(error) => failed(error),
        }),
        DaemonMessage::CloseStdin {} => shared.shim.close_stdin().err().map(failed),
        DaemonMessage::Signal { signal } => shared.shim.signal_child(signal).err().map(failed),
        DaemonMessage::AckExit {} => {
            if !shared.acknowledge() {
                eprintln!(
                    "collide shim: an acknowledgement came before the child ended, and was ignored"
                );
            }
            None
        }
        DaemonMessage::Welcome { .. } | DaemonMessage::Refused { .. } => {
            eprintln!("collide shim: a second handshake message closed a connection");
            return false;
        }
    };
    match answer {
        Some(answer) => send(writer, &answer).is_ok(),
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ENOSPC is 28 on macOS and on Linux.
    const ENOSPC: i32 = 28;

    fn enospc() -> io::Error {
        io::Error::from_raw_os_error(ENOSPC)
    }

    /// Takes `ok_writes` writes whole, then fails every write with `ENOSPC`. A failing write takes `partial` bytes of
    /// its buffer first, as a disk that fills up in the middle of a write does.
    #[derive(Clone)]
    struct FullDisk {
        ok_writes: usize,
        partial: usize,
        state: Arc<Mutex<(usize, Vec<u8>)>>,
    }

    impl FullDisk {
        fn after(ok_writes: usize) -> FullDisk {
            FullDisk::cutting(ok_writes, 0)
        }

        fn cutting(ok_writes: usize, partial: usize) -> FullDisk {
            FullDisk {
                ok_writes,
                partial,
                state: Arc::default(),
            }
        }

        fn taken(&self) -> Vec<u8> {
            locked(&self.state).1.clone()
        }
    }

    impl Write for FullDisk {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let mut state = locked(&self.state);
            if state.0 > self.ok_writes {
                return Err(enospc());
            }
            if state.0 == self.ok_writes {
                state.0 += 1;
                let part = self.partial.min(buf.len());
                state.1.extend_from_slice(&buf[..part]);
                return if part > 0 { Ok(part) } else { Err(enospc()) };
            }
            state.0 += 1;
            state.1.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// Fails on the first write, with `ENOSPC`.
    struct Fails;

    impl Write for Fails {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(enospc())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

    /// A folder made for one test, in the system's temp folder. `Drop` removes the files in it by their own paths
    /// and then the folder (never a recursive delete).
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> TempDir {
            let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let dir = std::env::temp_dir().join(format!("collide-shim-{}-{n}", std::process::id()));
            fs::create_dir_all(&dir).unwrap();
            TempDir(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            if let Ok(entries) = fs::read_dir(&self.0) {
                for entry in entries.flatten() {
                    let _ = fs::remove_file(entry.path());
                }
            }
            let _ = fs::remove_dir(&self.0);
        }
    }

    fn record(code: i32) -> ExitRecord {
        ExitRecord {
            protocol: SHIM_PROTOCOL,
            end: ExitEnd::Exited { code },
            cpu_user_us: 10,
            cpu_system_us: 5,
            max_rss_bytes: 4096,
        }
    }

    fn lines_of(splitter: &mut LineSplitter, chunks: &[&[u8]]) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        for chunk in chunks {
            splitter.push(chunk, &mut out);
        }
        out.extend(splitter.finish());
        out
    }

    // ---- constants and names -------------------------------------------------------------------------------

    #[test]
    fn the_pty_ring_keeps_one_mebibyte() {
        assert_eq!(PTY_RING_BYTES, 1_048_576);
    }

    #[test]
    fn the_exit_record_is_named_as_core_writes_it() {
        assert_eq!(files::EXIT_RECORD, "exit.json");
    }

    #[test]
    fn the_line_cap_fits_in_one_journal_record_with_its_marker() {
        assert!(LINE_CAP_BYTES + line_cut_marker(usize::MAX).len() < MAX_RECORD_BYTES);
    }

    #[test]
    fn a_write_error_names_the_context_and_the_cause() {
        let message = ShimError::Write {
            context: "journal /r/journal".to_string(),
            source: io::Error::other("no space left on device"),
        }
        .to_string();
        assert!(message.contains("/r/journal") && message.contains("no space left"));
    }

    // ---- the exit is taken from wait4 ----------------------------------------------------------------------

    /// The source of this module without its tests.
    fn production_source() -> &'static str {
        let source = include_str!("shim.rs");
        let tests = source
            .find("#[cfg(test)]\nmod tests")
            .expect("the tests module is at the end");
        &source[..tests]
    }

    #[test]
    fn the_shim_never_takes_an_exit_from_std_or_tokio() {
        let source = production_source();
        for call in [
            ".wait(",
            ".try_wait(",
            ".wait_with_output(",
            ".status(",
            ".output(",
            "tokio::",
        ] {
            assert!(
                !source.contains(call),
                "shim.rs calls `{call}`; an exit is taken with wait4 in `reap` and nowhere else"
            );
        }
        assert_eq!(source.matches("libc::wait4(").count(), 1);
    }

    // ---- wait4 and the child that is not usable --------------------------------------------------------------

    #[test]
    fn a_pid_that_is_not_the_shims_child_is_a_reap_error() {
        // Pid 1 is nobody's child but the kernel's: `wait4` says there is no such child.
        let Err(ShimError::Reap { pid, source }) = reap(1) else {
            panic!("expected a reap error");
        };
        assert_eq!(pid, 1);
        assert_eq!(source.raw_os_error(), Some(libc::ECHILD));
    }

    #[test]
    fn a_child_the_shim_cannot_use_is_killed_and_taken_so_nothing_is_left() {
        let child = Command::new("/usr/bin/python3")
            .args(["-c", "import time; time.sleep(60)"])
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .spawn()
            .expect("python3 starts");
        let pid = child.id();
        drop(child);
        kill_and_reap(pid);
        // SAFETY: signal 0 only asks whether the pid exists.
        let alive = unsafe { libc::kill(pid as libc::pid_t, 0) } == 0;
        assert!(
            !alive,
            "the child is gone and was reaped, so its pid is free"
        );
    }

    // ---- the spec and the identity -------------------------------------------------------------------------

    fn spec(stdio: ChildStdio) -> ShimSpec {
        ShimSpec {
            program: "/usr/bin/python3".into(),
            args: vec!["-c".into(), "print(1)".into()],
            env: BTreeMap::from([("A".to_string(), "b".to_string())]),
            cwd: "/tmp".into(),
            run_dir: "/r".into(),
            stdio,
            daemon: None,
        }
    }

    #[test]
    fn a_spec_survives_its_json() {
        for stdio in [ChildStdio::Pipes, ChildStdio::Pty { rows: 24, cols: 80 }] {
            let spec = spec(stdio);
            assert_eq!(ShimSpec::from_json(&spec.to_json()).unwrap(), spec);
        }
    }

    #[test]
    fn a_spec_that_starts_the_daemon_survives_its_json() {
        let mut with = spec(ChildStdio::Pipes);
        with.daemon = Some(DaemonStart {
            exe: "/bin/collide-daemon".into(),
            env: BTreeMap::from([("COLLIDE_DAEMON_ROOT".to_string(), "/r".to_string())]),
        });
        assert_eq!(ShimSpec::from_json(&with.to_json()).unwrap(), with);
    }

    #[test]
    fn a_spec_without_a_program_names_it() {
        let mut value = spec(ChildStdio::Pipes).to_json();
        value.as_object_mut().unwrap().remove("program");
        let message = ShimSpec::from_json(&value).unwrap_err().to_string();
        assert!(message.contains("program"), "{message}");
    }

    #[test]
    fn an_identity_survives_its_json() {
        let identity = ChildIdentity {
            pid: 42,
            started_unix_us: 1_700_000_000_123_456,
        };
        assert_eq!(
            ChildIdentity::from_json(&identity.to_json()),
            Some(identity)
        );
        assert_eq!(ChildIdentity::from_json(&json!({ "pid": 1 })), None);
    }

    // ---- the journal writer --------------------------------------------------------------------------------

    #[test]
    fn a_fresh_journal_writer_starts_at_sequence_zero() {
        assert_eq!(JournalWriter::new(Vec::new()).next_seq(), 0);
    }

    #[test]
    fn records_are_numbered_from_zero_and_read_back_whole() {
        let mut journal = JournalWriter::new(Vec::new());
        assert_eq!(journal.append(Direction::In, 10, b"hello\n").unwrap(), 0);
        assert_eq!(journal.append(Direction::Out, 20, b"world\n").unwrap(), 1);
        let read = collide_core::journal::read_journal(&journal.sink);
        assert_eq!(read.end, collide_core::journal::JournalEnd::Clean);
        assert_eq!(read.records.len(), 2);
        assert_eq!(read.records[1].bytes, b"world\n");
        assert_eq!(read.records[1].wall_ns, 20);
    }

    #[test]
    fn a_journal_that_fills_up_keeps_its_whole_records_and_takes_no_more() {
        for (ok_writes, partial) in [(2, 0), (2, 7), (2, 25), (0, 0)] {
            let disk = FullDisk::cutting(ok_writes, partial);
            let mut journal = JournalWriter::new(disk.clone());
            let mut results = Vec::new();
            for n in 0..5u64 {
                results.push(journal.append(Direction::Out, n, b"a line\n"));
            }
            assert_eq!(
                results.iter().filter(|r| r.is_ok()).count(),
                ok_writes,
                "({ok_writes}, {partial})"
            );
            assert!(
                results[ok_writes..].iter().all(|r| r.is_err()),
                "no record after a failure"
            );
            let read = collide_core::journal::read_journal(&disk.taken());
            assert_eq!(read.records.len(), ok_writes, "({ok_writes}, {partial})");
            assert!(
                !matches!(read.end, collide_core::journal::JournalEnd::Damaged { .. }),
                "({ok_writes}, {partial}): {:?}",
                read.end
            );
        }
    }

    #[test]
    fn a_failed_journal_write_is_an_enospc_error() {
        let mut journal = JournalWriter::new(Fails);
        let Err(ShimError::Write { source, context }) = journal.append(Direction::In, 1, b"x")
        else {
            panic!("expected a write error");
        };
        assert_eq!(source.raw_os_error(), Some(ENOSPC));
        assert_eq!(source.kind(), io::ErrorKind::StorageFull);
        assert!(context.contains("journal"));
    }

    #[test]
    fn a_record_over_the_limit_is_refused_and_the_journal_goes_on() {
        let mut journal = JournalWriter::new(Vec::new());
        assert!(
            journal
                .append(Direction::In, 1, &vec![0; MAX_RECORD_BYTES + 1])
                .is_err()
        );
        assert_eq!(journal.append(Direction::In, 2, b"ok").unwrap(), 0);
    }

    // ---- the line cap --------------------------------------------------------------------------------------

    #[test]
    fn lines_split_across_reads_come_out_whole() {
        let mut splitter = LineSplitter::new(100);
        let lines = lines_of(&mut splitter, &[b"one\ntw", b"o\nthr", b"ee\n"]);
        assert_eq!(
            lines,
            vec![b"one\n".to_vec(), b"two\n".to_vec(), b"three\n".to_vec()]
        );
    }

    #[test]
    fn a_last_line_with_no_newline_comes_out_as_it_is() {
        let mut splitter = LineSplitter::new(100);
        assert_eq!(
            lines_of(&mut splitter, &[b"a\nb"]),
            vec![b"a\n".to_vec(), b"b".to_vec()]
        );
    }

    #[test]
    fn a_line_over_the_cap_is_cut_and_marked_however_it_arrives() {
        let long = [b'x'; 25];
        let marker = line_cut_marker(15);
        let mut expected = b"xxxxxxxxxx".to_vec();
        expected.extend_from_slice(marker.as_bytes());
        expected.push(b'\n');
        for chunk_size in [1, 3, 10, 26, 100] {
            let mut input = long.to_vec();
            input.push(b'\n');
            let chunks: Vec<&[u8]> = input.chunks(chunk_size).collect();
            let mut splitter = LineSplitter::new(10);
            assert_eq!(
                lines_of(&mut splitter, &chunks),
                vec![expected.clone()],
                "chunk size {chunk_size}"
            );
        }
    }

    #[test]
    fn a_line_at_the_cap_is_not_cut_and_the_line_after_a_cut_one_is_whole() {
        let mut splitter = LineSplitter::new(4);
        let lines = lines_of(&mut splitter, &[b"abcd\nabcdef\nok\n"]);
        assert_eq!(lines[0], b"abcd\n");
        assert_eq!(
            lines[1],
            [b"abcd".as_slice(), line_cut_marker(2).as_bytes(), b"\n"].concat()
        );
        assert_eq!(lines[2], b"ok\n");
    }

    #[test]
    fn a_long_line_with_no_newline_at_the_end_is_cut_and_marked_too() {
        let mut splitter = LineSplitter::new(3);
        let lines = lines_of(&mut splitter, &[b"abcdefg"]);
        assert_eq!(
            lines,
            vec![[b"abc".as_slice(), line_cut_marker(4).as_bytes()].concat()]
        );
    }

    #[test]
    fn a_runaway_line_costs_the_cap_in_memory_and_no_more() {
        let mut splitter = LineSplitter::new(8);
        let mut out = Vec::new();
        for _ in 0..1000 {
            splitter.push(&[b'z'; 4096], &mut out);
            assert!(splitter.line.len() <= 8);
        }
        assert!(out.is_empty());
    }

    // ---- the log cap ---------------------------------------------------------------------------------------

    #[test]
    fn a_log_over_its_cap_stops_growing_with_a_final_marker() {
        let sink = Arc::new(Mutex::new(Vec::new()));
        struct Shared(Arc<Mutex<Vec<u8>>>);
        impl Write for Shared {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                locked(&self.0).extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut log = CappedLog::new(Shared(Arc::clone(&sink)), 20);
        for _ in 0..10 {
            log.write(b"0123456789\n").unwrap();
        }
        let text = String::from_utf8(locked(&sink).clone()).unwrap();
        let marker = log_full_marker(20);
        // One 11-byte line fits; the second would pass 20, so the marker is written and nothing follows it.
        assert_eq!(text, format!("0123456789\n{marker}"));
        assert!(text.ends_with(&marker));
        log.write(b"more\n").unwrap();
        assert_eq!(locked(&sink).len(), text.len(), "the log stopped growing");
    }

    #[test]
    fn a_log_that_hits_enospc_reports_once_and_drops_the_rest() {
        let disk = FullDisk::after(1);
        let mut log = CappedLog::new(disk.clone(), 1000);
        log.write(b"first\n").unwrap();
        let Err(ShimError::Write { source, .. }) = log.write(b"second\n") else {
            panic!("expected the write to fail");
        };
        assert_eq!(source.raw_os_error(), Some(ENOSPC));
        log.write(b"third\n").unwrap();
        assert_eq!(disk.taken(), b"first\n");
    }

    // ---- the PTY ring --------------------------------------------------------------------------------------

    #[test]
    fn a_ring_keeps_everything_under_its_capacity() {
        let mut ring = PtyRing::new(10);
        ring.push(b"abc");
        ring.push(b"def");
        assert_eq!(ring.snapshot(), b"abcdef");
    }

    #[test]
    fn a_ring_drops_the_oldest_bytes_past_its_capacity() {
        let mut ring = PtyRing::new(5);
        ring.push(b"abc");
        ring.push(b"def");
        assert_eq!(ring.snapshot(), b"bcdef");
    }

    #[test]
    fn one_push_bigger_than_the_ring_keeps_its_last_bytes() {
        let mut ring = PtyRing::new(4);
        ring.push(b"abc");
        ring.push(b"0123456789");
        assert_eq!(ring.snapshot(), b"6789");
    }

    #[test]
    fn the_real_ring_keeps_exactly_one_mebibyte_of_the_end() {
        let mut ring = PtyRing::new(PTY_RING_BYTES);
        let chunk: Vec<u8> = (0..255u8).cycle().take(300_000).collect();
        let mut all = Vec::new();
        for _ in 0..5 {
            ring.push(&chunk);
            all.extend_from_slice(&chunk);
        }
        assert_eq!(ring.snapshot(), all[all.len() - PTY_RING_BYTES..]);
    }

    // ---- exit.json -----------------------------------------------------------------------------------------

    #[test]
    fn exit_json_holds_the_record_and_no_temp_file_is_left() {
        let dir = TempDir::new();
        write_exit_record(dir.path(), &record(7)).unwrap();
        let bytes = fs::read(dir.path().join(files::EXIT_RECORD)).unwrap();
        assert_eq!(
            collide_core::shim::parse_exit_record(&bytes).unwrap(),
            record(7)
        );
        let names: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from("exit.json")]);
    }

    #[test]
    fn exit_json_is_private_to_the_user() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TempDir::new();
        write_exit_record(dir.path(), &record(0)).unwrap();
        let mode = fs::metadata(dir.path().join(files::EXIT_RECORD))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn a_write_that_fails_part_way_leaves_no_exit_json_and_no_temp_file() {
        let dir = TempDir::new();
        let result = write_exit_record_through(dir.path(), &record(1), |file| {
            struct HalfThenFull(File);
            impl Write for HalfThenFull {
                fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                    let half = buf.len() / 2;
                    self.0.write_all(&buf[..half])?;
                    Err(enospc())
                }
                fn flush(&mut self) -> io::Result<()> {
                    Ok(())
                }
            }
            HalfThenFull(file)
        });
        let Err(ShimError::Write { source, .. }) = result else {
            panic!("expected the write to fail");
        };
        assert_eq!(source.raw_os_error(), Some(ENOSPC));
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn a_write_that_fails_leaves_the_old_exit_json_whole() {
        let dir = TempDir::new();
        write_exit_record(dir.path(), &record(5)).unwrap();
        let result = write_exit_record_through(dir.path(), &record(9), |_| Fails);
        assert!(result.is_err());
        let bytes = fs::read(dir.path().join(files::EXIT_RECORD)).unwrap();
        assert_eq!(
            collide_core::shim::parse_exit_record(&bytes).unwrap(),
            record(5)
        );
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn a_folder_that_is_not_there_is_an_error_naming_it() {
        let dir = TempDir::new();
        let missing = dir.path().join("missing");
        let message = write_exit_record(&missing, &record(0))
            .unwrap_err()
            .to_string();
        assert!(message.contains("missing"), "{message}");
    }

    #[test]
    fn a_stale_temp_file_from_a_dead_shim_is_replaced() {
        let dir = TempDir::new();
        fs::write(dir.path().join("exit.json.tmp"), "{\"half").unwrap();
        write_exit_record(dir.path(), &record(3)).unwrap();
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn a_keeper_serves_the_record_until_it_is_acknowledged() {
        let mut keeper = ExitKeeper::new(record(2));
        assert_eq!(
            keeper.pending_message(),
            Some(ShimMessage::Exited { exit: record(2) })
        );
        assert_eq!(
            keeper.pending_message(),
            Some(ShimMessage::Exited { exit: record(2) }),
            "serving it does not use it up"
        );
        keeper.acknowledge();
        assert_eq!(keeper.pending_message(), None);
        assert_eq!(keeper.record(), &record(2), "the record is still held");
    }

    #[test]
    fn on_a_full_disk_the_record_is_kept_and_served_and_no_file_is_left() {
        let dir = TempDir::new();
        let outcome = ExitOutcome {
            write_error: write_exit_record_through(dir.path(), &record(4), |_| Fails).err(),
            keeper: ExitKeeper::new(record(4)),
        };
        assert!(outcome.write_error.is_some());
        assert!(!dir.path().join(files::EXIT_RECORD).exists());
        let mut keeper = outcome.keeper;
        assert_eq!(
            keeper.pending_message(),
            Some(ShimMessage::Exited { exit: record(4) })
        );
        keeper.acknowledge();
        assert_eq!(keeper.pending_message(), None);
    }

    #[test]
    fn recording_an_exit_writes_the_file_and_keeps_the_record() {
        let dir = TempDir::new();
        let outcome = record_exit(dir.path(), record(6));
        assert!(outcome.write_error.is_none());
        assert!(dir.path().join(files::EXIT_RECORD).exists());
        assert!(outcome.keeper.pending_message().is_some());
    }

    #[test]
    fn recording_an_exit_into_a_missing_folder_keeps_the_record() {
        let dir = TempDir::new();
        let outcome = record_exit(&dir.path().join("gone"), record(6));
        assert!(outcome.write_error.is_some());
        assert_eq!(
            outcome.keeper.pending_message(),
            Some(ShimMessage::Exited { exit: record(6) })
        );
    }
}
