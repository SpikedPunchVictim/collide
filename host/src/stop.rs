//! Stopping a tree of processes (0.5, Stopping; Stage 3a). A stop is not a signal to one pid: hooks run in their own
//! process groups, and `iter` runs each step in its own again, so the stop finds everything first and escalates in a
//! fixed order. `collide stop` and the daemon's stop of a run both come here ([`crate::phases::Phases::stop_run`]);
//! each hook runs under a shim, so the same rules cover it.
//!
//! **Finding the tree** ([`find_tree`]) combines three things (0.2):
//!
//! - the ppid walk below the recorded roots (the child of each phase's shim, by its pid and start time);
//! - the marker scan: every process of the user whose environment holds `COLLIDE_RUN=<the run's id>`, read through
//!   libproc and `KERN_PROCARGS2`, skipping processes older than the run's start clock. The value is matched whole,
//!   never as a prefix or a substring;
//! - the descendants of everything the first two found. A process that double-forked still carries the marker, and
//!   a descendant of one that does not is reached from its parent.
//!
//! A process is named by its pid **and** its `p_uniqueid` ([`ProcId`]), which macOS never gives to a second process.
//! The scan reads each process's record once and walks the tree by those records, so a pid that was reused while the
//! scan ran is never taken for its predecessor, and every signal looks the process up again immediately before it
//! goes ([`signal_if_same`]). The daemon and the shims carry `COLLIDE_RUNTIME=1` ([`RUNTIME_MARKER`]): they are never
//! found, nor is anything reached only through them, because a shim must outlive the stop to reap its child and write
//! `exit.json`. The scan reads the table and signals nothing. A stop with an empty run id is refused
//! ([`StopRequest::check`]): with no marker the scan would match a process that has none.
//!
//! **The order** is [`ORDER`] (measured S3); an agent first gets `interrupt`, then its stdin closes (0.3):
//!
//! 1. SIGTERM to everything found (each also gets SIGCONT, which a stopped process needs to act on the SIGTERM).
//! 2. The grace period, [`DEFAULT_GRACE`] unless the hook sets one (`iter` escalates after 30 s). It ends as soon as
//!    everything signalled has gone.
//! 3. A rescan, then SIGKILL to everything found.
//! 4. The workspace implementation's `cleanup` hook ([`Cleanup`]), given the processes the hooks recorded as `detached`
//!    ([`Detached`]). It runs whatever the earlier steps found, because it is the only thing that reaches what the
//!    scans miss.
//! 5. A final rescan and the survivor report ([`StopReport::survivors`]).
//!
//! **The survivor report** lists what is still alive: processes of the tree that outlived SIGKILL, recorded
//! `detached` processes still alive, and suspects (parent pid 1, the user's, started after the run, environment
//! unreadable, not an executable of the system's own services under `/System/`, `/usr/libexec/` or `/usr/sbin/`).
//! It reports suspects and never signals them: a process that carries no marker and was not recorded is
//! beyond what a stop may kill (0.5).
//!
//! The shim checks that its child's process group still exists (`waitid` with `WNOWAIT`) before it signals the group,
//! so a reused group id is never hit.
//!
//! | tag | function | what it does |
//! |---|---|---|
//! | `stop: find tree` | [`find_tree`] | the ppid walk, the marker scan and their descendants |
//! | `stop: escalation` | [`stop_tree`] | the steps of [`ORDER`] and the [`StopReport`] |

use crate::sysproc::{self, EnvRead, ProcRecord};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::time::{Duration, Instant};

/// The time a stopped hook gets between SIGTERM and SIGKILL when its entry names none (0.5).
pub const DEFAULT_GRACE: Duration = Duration::from_secs(60);

/// The variable whose value is a run's id: every process of a run carries it (0.2).
pub const RUN_MARKER: &str = "COLLIDE_RUN";

/// The variable the daemon and the shims carry (value `1`): a process that has it is never swept.
pub const RUNTIME_MARKER: &str = "COLLIDE_RUNTIME";

/// How often a wait looks at the table.
pub const POLL: Duration = Duration::from_millis(50);

/// How long a stop waits, after SIGKILL and after the cleanup hook, for the processes to leave the table before the
/// final scan. The plan sets none: a killed process is gone within a few milliseconds, and a zombie counts as gone.
pub const SETTLE: Duration = Duration::from_secs(2);

/// One step of a stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopStep {
    /// SIGTERM to everything found.
    Terminate,
    /// Wait for the processes to go, up to the grace period.
    Grace,
    /// A rescan, then SIGKILL to everything found.
    Kill,
    /// The workspace implementation's `cleanup` hook.
    Cleanup,
    /// A final rescan and the survivor report.
    Report,
}

/// The steps in the order they run.
pub const ORDER: [StopStep; 5] = [
    StopStep::Terminate,
    StopStep::Grace,
    StopStep::Kill,
    StopStep::Cleanup,
    StopStep::Report,
];

/// One process, for good: its pid and its `p_uniqueid`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProcId {
    pub pid: u32,
    pub unique_id: u64,
}

impl ProcId {
    fn of(record: &ProcRecord) -> ProcId {
        ProcId {
            pid: record.pid,
            unique_id: record.unique_id,
        }
    }
}

/// A process the tree starts from: a pid and the time it started, as the shim recorded them in `child.json`. It is
/// taken only while a process with that pid started at that time, so a pid that was reused is not a root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Root {
    pub pid: u32,
    pub started_us: u64,
}

/// What a hook recorded as started in the background (0.5, Hooks record what they detach): a process by pid and
/// start time, or a container by id. The cleanup hook gets the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Detached {
    Process { pid: u32, started_unix_us: u64 },
    Container { id: String },
}

