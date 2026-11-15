//! The `collide shim` mode (0.2, The owner is `collide-daemon`, with a per-child shim; Stage 3a; spike S3). One
//! shim runs for one child, so the child's owner is a small process that a daemon restart does not touch. The
//! protocol it speaks is `core`'s ([`collide_core::shim`]), the framing is [`collide_core::frame`], and the journal's
//! record is [`collide_core::journal`]; this module is the process that uses them.
//!
//! The shim is synchronous. It:
//!
//! 1. calls `setsid`, with the child started by `std::process::Command` and `pre_exec(setsid)` ([`spawn_child`]);
//! 2. gives the child's stdout and stderr to log files, and holds its stdin pipe, or for an interactive session the
//!    PTY master, across a daemon restart ([`ChildStdio`]);
//! 3. records the child's pid and start time in the run directory ([`ChildIdentity`]), so the daemon can stop that
//!    child when it marks the phase `lost`;
//! 4. journals the traffic in one ordered file, each record written before the child sees the bytes
//!    ([`JournalWriter`]);
//! 5. for a PTY keeps the last [`PTY_RING_BYTES`] of output and replays it on attach, atomically with subscribing
//!    ([`PtyRing`]);
//! 6. reaps the child with `wait4` ([`reap`]). An agent's exit is **never** taken from std's or tokio's `wait`, and a
//!    tokio `Child` is never dropped (measured S3);
//! 7. writes `exit.json` atomically ([`write_exit_record`]): temp file, `fsync`, rename, `fsync` of the directory.
//!    On a full disk it keeps the record in memory and serves it until the daemon acknowledges it ([`ExitKeeper`]).
//!
//! A shim that dies without writing `exit.json` makes its phase `lost`, whatever the agent's stream said. That is the
//! daemon's rule, so nothing here writes a record for a shim that did not finish.
//!
//! | tag | function | what it does |
//! |---|---|---|
//! | `shim: entry` | [`run`] | the shim's `main`: arguments, the steps above in order, the exit code |
//! | `shim: spawn child` | [`spawn_child`] | `setsid`, the log files, the pipes or the PTY, and the child's identity file |
//! | `shim: reap` | [`reap`] | `wait4`, and the exit record built from its status and `rusage` |
//! | `shim: journal writer` | [`JournalWriter`] | one ordered file of records, journal first |
//! | `shim: exit record` | [`write_exit_record`], [`ExitKeeper`] | the atomic `exit.json`, and the in-memory copy on a full disk |
//! | `shim: pty ring` | [`PtyRing`] | the last 1 MiB of a PTY's output, and the replay on attach |
//! | `shim: daemon link` | [`serve_daemon`] | the shim's socket: handshake, `Write`, `CloseStdin`, `Signal`, `AckExit` |

use collide_core::journal::Direction;
use collide_core::shim::{ExitRecord, ShimMessage};
use std::collections::BTreeMap;
use std::fmt;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// The output a PTY's ring keeps (0.2, measured S2): the last 1 MiB.
pub const PTY_RING_BYTES: usize = 1024 * 1024;

/// The files of a run directory, by name. The plan names `exit.json` and the journal and leaves the rest unnamed, so
/// these are this outline's choice.
pub mod files {
    pub const EXIT_RECORD: &str = "exit.json";
    pub const JOURNAL: &str = "journal";
    pub const STDOUT_LOG: &str = "stdout.log";
    pub const STDERR_LOG: &str = "stderr.log";
    /// The child's pid and start time ([`super::ChildIdentity`]).
    pub const CHILD: &str = "child.json";
    /// The shim's socket directory, mode `0700` (0.2).
    pub const SOCKET_DIR: &str = "shim";
    /// The shim's socket, inside [`SOCKET_DIR`]. Short, because the whole path is limited to 103 bytes (0.2).
    pub const SOCKET: &str = "s";
}

/// How the child's stdin and output are connected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildStdio {
    /// A pipe for stdin, and files for stdout and stderr. A `claude -p` phase.
    Pipes,
    /// A PTY, whose master the shim holds. An interactive session (a take-over, a new session).
    Pty { rows: u16, cols: u16 },
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
}

/// The child's pid and the time it started, as `p_starttime` gives it, recorded in the run directory. The daemon
/// reattaches by both, because a pid alone can be reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChildIdentity {
    pub pid: u32,
    /// Microseconds since the Unix epoch.
    pub started_unix_us: u64,
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
}

impl fmt::Display for ShimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShimError::Spawn { context, source } => {
                write!(f, "could not start the child: {context}: {source}")
            }
            ShimError::Reap { pid, source } => write!(f, "wait4 on {pid} failed: {source}"),
            ShimError::Write { context, source } => write!(f, "{context}: {source}"),
        }
    }
}

