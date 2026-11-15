//! Real processes a test starts and signals, and the guard that keeps a stop from reaching any other (ts-0059-xs).
//!
//! A stop finds processes by scanning the whole process table, so every test that stops real processes goes through
//! [`Guarded`]: its `signal` panics, before anything is sent, for a pid that is not one the test recorded (by pid and
//! start time) or a descendant of one, and it logs the refusal so a test that ran the stop on a thread other than its
//! own still fails ([`Guarded::assert_nothing_refused`]).

use collide_host::stop::{Processes, StopRequest, SystemProcesses, find_tree};
use collide_host::sysproc::{self, EnvRead, ProcRecord};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A run id no other process on this machine carries.
pub fn unique_run_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!(
        "test-{}-{nanos}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    )
}

/// The root of a hostile tree. It ignores SIGHUP and SIGTERM, and starts a grandchild in a session of its own
/// (ignoring both too) and a double-forked orphan (ignoring both too, since the signal dispositions are inherited).
pub const HOSTILE: &str = r#"
import os, signal, subprocess, sys, time
signal.signal(signal.SIGHUP, signal.SIG_IGN)
signal.signal(signal.SIGTERM, signal.SIG_IGN)
pidfile = sys.argv[1]
def note(tag):
    with open(pidfile, 'a') as f:
        f.write('%s %d\n' % (tag, os.getpid()))
grandchild = (
    "import os, signal, sys, time\n"
    "signal.signal(signal.SIGHUP, signal.SIG_IGN)\n"
    "signal.signal(signal.SIGTERM, signal.SIG_IGN)\n"
    "with open(sys.argv[1], 'a') as f:\n"
    "    f.write('setsid %d\\n' % os.getpid())\n"
    "time.sleep(120)\n"
)
subprocess.Popen([sys.executable, '-c', grandchild, pidfile], start_new_session=True)
if os.fork() == 0:
    os.setsid()
    if os.fork() == 0:
        note('orphan')
        time.sleep(120)
    os._exit(0)
os.wait()
note('root')
time.sleep(120)
"#;

/// [`HOSTILE`] whose root also reads its stdin to the end once it has written its pids, writes `eof <pid>` when the
/// read returns, and then goes on running with SIGHUP and SIGTERM ignored: a child that ignores EOF and hangup.
pub fn hostile_reading_stdin() -> String {
    let tail = "note('root')\ntime.sleep(120)\n";
    assert!(HOSTILE.contains(tail), "the fixture ends as expected");
    HOSTILE.replace(
        tail,
        "note('root')\nsys.stdin.buffer.read()\nnote('eof')\ntime.sleep(120)\n",
    )
}

/// A root that starts two double-forked orphans which become `/bin/sleep`, an Apple platform binary: the environment
/// of each is hidden, so no marker scan can see it.
pub const SLEEP_ORPHANS: &str = r#"
import os, sys, time
pidfile = sys.argv[1]
def note(tag):
    with open(pidfile, 'a') as f:
        f.write('%s %d\n' % (tag, os.getpid()))
def orphan(tag):
    if os.fork() == 0:
        os.setsid()
        if os.fork() == 0:
            note(tag)
            os.execv('/bin/sleep', ['sleep', '120'])
        os._exit(0)
    os.wait()
orphan('orphan_a')
orphan('orphan_b')
note('root')
time.sleep(120)
"#;

/// The processes a test started, by pid and start time. Whatever is in here is the test's to signal.
#[derive(Default)]
pub struct Owned {
    started: Mutex<BTreeMap<u32, u64>>,
}

impl Owned {
    pub fn add(&self, pid: u32) {
        let started = sysproc::start_time_us(pid).expect("a process the test just saw start");
        self.started.lock().unwrap().insert(pid, started);
    }

    /// The start time the test recorded for `pid`, if it recorded one.
    pub fn recorded_start(&self, pid: u32) -> Option<u64> {
        self.started.lock().unwrap().get(&pid).copied()
    }

    /// Whether `pid` is one the test recorded, still the same process.
    pub fn has(&self, pid: u32) -> bool {
        // `None == None` must not pass: a pid the test never recorded and the system cannot read is not the test's.
        match self.started.lock().unwrap().get(&pid) {
            Some(&started) => sysproc::start_time_us(pid) == Some(started),
            None => false,
        }
    }

    /// Whether `pid` is the test's: recorded, or below a recorded process.
    pub fn is_ours(&self, pid: u32) -> bool {
        let mut at = pid;
        for _ in 0..64 {
            if self.has(at) {
                return true;
            }
            match sysproc::parent_pid(at) {
                Some(parent) => at = parent,
                None => return false,
            }
        }
        false
    }