impl Detached {
    fn to_json(&self) -> Value {
        match self {
            Detached::Process {
                pid,
                started_unix_us,
            } => json!({ "pid": pid, "started_unix_us": started_unix_us }),
            Detached::Container { id } => json!({ "container": id }),
        }
    }

    fn from_json(value: &Value) -> Option<Detached> {
        if let Some(id) = value.get("container") {
            return Some(Detached::Container {
                id: id.as_str()?.to_string(),
            });
        }
        Some(Detached::Process {
            pid: u32::try_from(value.get("pid")?.as_u64()?).ok()?,
            started_unix_us: value.get("started_unix_us")?.as_u64()?,
        })
    }
}

/// The file in a run's directory that holds its `detached` records.
pub const DETACHED_FILE: &str = "detached.json";

/// The records a run's hooks made, oldest first. A run with none has no file, and that is an empty list; a file that
/// cannot be read as records is an error and not an empty list, so a stop never decides nothing was detached from a
/// file it could not read.
pub fn read_detached(run_dir: &Path) -> io::Result<Vec<Detached>> {
    let text = match fs::read_to_string(run_dir.join(DETACHED_FILE)) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let bad = |what: &str| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{DETACHED_FILE}: {what}"),
        )
    };
    let value: Value = serde_json::from_str(&text).map_err(|e| bad(&e.to_string()))?;
    value
        .get("detached")
        .and_then(Value::as_array)
        .ok_or_else(|| bad("`detached` is missing or is not a list"))?
        .iter()
        .map(|item| Detached::from_json(item).ok_or_else(|| bad("an entry is not a record")))
        .collect()
}

/// Adds one record to the run's file, atomically (a temporary file, then a rename), so a stop that reads while a hook
/// writes sees the old list or the new one.
pub fn record_detached(run_dir: &Path, item: &Detached) -> io::Result<()> {
    let mut all = read_detached(run_dir)?;
    all.push(item.clone());
    let body = json!({ "detached": all.iter().map(Detached::to_json).collect::<Vec<_>>() });
    let temporary = run_dir.join(format!("{DETACHED_FILE}.tmp-{}", std::process::id()));
    let mut file = fs::File::create(&temporary)?;
    file.write_all(body.to_string().as_bytes())?;
    file.sync_all()?;
    fs::rename(&temporary, run_dir.join(DETACHED_FILE))
}

/// What a stop is asked to stop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopRequest {
    /// The roots of the tree: each phase's child. The shim is not one: it reaps the child and writes `exit.json`.
    pub roots: Vec<Root>,
    /// The run's marker, the value the processes of the run carry in `COLLIDE_RUN`. Never empty ([`Self::check`]).
    pub marker: String,
    /// The run's start clock, in microseconds since the Unix epoch: a process that started before it is not found by
    /// the marker, and suspects must have started after it. `None` where it is not known, which also means no
    /// suspects.
    pub run_start_us: Option<u64>,
    /// Whether the marker scan runs. It is on for a stop of a run; it is off for the stop of one lost phase while other
    /// phases of the same run are live, so that the stop reaches the roots' trees only.
    pub by_marker: bool,
    pub grace: Duration,
    /// How often the waits look at the table ([`POLL`]).
    pub poll: Duration,
    /// How long the waits after SIGKILL and the cleanup hook last ([`SETTLE`]).
    pub settle: Duration,
    /// What the run's hooks recorded as detached.
    pub detached: Vec<Detached>,
}

impl StopRequest {
    /// A request for the run `marker`, with the default grace, no roots and no records.
    pub fn new(marker: impl Into<String>) -> StopRequest {
        StopRequest {
            roots: Vec::new(),
            marker: marker.into(),
            run_start_us: None,
            by_marker: true,
            grace: DEFAULT_GRACE,
            poll: POLL,
            settle: SETTLE,
            detached: Vec::new(),
        }
    }

    /// Refuses a request that could signal a process of another run: an empty marker, or one that is not a plain
    /// value (a NUL, a newline, or an `=` in the id cannot be what a process's environment holds).
    pub fn check(&self) -> io::Result<()> {
        let bad = self.marker.is_empty()
            || self
                .marker
                .chars()
                .any(|c| c == '\0' || c == '\n' || c.is_control());
        if bad {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "refusing to stop with the run id {:?}: a stop finds processes by their run id, and this one would match the wrong ones",
                    self.marker
                ),
            ));
        }
        Ok(())
    }
}

/// How a process came to be in the survivor report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurvivorKind {
    /// Found by the scan and still alive after SIGKILL.
    Found,
    /// A `detached` process the hooks recorded, still alive after the cleanup hook.
    Recorded,
    /// Not recorded and not carrying the marker, but the kind a double-forked orphan is: parent pid 1, the user's,
    /// started after the run, environment unreadable.
    Suspect,
}

impl SurvivorKind {
    pub fn word(self) -> &'static str {
        match self {
            SurvivorKind::Found => "found",
            SurvivorKind::Recorded => "recorded",
            SurvivorKind::Suspect => "suspect",
        }
    }
}

/// A process still alive after the final rescan. A double-forked orphan that its hook recorded is stopped by the
/// cleanup hook; one it did not record is listed here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Survivor {
    pub pid: u32,
    pub command: String,
    pub kind: SurvivorKind,
}

/// What the cleanup hook did.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum CleanupOutcome {
    /// There is no cleanup hook to run.
    #[default]
    NotRun,
    Ran,
    /// It failed, and what it said. The stop goes on to its report.
    Failed(String),
}