impl std::error::Error for ShimError {}

/// The shim's `main`, for `collide shim <args>`: reads the spec, starts the child, serves the daemon, reaps the child
/// and writes `exit.json`. Returns the process exit code of the shim itself, which is not the child's.
pub fn run(args: &[String]) -> i32 {
    let _ = args;
    todo!("shim: entry")
}

/// Starts the child with `pre_exec(setsid)`, stdout and stderr to the log files of [`files`] and stdin as
/// [`ChildStdio`] says, and writes its [`ChildIdentity`] to the run directory.
pub fn spawn_child(spec: &ShimSpec) -> Result<ChildIdentity, ShimError> {
    let _ = spec;
    todo!("shim: spawn child")
}

/// Waits for `pid` with `wait4` and returns what `exit.json` holds: how it ended, and the CPU time and largest
/// resident set of it and its waited-for descendants.
pub fn reap(pid: u32) -> Result<ExitRecord, ShimError> {
    let _ = pid;
    todo!("shim: reap")
}

/// The journal of one child (0.2): one ordered file of `{seq, wall_ns, dir, bytes}` records, so replay has one order
/// that two logs could not give. It writes to any [`Write`], so a test passes a writer that fails with `ENOSPC` on
/// demand and the failing write is a chosen one (0.2, Durability).
pub struct JournalWriter<W: Write> {
    sink: W,
    next_seq: u64,
}

impl<W: Write> JournalWriter<W> {
    /// A writer that starts at `seq` 0 on an empty sink.
    pub fn new(sink: W) -> Self {
        JournalWriter { sink, next_seq: 0 }
    }

    /// The `seq` the next record gets.
    pub fn next_seq(&self) -> u64 {
        self.next_seq
    }

    /// Writes one record and flushes it, and returns its `seq`. The caller passes the bytes to the child only after
    /// this returns, so an `in` record never goes missing. `wall_ns` is a reading of the wall clock
    /// ([`crate::clock::Clock::Wall`]).
    pub fn append(&mut self, dir: Direction, wall_ns: u64, bytes: &[u8]) -> Result<u64, ShimError> {
        let _ = (&mut self.sink, dir, wall_ns, bytes);
        todo!("shim: journal writer")
    }
}

/// Writes `record` to `dir/exit.json` atomically: a temp file, `fsync`, rename, and `fsync` of `dir`. A file that
/// cannot be written leaves the old one whole.
pub fn write_exit_record(dir: &Path, record: &ExitRecord) -> Result<(), ShimError> {
    let _ = (dir, record);
    todo!("shim: exit record")
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
        let _ = (&self.record, self.acknowledged);
        todo!("shim: exit record")
    }

    /// The daemon has the record: the shim may stop serving it and exit.
    pub fn acknowledge(&mut self) {
        todo!("shim: exit record")
    }
}

/// The last [`PTY_RING_BYTES`] of a PTY's output. On attach the shim replays it and subscribes to new output in one
/// step, so a view opened after an app restart shows the session so far with no gap and no repeat (measured S2).
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
        let _ = (&mut self.bytes, self.capacity, output);
        todo!("shim: pty ring")
    }

    /// What the ring holds, oldest first, for the replay on attach.
    pub fn snapshot(&self) -> Vec<u8> {
        todo!("shim: pty ring")
    }
}

/// Serves the daemon on the shim's socket: the handshake (the shim's [`ShimMessage::Hello`], answered by
/// [`collide_core::shim::answer_hello`]), then each [`collide_core::shim::DaemonMessage`] until the exit record is
/// acknowledged. The socket's directory is mode
/// `0700` under the run directory and its peer's uid is checked, with no token (0.2).
pub fn serve_daemon(run_dir: &Path, child: ChildIdentity) -> Result<(), ShimError> {
    let _ = (run_dir, child);
    todo!("shim: daemon link")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pty_ring_keeps_one_mebibyte() {
        assert_eq!(PTY_RING_BYTES, 1_048_576);
    }

    #[test]
    fn the_exit_record_is_named_as_core_writes_it() {
        assert_eq!(files::EXIT_RECORD, "exit.json");
    }

    #[test]
    fn a_fresh_journal_writer_starts_at_sequence_zero() {
        assert_eq!(JournalWriter::new(Vec::new()).next_seq(), 0);
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

    #[test]
    #[should_panic(expected = "shim: reap")]
    fn reaping_is_a_stub_tagged_for_stage_3a() {
        let _ = reap(1);
    }
}