    /// SIGKILL to each recorded process that is still the one recorded. Nothing else is signalled.
    pub fn kill_all(&self) {
        for (pid, started) in self.all() {
            if sysproc::start_time_us(pid) == Some(started) {
                // SAFETY: `kill` takes two integers; the pid is one the test recorded about its own fixture, and its
                // start time still matches, so it is that process and not a reused pid.
                unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) };
            }
        }
    }

    pub fn all(&self) -> Vec<(u32, u64)> {
        self.started
            .lock()
            .unwrap()
            .iter()
            .map(|(&p, &s)| (p, s))
            .collect()
    }
}

/// The real table, with a `signal` that refuses, by panicking before anything is sent, a pid that is not the test's.
pub struct Guarded {
    inner: SystemProcesses,
    pub owned: Vec<Arc<Owned>>,
    /// Runs directories of the test's own daemons: a phase's child is the test's when `child.json` there names it.
    runs_dirs: Vec<PathBuf>,
    refused: Mutex<Vec<u32>>,
    /// What the stop's own scans last read of each pid (record and environment), so that a refusal can say what a
    /// process was even when it has ended by the time the message is made.
    seen: Mutex<BTreeMap<u32, (ProcRecord, Option<EnvRead>)>>,
    /// Pids the stop asked to signal that could no longer be read by then: nothing was sent to them, and
    /// [`Guarded::assert_nothing_refused`] checks from what the scan read of them that they were the test's.
    vanished: Mutex<Vec<u32>>,
}

impl Guarded {
    pub fn of(owned: Vec<Arc<Owned>>) -> Guarded {
        Guarded {
            inner: SystemProcesses,
            owned,
            runs_dirs: Vec::new(),
            refused: Mutex::new(Vec::new()),
            seen: Mutex::new(BTreeMap::new()),
            vanished: Mutex::new(Vec::new()),
        }
    }

    /// A guard that also takes as the test's the children that `child.json` files under `runs_dir` name (by pid and
    /// start time), and what is below them.
    pub fn under(runs_dir: PathBuf, owned: Vec<Arc<Owned>>) -> Guarded {
        Guarded {
            runs_dirs: vec![runs_dir],
            ..Guarded::of(owned)
        }
    }

    /// Whether the test started `pid`, or it is below something the test started.
    pub fn is_ours(&self, pid: u32) -> bool {
        self.owned.iter().any(|owned| owned.is_ours(pid)) || self.is_recorded_child_or_below(pid)
    }

    fn is_recorded_child_or_below(&self, pid: u32) -> bool {
        let children = self.recorded_children();
        let mut at = pid;
        for _ in 0..64 {
            if children
                .iter()
                .any(|&(child, started)| child == at && sysproc::start_time_us(at) == Some(started))
            {
                return true;
            }
            match sysproc::parent_pid(at) {
                Some(parent) => at = parent,
                None => return false,
            }
        }
        false
    }

    /// The `(pid, start time)` of every child that a `child.json` under the runs directories names.
    fn recorded_children(&self) -> Vec<(u32, u64)> {
        let mut found = Vec::new();
        for runs_dir in &self.runs_dirs {
            let Ok(runs) = fs::read_dir(runs_dir) else {
                continue;
            };
            for run in runs.flatten() {
                let Ok(phases) = fs::read_dir(run.path()) else {
                    continue;
                };
                for phase in phases.flatten() {
                    let Ok(text) = fs::read_to_string(phase.path().join("child.json")) else {
                        continue;
                    };
                    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
                        continue;
                    };
                    if let Some(id) = collide_host::shim::ChildIdentity::from_json(&value) {
                        found.push((id.pid, id.started_unix_us));
                    }
                }
            }
        }
        found
    }

    /// What can be told of `pid` and its ancestry, for a refusal's message: the record, the command and the parents.
    fn describe(&self, pid: u32) -> String {
        let seen = self.seen.lock().unwrap().get(&pid).cloned();
        let mut parts = vec![format!("as the stop's scan read it: {seen:?}")];
        let mut at = pid;
        for _ in 0..8 {
            let record = sysproc::record_of(at);
            parts.push(format!(
                "pid {at}: {record:?} command {:?} run marker {:?}",
                self.inner.command(at),
                sysproc::env_vars_of(at, &["COLLIDE_RUN"])
            ));
            match record {
                Some(record) if record.parent > 1 => at = record.parent,
                _ => break,
            }
        }
        parts.join("; ")
    }

    /// Whether what the stop's scan read of `pid` (by pid and start time) is the test's: recorded, a recorded child
    /// of a daemon, or a child of such a process.
    fn was_ours_when_seen(&self, pid: u32) -> bool {
        let children = self.recorded_children();
        let mut at = pid;
        for _ in 0..8 {
            let Some((record, _)) = self.seen.lock().unwrap().get(&at).cloned() else {
                return false;
            };
            let recorded = self
                .owned
                .iter()
                .any(|owned| owned.recorded_start(at) == Some(record.started_us))
                || children
                    .iter()
                    .any(|&(child, started)| child == at && started == record.started_us);
            if recorded {
                return true;
            }
            at = record.parent;
        }
        false
    }

    /// The pids the guard refused.
    pub fn refused_pids(&self) -> Vec<u32> {
        self.refused.lock().unwrap().clone()
    }

    /// Panics if the stop asked to signal a process that has since ended and was not the test's by what the scan read
    /// of it.
    pub fn assert_vanished_were_ours(&self) {
        for &pid in self.vanished.lock().unwrap().iter() {
            assert!(
                self.was_ours_when_seen(pid),
                "the stop asked to signal pid {pid}, which has ended and was not the test's ({})",
                self.describe(pid)
            );
        }
    }

    /// Panics if the guard ever refused a signal, on whichever thread the stop ran, or if the stop asked to signal a
    /// process that has since ended and was not the test's by what the scan read of it.
    pub fn assert_nothing_refused(&self) {
        self.assert_vanished_were_ours();
        let refused = self.refused.lock().unwrap();
        assert!(
            refused.is_empty(),
            "the guard refused to signal pids {refused:?}, which the test did not start"
        );
    }
}