/// What a stop did.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StopReport {
    /// The pids found by the first scan and signalled with SIGTERM.
    pub signalled: Vec<u32>,
    /// The pids found by the rescan and signalled with SIGKILL: what outlived the grace period.
    pub killed: Vec<u32>,
    pub cleanup: CleanupOutcome,
    pub survivors: Vec<Survivor>,
    /// Signals that could not be sent for a reason other than the process being gone, and what a scan could not
    /// read. A process that was not signalled and is still alive is a survivor too.
    pub problems: Vec<String>,
}

/// The workspace implementation's `cleanup` hook (0.5). The hook executor is Stage 7; until it exists a stop is given
/// whatever stands in for it. A closure is one.
pub trait Cleanup {
    fn run(&self, detached: &[Detached]) -> io::Result<()>;
}

impl<F: Fn(&[Detached]) -> io::Result<()>> Cleanup for F {
    fn run(&self, detached: &[Detached]) -> io::Result<()> {
        self(detached)
    }
}

/// The process table, as a stop reads and signals it. The system's is [`SystemProcesses`]; a test gives a fake one,
/// so that a reused pid can be staged.
pub trait Processes {
    /// The uid whose processes may be found.
    fn own_uid(&self) -> u32;
    /// The pid of the process that is stopping, which is never found.
    fn own_pid(&self) -> u32;
    /// Every pid in the table.
    fn pids(&self) -> io::Result<Vec<u32>>;
    /// The record of a pid, or `None` when it is gone or cannot be read.
    fn record(&self, pid: u32) -> Option<ProcRecord>;
    /// The `COLLIDE_RUN` and `COLLIDE_RUNTIME` values of the process's environment, in that order.
    fn environment(&self, pid: u32) -> EnvRead;
    /// What to call the process in a report.
    fn command(&self, pid: u32) -> String;
    /// Sends `signal` to `pid`. Whoever calls this has just checked the process's identity ([`signal_if_same`]).
    fn signal(&self, pid: u32, signal: i32) -> io::Result<()>;
}

/// The real table: libproc and `kill`.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemProcesses;

impl Processes for SystemProcesses {
    fn own_uid(&self) -> u32 {
        sysproc::daemon_uid()
    }

    fn own_pid(&self) -> u32 {
        std::process::id()
    }

    fn pids(&self) -> io::Result<Vec<u32>> {
        sysproc::all_pids()
    }

    fn record(&self, pid: u32) -> Option<ProcRecord> {
        sysproc::record_of(pid)
    }

    fn environment(&self, pid: u32) -> EnvRead {
        sysproc::env_vars_of(pid, &[RUN_MARKER, RUNTIME_MARKER])
    }

    fn command(&self, pid: u32) -> String {
        sysproc::executable_of(pid).map_or_else(
            || "(unknown)".to_string(),
            |path| path.display().to_string(),
        )
    }

    fn signal(&self, pid: u32, signal: i32) -> io::Result<()> {
        let pid = libc::pid_t::try_from(pid)
            .ok()
            .filter(|&p| p > 1)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, format!("pid {pid}")))?;
        // SAFETY: `kill` takes two integers. The pid is above 1, so it is no group, no broadcast and not the init
        // process (`kill` of a negative pid or 0 would signal a group or everything), and the caller looked the
        // process up by pid and `p_uniqueid` just before this call ([`signal_if_same`]).
        if unsafe { libc::kill(pid, signal) } == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
}

/// Whether the process `id` names is in the table now, is the same process and has not ended.
fn is_live(table: &dyn Processes, id: ProcId) -> bool {
    table
        .record(id.pid)
        .is_some_and(|r| r.unique_id == id.unique_id && !r.zombie)
}

/// Sends `signal` to `id` if, and only if, the process with that pid is still the process with that `p_uniqueid`.
/// Returns `Ok(false)` for a process that is gone or is now another: nothing is sent to it.
pub fn signal_if_same(table: &dyn Processes, id: ProcId, signal: i32) -> io::Result<bool> {
    let Some(record) = table.record(id.pid) else {
        return Ok(false);
    };
    if record.unique_id != id.unique_id {
        return Ok(false);
    }
    match table.signal(id.pid, signal) {
        Ok(()) => Ok(true),
        Err(error) if error.raw_os_error() == Some(libc::ESRCH) => Ok(false),
        Err(error) => Err(error),
    }
}

/// One scan of the table: the records, and the children each process has by pid and `p_uniqueid`.
struct Snapshot {
    records: BTreeMap<u32, ProcRecord>,
    children: BTreeMap<u32, Vec<u32>>,
}

impl Snapshot {
    fn take(table: &dyn Processes) -> io::Result<Snapshot> {
        let records: BTreeMap<u32, ProcRecord> = table
            .pids()?
            .into_iter()
            .filter_map(|pid| table.record(pid))
            .map(|record| (record.pid, record))
            .collect();
        let mut children: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
        for record in records.values() {
            // A child belongs to the parent its pid names only while the parent's `p_uniqueid` is the one the child
            // recorded: a parent pid that was reused is another process.
            if records
                .get(&record.parent)
                .is_some_and(|parent| parent.unique_id == record.parent_unique_id)
            {
                children.entry(record.parent).or_default().push(record.pid);
            }
        }
        Ok(Snapshot { records, children })
    }
}

/// The reading of one process's `COLLIDE_RUN` and `COLLIDE_RUNTIME`.
enum Marks {
    /// Not readable, or the system hides it.
    Unknown,
    Read {
        run: Option<String>,
        runtime: bool,
    },
}

fn marks(table: &dyn Processes, pid: u32) -> Marks {
    match table.environment(pid) {
        EnvRead::Read(values) => Marks::Read {
            run: values.first().cloned().flatten(),
            runtime: values.get(1).is_some_and(|v| v.is_some()),
        },
        EnvRead::Hidden | EnvRead::Unreadable => Marks::Unknown,
    }
}

/// Every process of the tree below the request's roots and every process carrying its marker, with their
/// descendants. Read-only. A process is in the result when it is the user's, is not this one, is not ended, and is not
/// a runtime process (`COLLIDE_RUNTIME` set), and nothing is reached through a runtime process.
pub fn find_tree(table: &dyn Processes, request: &StopRequest) -> io::Result<Vec<ProcId>> {
    request.check()?;
    let snapshot = Snapshot::take(table)?;
    let uid = table.own_uid();
    let me = table.own_pid();
    let eligible = |record: &ProcRecord| record.uid == uid && record.pid != me && record.pid > 1;
    let mut seeds: Vec<u32> = Vec::new();
    for root in &request.roots {
        if snapshot
            .records
            .get(&root.pid)
            .is_some_and(|r| r.started_us == root.started_us && eligible(r))
        {
            seeds.push(root.pid);
        }
    }
    for record in snapshot.records.values() {
        if !request.by_marker || !eligible(record) || record.zombie {
            continue;
        }
        if request
            .run_start_us
            .is_some_and(|start| record.started_us < start)
        {
            continue;
        }
        if let Marks::Read {
            run: Some(run),
            runtime: false,
        } = marks(table, record.pid)
            && run == request.marker
        {
            seeds.push(record.pid);
        }
    }
    let mut seen: BTreeSet<u32> = BTreeSet::new();
    let mut found: Vec<ProcId> = Vec::new();
    let mut queue = seeds;
    while let Some(pid) = queue.pop() {
        if !seen.insert(pid) {
            continue;
        }
        let Some(record) = snapshot.records.get(&pid) else {
            continue;
        };
        if !eligible(record) {
            continue;
        }
        if matches!(marks(table, pid), Marks::Read { runtime: true, .. }) {
            continue;
        }
        if !record.zombie {
            found.push(ProcId::of(record));
        }
        queue.extend(snapshot.children.get(&pid).into_iter().flatten().copied());
    }
    found.sort();
    Ok(found)
}

/// Sends `signal` to each of `ids` (checking each one just before) and then SIGCONT, so that a stopped process takes
/// it. Failures other than a process being gone go to `problems`.
fn signal_all(table: &dyn Processes, ids: &[ProcId], signal: i32, problems: &mut Vec<String>) {
    for &id in ids {
        for sig in [signal, libc::SIGCONT] {
            if let Err(error) = signal_if_same(table, id, sig) {
                problems.push(format!("signal {sig} to {}: {error}", id.pid));
            }
        }
    }
}

/// Waits until none of `ids` is live, or `within` has passed. Returns whether they all went.
fn wait_gone(table: &dyn Processes, ids: &[ProcId], within: Duration, poll: Duration) -> bool {
    let until = Instant::now() + within;
    loop {
        if !ids.iter().any(|&id| is_live(table, id)) {
            return true;
        }
        if Instant::now() >= until {
            return false;
        }
        std::thread::sleep(poll.min(until.saturating_duration_since(Instant::now())));
    }
}

/// The recorded `detached` processes that are alive now: the pid has a process that started at the recorded time.
fn recorded_alive(table: &dyn Processes, detached: &[Detached]) -> Vec<u32> {
    detached
        .iter()
        .filter_map(|item| match item {
            Detached::Process {
                pid,
                started_unix_us,
            } => table
                .record(*pid)
                .filter(|r| r.started_us == *started_unix_us && !r.zombie)
                .map(|r| r.pid),
            Detached::Container { .. } => None,
        })
        .collect()
}

/// Processes that look like a double-forked orphan this stop did not find: the user's, parent pid 1, started after
/// the run began, environment unreadable. Never signalled.
fn suspects(table: &dyn Processes, request: &StopRequest, known: &BTreeSet<u32>) -> Vec<u32> {
    let Some(start) = request.run_start_us else {
        return Vec::new();
    };
    let Ok(snapshot) = Snapshot::take(table) else {
        return Vec::new();
    };
    let uid = table.own_uid();
    snapshot
        .records
        .values()
        .filter(|r| {
            r.parent == 1
                && r.uid == uid
                && !r.zombie
                && r.pid > 1
                && r.pid != table.own_pid()
                && r.started_us >= start
                && !known.contains(&r.pid)
        })
        .filter(|r| matches!(marks(table, r.pid), Marks::Unknown))
        .filter(|r| !is_system_service(&table.command(r.pid)))
        .map(|r| r.pid)
        .collect()
}

/// Where the operating system keeps its own services. They are launchd's children whose environment is hidden, which
/// is all a double-forked platform-binary orphan is, and macOS records launchd as the parent of both: measured,
/// `p_puniqueid` of a reparented `/bin/sleep` and of an `mdworker_shared` are the same (launchd's unique id), so it
/// cannot tell them apart. The executable's place does. `/bin` and `/usr/bin` are not here: orphans of the tools a
/// run starts (`sleep`, `sh`) live there.
const SYSTEM_SERVICE_DIRS: [&str; 3] = ["/System/", "/usr/libexec/", "/usr/sbin/"];

fn is_system_service(command: &str) -> bool {
    SYSTEM_SERVICE_DIRS
        .iter()
        .any(|dir| command.starts_with(dir))
}