impl Processes for Guarded {
    fn own_uid(&self) -> u32 {
        self.inner.own_uid()
    }
    fn own_pid(&self) -> u32 {
        self.inner.own_pid()
    }
    fn pids(&self) -> io::Result<Vec<u32>> {
        self.inner.pids()
    }
    fn record(&self, pid: u32) -> Option<ProcRecord> {
        let record = self.inner.record(pid)?;
        self.seen.lock().unwrap().insert(pid, (record, None));
        Some(record)
    }
    fn environment(&self, pid: u32) -> EnvRead {
        let env = self.inner.environment(pid);
        if let Some(entry) = self.seen.lock().unwrap().get_mut(&pid) {
            entry.1 = Some(env.clone());
        }
        env
    }
    fn command(&self, pid: u32) -> String {
        self.inner.command(pid)
    }
    fn signal(&self, pid: u32, signal: i32) -> io::Result<()> {
        if !self.is_ours(pid) {
            // A process of the test's that is ending cannot be read, so it looks like a stranger for a moment. A pid
            // that cannot be read at all gets nothing sent (it is gone, as `kill` would say); one that can be read and
            // is not the test's is refused.
            if sysproc::record_of(pid).is_none() && sysproc::start_time_us(pid).is_none() {
                self.vanished.lock().unwrap().push(pid);
                return Err(io::Error::from_raw_os_error(libc::ESRCH));
            }
            self.refused.lock().unwrap().push(pid);
            panic!(
                "refusing to signal pid {pid}: this test did not start it, and nothing was sent ({})",
                self.describe(pid)
            );
        }
        self.inner.signal(pid, signal)
    }
}

/// Panics, with nothing signalled, unless every pid a stop would signal now is one of the test's.
pub fn assert_the_kill_set_is_ours(table: &Guarded, request: &StopRequest) {
    let found = find_tree(table, request).expect("the scan reads the table");
    for id in &found {
        assert!(
            table.is_ours(id.pid),
            "the stop would signal pid {} ({}), which this test did not start; nothing was sent",
            id.pid,
            table.command(id.pid)
        );
    }
}

pub fn read_pids(pidfile: &PathBuf, tags: &[&str]) -> Option<BTreeMap<String, u32>> {
    let text = fs::read_to_string(pidfile).ok()?;
    let found: BTreeMap<String, u32> = text
        .lines()
        .filter_map(|line| {
            let (tag, pid) = line.split_once(' ')?;
            Some((tag.to_string(), pid.parse().ok()?))
        })
        .collect();
    tags.iter().all(|t| found.contains_key(*t)).then_some(found)
}

/// The pids `ps` still lists out of `pids`.
pub fn listed_by_ps(pids: &[u32]) -> Vec<u32> {
    let list: Vec<String> = pids.iter().map(u32::to_string).collect();
    let output = Command::new("/bin/ps")
        .args(["-o", "pid=", "-p", &list.join(",")])
        .output()
        .expect("ps runs");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.trim().parse().ok())
        .collect()
}