/// Stops the tree in the order of [`ORDER`]. An `Err` is a failure to scan the table, or a request that is refused
/// ([`StopRequest::check`]); a signal that cannot be sent, a cleanup hook that fails and processes that would not die
/// are not an error but part of the report.
pub fn stop_tree(
    table: &dyn Processes,
    request: &StopRequest,
    cleanup: Option<&dyn Cleanup>,
) -> io::Result<StopReport> {
    request.check()?;
    let mut report = StopReport::default();
    let mut first: Vec<ProcId> = Vec::new();
    for step in ORDER {
        match step {
            StopStep::Terminate => {
                first = find_tree(table, request)?;
                report.signalled = first.iter().map(|id| id.pid).collect();
                signal_all(table, &first, libc::SIGTERM, &mut report.problems);
            }
            StopStep::Grace => {
                wait_gone(table, &first, request.grace, request.poll);
            }
            StopStep::Kill => {
                let rescanned = find_tree(table, request)?;
                report.killed = rescanned.iter().map(|id| id.pid).collect();
                signal_all(table, &rescanned, libc::SIGKILL, &mut report.problems);
                wait_gone(table, &rescanned, request.settle, request.poll);
            }
            StopStep::Cleanup => {
                if let Some(cleanup) = cleanup {
                    report.cleanup = match cleanup.run(&request.detached) {
                        Ok(()) => CleanupOutcome::Ran,
                        Err(error) => CleanupOutcome::Failed(error.to_string()),
                    };
                }
                let recorded = recorded_alive_ids(table, &request.detached);
                wait_gone(table, &recorded, request.settle, request.poll);
            }
            StopStep::Report => {
                let mut known: BTreeSet<u32> = BTreeSet::new();
                for id in find_tree(table, request)? {
                    known.insert(id.pid);
                    report.survivors.push(Survivor {
                        pid: id.pid,
                        command: table.command(id.pid),
                        kind: SurvivorKind::Found,
                    });
                }
                for pid in recorded_alive(table, &request.detached) {
                    if known.insert(pid) {
                        report.survivors.push(Survivor {
                            pid,
                            command: table.command(pid),
                            kind: SurvivorKind::Recorded,
                        });
                    }
                }
                for pid in suspects(table, request, &known) {
                    report.survivors.push(Survivor {
                        pid,
                        command: table.command(pid),
                        kind: SurvivorKind::Suspect,
                    });
                }
            }
        }
    }
    Ok(report)
}

/// The recorded `detached` processes that are alive, as ids, to wait for them to go.
fn recorded_alive_ids(table: &dyn Processes, detached: &[Detached]) -> Vec<ProcId> {
    recorded_alive(table, detached)
        .into_iter()
        .filter_map(|pid| table.record(pid).map(|r| ProcId::of(&r)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// A process table of made-up processes. A test builds the table, stages the reuse of a pid by replacing a process
    /// in it, and reads which signals were sent.
    struct Fake {
        uid: u32,
        me: u32,
        procs: RefCell<BTreeMap<u32, FakeProc>>,
        sent: RefCell<Vec<(u32, i32)>>,
        events: RefCell<Vec<&'static str>>,
        /// Signals that end a process, by pid; a process not in the map ignores everything.
        mortal: RefCell<BTreeMap<u32, Vec<i32>>>,
        /// The command of a process by pid, where it is not `/fake/<pid>`.
        commands: RefCell<BTreeMap<u32, String>>,
    }

    #[derive(Clone)]
    struct FakeProc {
        record: ProcRecord,
        env: EnvRead,
    }

    fn rec(
        pid: u32,
        unique_id: u64,
        parent: u32,
        parent_unique_id: u64,
        started_us: u64,
    ) -> ProcRecord {
        ProcRecord {
            pid,
            parent,
            uid: 501,
            unique_id,
            parent_unique_id,
            started_us,
            zombie: false,
        }
    }

    fn env(run: Option<&str>, runtime: bool) -> EnvRead {
        EnvRead::Read(vec![
            run.map(str::to_string),
            runtime.then(|| "1".to_string()),
        ])
    }

    impl Fake {
        fn new() -> Fake {
            Fake {
                uid: 501,
                me: 9000,
                procs: RefCell::new(BTreeMap::new()),
                sent: RefCell::new(Vec::new()),
                events: RefCell::new(Vec::new()),
                mortal: RefCell::new(BTreeMap::new()),
                commands: RefCell::new(BTreeMap::new()),
            }
        }

        /// Gives a process the executable `command`.
        fn named(&self, pid: u32, command: &str) {
            self.commands.borrow_mut().insert(pid, command.to_string());
        }

        fn add(&self, record: ProcRecord, env: EnvRead) {
            self.procs
                .borrow_mut()
                .insert(record.pid, FakeProc { record, env });
        }

        /// A process that dies of SIGTERM and SIGKILL.
        fn mortal(&self, pid: u32, signals: &[i32]) {
            self.mortal.borrow_mut().insert(pid, signals.to_vec());
        }

        fn sent(&self) -> Vec<(u32, i32)> {
            self.sent.borrow().clone()
        }
    }

    impl Processes for Fake {
        fn own_uid(&self) -> u32 {
            self.uid
        }
        fn own_pid(&self) -> u32 {
            self.me
        }
        fn pids(&self) -> io::Result<Vec<u32>> {
            Ok(self.procs.borrow().keys().copied().collect())
        }
        fn record(&self, pid: u32) -> Option<ProcRecord> {
            self.procs.borrow().get(&pid).map(|p| p.record)
        }
        fn environment(&self, pid: u32) -> EnvRead {
            self.procs
                .borrow()
                .get(&pid)
                .map_or(EnvRead::Unreadable, |p| p.env.clone())
        }
        fn command(&self, pid: u32) -> String {
            self.commands
                .borrow()
                .get(&pid)
                .cloned()
                .unwrap_or_else(|| format!("/fake/{pid}"))
        }
        fn signal(&self, pid: u32, signal: i32) -> io::Result<()> {
            self.sent.borrow_mut().push((pid, signal));
            if self
                .mortal
                .borrow()
                .get(&pid)
                .is_some_and(|dies| dies.contains(&signal))
            {
                self.procs.borrow_mut().remove(&pid);
            }
            Ok(())
        }
    }

    fn request(marker: &str) -> StopRequest {
        let mut request = StopRequest::new(marker);
        request.grace = Duration::from_millis(30);
        request.poll = Duration::from_millis(1);
        request.settle = Duration::from_millis(30);
        request
    }

    fn pids(found: &[ProcId]) -> Vec<u32> {
        found.iter().map(|id| id.pid).collect()
    }

    #[test]
    fn a_stop_ends_the_hook_before_its_cleanup_and_reports_last() {
        assert_eq!(
            ORDER,
            [
                StopStep::Terminate,
                StopStep::Grace,
                StopStep::Kill,
                StopStep::Cleanup,
                StopStep::Report
            ]
        );
    }

    #[test]
    fn the_default_grace_is_sixty_seconds() {
        assert_eq!(DEFAULT_GRACE, Duration::from_secs(60));
    }

    #[test]
    fn an_empty_run_id_is_refused_before_anything_is_read_or_sent() {
        let table = Fake::new();
        table.add(rec(100, 1, 1, 1, 10), env(Some(""), false));
        for marker in ["", "a\0b", "line\nbreak"] {
            let request = request(marker);
            assert_eq!(
                find_tree(&table, &request).unwrap_err().kind(),
                io::ErrorKind::InvalidInput,
                "{marker:?}"
            );
            assert_eq!(
                stop_tree(&table, &request, None).unwrap_err().kind(),
                io::ErrorKind::InvalidInput
            );
        }
        assert!(table.sent().is_empty());
    }

    #[test]
    fn the_marker_must_match_whole_and_never_as_a_prefix_or_substring() {
        let table = Fake::new();
        table.add(rec(100, 1, 1, 1, 10), env(Some("run-7"), false));
        table.add(rec(101, 2, 1, 1, 10), env(Some("run-70"), false));
        table.add(rec(102, 3, 1, 1, 10), env(Some("xrun-7"), false));
        table.add(rec(103, 4, 1, 1, 10), env(Some("run-"), false));
        table.add(rec(104, 5, 1, 1, 10), env(Some("RUN-7"), false));
        let found = find_tree(&table, &request("run-7")).unwrap();
        assert_eq!(pids(&found), [100]);
    }

    #[test]
    fn a_process_older_than_the_runs_start_is_not_found_by_the_marker() {
        let table = Fake::new();
        table.add(rec(100, 1, 1, 1, 500), env(Some("run-7"), false));
        table.add(rec(101, 2, 1, 1, 1500), env(Some("run-7"), false));
        let mut request = request("run-7");
        request.run_start_us = Some(1000);
        assert_eq!(pids(&find_tree(&table, &request).unwrap()), [101]);
    }

    #[test]
    fn the_descendants_of_a_marker_hit_and_of_a_root_are_found_without_the_marker() {
        let table = Fake::new();
        // 100 carries the marker; 110 is its child and 111 its grandchild, neither with the marker.
        table.add(rec(100, 1, 1, 1, 10), env(Some("run-7"), false));
        table.add(rec(110, 2, 100, 1, 11), EnvRead::Hidden);
        table.add(rec(111, 3, 110, 2, 12), env(None, false));
        // 200 is a root recorded by the shim; 210 is its child.
        table.add(rec(200, 4, 50, 99, 20), env(None, false));
        table.add(rec(210, 5, 200, 4, 21), env(None, false));
        // 300 is nobody's.
        table.add(rec(300, 6, 1, 1, 30), env(None, false));
        let mut request = request("run-7");
        request.roots = vec![Root {
            pid: 200,
            started_us: 20,
        }];
        assert_eq!(
            pids(&find_tree(&table, &request).unwrap()),
            [100, 110, 111, 200, 210]
        );
    }

    #[test]
    fn with_the_marker_scan_off_only_the_roots_trees_are_found() {
        let table = Fake::new();
        table.add(rec(100, 1, 1, 1, 10), env(Some("run-7"), false));
        table.add(rec(200, 4, 50, 99, 20), env(None, false));
        table.add(rec(210, 5, 200, 4, 21), env(None, false));
        let mut request = request("run-7");
        request.by_marker = false;
        request.roots = vec![Root {
            pid: 200,
            started_us: 20,
        }];
        assert_eq!(pids(&find_tree(&table, &request).unwrap()), [200, 210]);
    }

    #[test]
    fn a_root_whose_pid_was_reused_is_not_a_root() {
        let table = Fake::new();
        // The shim recorded pid 200 started at 20; the process there now started at 25.
        table.add(rec(200, 4, 1, 1, 25), env(None, false));
        table.add(rec(210, 5, 200, 4, 26), env(None, false));
        let mut request = request("run-7");
        request.roots = vec![Root {
            pid: 200,
            started_us: 20,
        }];
        assert!(find_tree(&table, &request).unwrap().is_empty());
    }

    #[test]
    fn a_child_of_a_reused_parent_pid_is_not_its_predecessors_descendant() {
        let table = Fake::new();
        // 100 is a marker hit with unique id 1. 120 says its parent is pid 100 with unique id 7: an earlier process
        // that had pid 100. It is not below the process that has the pid now.
        table.add(rec(100, 1, 1, 1, 10), env(Some("run-7"), false));
        table.add(rec(120, 3, 100, 7, 12), env(None, false));
        assert_eq!(pids(&find_tree(&table, &request("run-7")).unwrap()), [100]);
    }

    #[test]
    fn runtime_processes_are_never_found_and_nothing_is_reached_through_them() {
        let table = Fake::new();
        // A shim below a marker hit, and its child: the shim is skipped and so is everything only it reaches.
        table.add(rec(100, 1, 1, 1, 10), env(Some("run-7"), false));
        table.add(rec(110, 2, 100, 1, 11), env(None, true));
        table.add(rec(111, 3, 110, 2, 12), env(None, false));
        // A daemon that carries the marker as well as the runtime flag.
        table.add(rec(120, 4, 1, 1, 13), env(Some("run-7"), true));
        assert_eq!(pids(&find_tree(&table, &request("run-7")).unwrap()), [100]);
    }

    #[test]
    fn this_process_other_users_processes_zombies_and_init_are_never_found() {
        let table = Fake::new();
        table.add(rec(9000, 1, 1, 1, 10), env(Some("run-7"), false));
        let mut other = rec(101, 2, 1, 1, 10);
        other.uid = 502;
        table.add(other, env(Some("run-7"), false));
        let mut zombie = rec(102, 3, 1, 1, 10);
        zombie.zombie = true;
        table.add(zombie, env(Some("run-7"), false));
        table.add(rec(1, 4, 0, 0, 1), env(Some("run-7"), false));
        table.add(rec(103, 5, 1, 1, 10), env(Some("run-7"), false));
        assert_eq!(pids(&find_tree(&table, &request("run-7")).unwrap()), [103]);
    }

    #[test]
    fn a_reused_pid_is_never_signalled() {
        let table = Fake::new();
        table.add(rec(100, 1, 1, 1, 10), env(Some("run-7"), false));
        let found = find_tree(&table, &request("run-7")).unwrap();
        // The process ended and another one got its pid before the signal.
        table.add(rec(100, 99, 1, 1, 50), env(Some("someone-else"), false));
        let mut problems = Vec::new();
        signal_all(&table, &found, libc::SIGKILL, &mut problems);
        assert!(table.sent().is_empty(), "{:?}", table.sent());
        assert!(problems.is_empty());
        assert!(!signal_if_same(&table, found[0], libc::SIGTERM).unwrap());
    }

    #[test]
    fn a_process_that_has_gone_is_not_signalled_and_is_no_error() {
        let table = Fake::new();
        table.add(rec(100, 1, 1, 1, 10), env(Some("run-7"), false));
        let found = find_tree(&table, &request("run-7")).unwrap();
        table.procs.borrow_mut().clear();
        assert!(!signal_if_same(&table, found[0], libc::SIGTERM).unwrap());
        assert!(table.sent().is_empty());
    }

    #[test]
    fn a_process_that_dies_of_sigterm_is_not_killed_and_one_that_ignores_it_is() {
        let table = Fake::new();
        table.add(rec(100, 1, 1, 1, 10), env(Some("run-7"), false));
        table.add(rec(101, 2, 1, 1, 10), env(Some("run-7"), false));
        table.mortal(100, &[libc::SIGTERM, libc::SIGKILL]);
        table.mortal(101, &[libc::SIGKILL]);
        let report = stop_tree(&table, &request("run-7"), None).unwrap();
        assert_eq!(report.signalled, [100, 101]);
        assert_eq!(report.killed, [101]);
        assert!(report.survivors.is_empty(), "{:?}", report.survivors);
        let sent = table.sent();
        assert!(sent.contains(&(100, libc::SIGTERM)));
        assert!(!sent.contains(&(100, libc::SIGKILL)));
        assert!(sent.contains(&(101, libc::SIGKILL)));
    }

    #[test]
    fn a_process_that_ignores_everything_is_a_survivor_of_the_found_kind() {
        let table = Fake::new();
        table.add(rec(100, 1, 1, 1, 10), env(Some("run-7"), false));
        let report = stop_tree(&table, &request("run-7"), None).unwrap();
        assert_eq!(
            report.survivors,
            [Survivor {
                pid: 100,
                command: "/fake/100".into(),
                kind: SurvivorKind::Found
            }]
        );
    }

    #[test]
    fn the_steps_run_in_the_order_of_the_constant() {
        // SIGTERM, then the grace wait, then SIGKILL, then the cleanup hook, then the report: the log of the fake
        // shows the first signal of each kind before the cleanup, and the cleanup before the last look.
        let table = Fake::new();
        table.add(rec(100, 1, 1, 1, 10), env(Some("run-7"), false));
        let hook = |_: &[Detached]| -> io::Result<()> {
            table.events.borrow_mut().push("cleanup");
            Ok(())
        };
        let report = stop_tree(&table, &request("run-7"), Some(&hook)).unwrap();
        let sent = table.sent();
        let term = sent.iter().position(|s| s.1 == libc::SIGTERM).unwrap();
        let kill = sent.iter().position(|s| s.1 == libc::SIGKILL).unwrap();
        assert!(term < kill);
        assert_eq!(table.events.borrow().as_slice(), ["cleanup"]);
        assert_eq!(report.cleanup, CleanupOutcome::Ran);
        // The process ignores both, so the final scan found it after the cleanup.
        assert_eq!(report.survivors.len(), 1);
    }

    #[test]
    fn a_stopped_process_is_continued_so_that_it_can_take_the_signal() {
        let table = Fake::new();
        table.add(rec(100, 1, 1, 1, 10), env(Some("run-7"), false));
        table.mortal(100, &[libc::SIGKILL]);
        stop_tree(&table, &request("run-7"), None).unwrap();
        assert!(table.sent().contains(&(100, libc::SIGCONT)));
    }

    #[test]
    fn the_cleanup_hook_gets_the_records_and_runs_when_nothing_was_found() {
        let table = Fake::new();
        let seen = RefCell::new(Vec::new());
        let hook = |detached: &[Detached]| -> io::Result<()> {
            seen.borrow_mut().extend_from_slice(detached);
            Ok(())
        };
        let mut request = request("run-7");
        let record = Detached::Process {
            pid: 77,
            started_unix_us: 5,
        };
        request.detached = vec![record.clone(), Detached::Container { id: "abc".into() }];
        let report = stop_tree(&table, &request, Some(&hook)).unwrap();
        assert_eq!(seen.borrow().len(), 2);
        assert_eq!(seen.borrow()[0], record);
        assert!(report.signalled.is_empty());
        assert!(report.survivors.is_empty());
    }

    #[test]
    fn a_cleanup_hook_that_fails_is_in_the_report_and_the_stop_goes_on() {
        let table = Fake::new();
        let hook = |_: &[Detached]| -> io::Result<()> { Err(io::Error::other("no docker")) };
        let report = stop_tree(&table, &request("run-7"), Some(&hook)).unwrap();
        assert_eq!(report.cleanup, CleanupOutcome::Failed("no docker".into()));
    }

    #[test]
    fn a_recorded_process_that_is_alive_after_the_cleanup_is_a_survivor_and_a_dead_one_is_not() {
        let table = Fake::new();
        table.add(rec(300, 1, 1, 1, 100), EnvRead::Hidden);
        table.add(rec(301, 2, 1, 1, 100), EnvRead::Hidden);
        let mut request = request("run-7");
        request.detached = vec![
            Detached::Process {
                pid: 300,
                started_unix_us: 100,
            },
            Detached::Process {
                pid: 301,
                started_unix_us: 100,
            },
            // Recorded with a start time that is not the process's now: that pid was reused.
            Detached::Process {
                pid: 302,
                started_unix_us: 100,
            },
        ];
        table.add(rec(302, 3, 1, 1, 555), EnvRead::Hidden);
        table.mortal(301, &[libc::SIGKILL]);
        let hook = |detached: &[Detached]| -> io::Result<()> {
            if let Some(Detached::Process { pid, .. }) = detached.get(1) {
                table.signal(*pid, libc::SIGKILL)?;
            }
            Ok(())
        };
        let report = stop_tree(&table, &request, Some(&hook)).unwrap();
        let recorded: Vec<u32> = report
            .survivors
            .iter()
            .filter(|s| s.kind == SurvivorKind::Recorded)
            .map(|s| s.pid)
            .collect();
        assert_eq!(recorded, [300]);
    }

    #[test]
    fn a_suspect_is_listed_and_never_signalled() {
        let table = Fake::new();
        // Parent 1, the user's, started after the run, environment hidden: a suspect.
        table.add(rec(400, 1, 1, 1, 2000), EnvRead::Hidden);
        // Environment readable and without the marker: not a suspect, whatever its parent.
        table.add(rec(401, 2, 1, 1, 2000), env(None, false));
        // Started before the run.
        table.add(rec(402, 3, 1, 1, 500), EnvRead::Hidden);
        // Another parent.
        table.add(rec(403, 4, 77, 9, 2000), EnvRead::Hidden);
        // The system's own services (Spotlight workers and the like) are launchd's children with a hidden
        // environment too; macOS records launchd as their parent exactly as it does for an orphan, so only the
        // executable's place tells them apart.
        for (pid, command) in [
            (
                404,
                "/System/Library/Frameworks/Metadata.framework/mdworker_shared",
            ),
            (405, "/usr/libexec/trustd"),
            (406, "/usr/sbin/cfprefsd"),
        ] {
            table.add(rec(pid, u64::from(pid), 1, 1, 2000), EnvRead::Hidden);
            table.named(pid, command);
        }
        // `/bin` and `/usr/bin` are where an orphan the criterion names lives, so they stay suspects.
        table.add(rec(407, 407, 1, 1, 2000), EnvRead::Hidden);
        table.named(407, "/usr/bin/yes");
        // An executable that cannot be read is no reason to leave a process out.
        table.add(rec(408, 408, 1, 1, 2000), EnvRead::Hidden);
        table.named(408, "(unknown)");
        let mut request = request("run-7");
        request.run_start_us = Some(1000);
        let report = stop_tree(&table, &request, None).unwrap();
        let suspects: Vec<u32> = report.survivors.iter().map(|s| s.pid).collect();
        assert_eq!(suspects, [400, 407, 408]);
        assert!(
            report
                .survivors
                .iter()
                .all(|s| s.kind == SurvivorKind::Suspect)
        );
        assert!(table.sent().is_empty(), "{:?}", table.sent());
    }

    #[test]
    fn there_are_no_suspects_without_a_start_clock() {
        let table = Fake::new();
        table.add(rec(400, 1, 1, 1, 2000), EnvRead::Hidden);
        let report = stop_tree(&table, &request("run-7"), None).unwrap();
        assert!(report.survivors.is_empty());
    }

    #[test]
    fn detached_records_are_written_and_read_back_in_order() {
        let dir =
            std::env::temp_dir().join(format!("collide-stop-detached-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        assert_eq!(read_detached(&dir).unwrap(), []);
        let first = Detached::Process {
            pid: 4,
            started_unix_us: 9,
        };
        let second = Detached::Container { id: "c1".into() };
        record_detached(&dir, &first).unwrap();
        record_detached(&dir, &second).unwrap();
        assert_eq!(read_detached(&dir).unwrap(), [first, second]);
        fs::write(dir.join(DETACHED_FILE), "{").unwrap();
        assert_eq!(
            read_detached(&dir).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        fs::remove_file(dir.join(DETACHED_FILE)).unwrap();
        fs::remove_dir(&dir).unwrap();
    }
}
