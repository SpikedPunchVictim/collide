//! The stop of 0.5 over real processes (ts-0059-xs): the marker scan, the ppid walk and the order, with python3 trees
//! that fight back (a setsid grandchild, a double-forked orphan, children that ignore SIGHUP and SIGTERM) and
//! double-forked `/bin/sleep` orphans, which an Apple platform binary's hidden environment makes invisible to a scan.
//!
//! SAFETY of these tests. A stop signals processes it finds by scanning the whole process table, so every rule below
//! is about making sure it can only ever reach what the test started:
//!
//! - Every test uses a run id of its own (`test-<pid>-<nanos>-<n>`), so no process on the machine can carry it.
//! - The table the stop signals through is [`Guarded`]: its `signal` panics, before anything is sent, for a pid that is
//!   not one the test recorded (by pid and start time) or a descendant of one. The pids are the ones the test's python
//!   fixtures wrote down themselves.
//! - Before the first signal of a stop, the test computes the kill set with a read-only scan and asserts that every pid
//!   in it is the test's ([`assert_the_kill_set_is_ours`]).
//! - Every process a test starts has a cleared environment and lives at most two minutes by itself; a guard kills what
//!   is left, by pid and start time, when the test ends.

mod common;

use collide_host::stop::{
    CleanupOutcome, DEFAULT_GRACE, Detached, ProcId, Processes, Root, StopRequest, SurvivorKind,
    find_tree, read_detached, record_detached, stop_tree,
};
use collide_host::sysproc::{self, EnvRead};
use common::procs::{
    Guarded, HOSTILE, Owned, SLEEP_ORPHANS, assert_the_kill_set_is_ours, listed_by_ps, read_pids,
    unique_run_id,
};
use common::{PYTHON, TempRoot, wait_until};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn now_us() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64
}

/// A chain root -> a -> b where only the root carries the run's marker: the children are started with a cleared
/// environment, so they are found as descendants of a marker hit.
const CHAIN: &str = r#"
import os, signal, subprocess, sys, time
signal.signal(signal.SIGTERM, signal.SIG_IGN)
pidfile, tag = sys.argv[1], sys.argv[2]
with open(pidfile, 'a') as f:
    f.write('%s %d\n' % (tag, os.getpid()))
following = {'root': 'a', 'a': 'b'}.get(tag)
if following:
    subprocess.Popen([sys.executable, __file__, pidfile, following], env={'PATH': '/usr/bin:/bin'})
time.sleep(120)
"#;

fn guarded(trees: &[&Tree]) -> Guarded {
    Guarded::of(trees.iter().map(|t| t.owned.clone()).collect())
}

/// A python3 tree of the test's, its pid file, and a guard that kills whatever is left.
struct Tree {
    root: Option<Child>,
    owned: Arc<Owned>,
    pids: BTreeMap<String, u32>,
    run_id: String,
}

impl Tree {
    /// Starts `script` with `COLLIDE_RUN=<run_id>` as the only variable but `PATH`, and waits for the processes the
    /// script names in `tags` to write their pids.
    fn start(root: &TempRoot, run_id: &str, script: &str, tags: &[&str]) -> Tree {
        let script_path = root.join(&format!("{}.py", run_id.replace('/', "_")));
        fs::write(&script_path, script).unwrap();
        let pidfile = root.join(&format!("{}.pids", run_id.replace('/', "_")));
        fs::write(&pidfile, "").unwrap();
        let mut command = Command::new(PYTHON);
        command
            .arg(&script_path)
            .arg(&pidfile)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", root.path())
            .env("COLLIDE_RUN", run_id)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if script == CHAIN {
            command.arg("root");
        }
        let child = command.spawn().expect("python3 starts");
        let mut tree = Tree {
            root: Some(child),
            owned: Arc::new(Owned::default()),
            pids: BTreeMap::new(),
            run_id: run_id.to_string(),
        };
        tree.owned.add(tree.root_pid());
        wait_until(
            &format!("the fixture {tags:?} to write its pids"),
            Duration::from_secs(20),
            || read_pids(&pidfile, tags).is_some(),
        );
        tree.pids = read_pids(&pidfile, tags).unwrap();
        for pid in tree.pids.values() {
            tree.owned.add(*pid);
        }
        tree
    }

    fn root_pid(&self) -> u32 {
        self.root.as_ref().unwrap().id()
    }

    fn pid(&self, tag: &str) -> u32 {
        self.pids[tag]
    }

    /// Every pid the test recorded.
    fn all_pids(&self) -> Vec<u32> {
        self.owned.all().into_iter().map(|(p, _)| p).collect()
    }

    fn root(&self) -> Root {
        Root {
            pid: self.root_pid(),
            started_us: sysproc::start_time_us(self.root_pid()).unwrap(),
        }
    }

    /// Whether the process the test recorded as `pid` is still running (not a zombie).
    fn alive(&self, pid: u32) -> bool {
        self.owned.has(pid) && sysproc::is_zombie(pid) == Some(false)
    }

    /// Reaps the root, which is the test's own child, so that it is not left a zombie for `ps` to list.
    fn reap_root(&mut self) {
        if let Some(mut root) = self.root.take() {
            let _ = root.wait();
        }
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        for (pid, started) in self.owned.all() {
            // Only a process the test recorded, only while it is still the same one.
            if sysproc::start_time_us(pid) == Some(started) {
                // SAFETY: `kill` takes two integers; the pid is one this test's fixture wrote down about itself, and
                // its start time still matches the one recorded, so it is that process and not a reused pid.
                unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) };
            }
        }
        self.reap_root();
        let _ = &self.run_id;
    }
}

/// A request with a grace and waits short enough for a test.
fn request(run_id: &str) -> StopRequest {
    let mut request = StopRequest::new(run_id);
    request.grace = Duration::from_millis(300);
    request.poll = Duration::from_millis(20);
    request.settle = Duration::from_secs(3);
    request
}

fn pids_of(found: &[ProcId]) -> Vec<u32> {
    let mut pids: Vec<u32> = found.iter().map(|id| id.pid).collect();
    pids.sort();
    pids
}

fn sorted(mut pids: Vec<u32>) -> Vec<u32> {
    pids.sort();
    pids
}

#[test]
fn the_guard_panics_before_it_signals_a_process_the_test_did_not_start() {
    let root = TempRoot::new();
    let id = unique_run_id();
    let tree = Tree::start(&root, &id, CHAIN, &["root", "a", "b"]);
    let table = guarded(&[&tree]);
    // This test's own pid is not one the tree recorded. SIGCONT is harmless, and the panic comes before it anyway.
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = table.signal(std::process::id(), libc::SIGCONT);
    }));
    assert!(outcome.is_err(), "the guard refused");
    assert!(
        tree.all_pids().iter().all(|&p| tree.alive(p)),
        "nothing of the tree was touched"
    );
}

#[test]
fn a_stop_leaves_zero_processes_of_a_hostile_tree_found_by_the_marker_alone() {
    let root = TempRoot::new();
    let id = unique_run_id();
    let mut tree = Tree::start(&root, &id, HOSTILE, &["root", "setsid", "orphan"]);
    let table = guarded(&[&tree]);
    let mut request = request(&id);
    request.run_start_us = Some(now_us() - 60_000_000);
    assert_the_kill_set_is_ours(&table, &request);

    let report = stop_tree(&table, &request, None).unwrap();

    let all = tree.all_pids();
    assert_eq!(sorted(report.signalled.clone()), sorted(all.clone()));
    assert_eq!(
        sorted(report.killed.clone()),
        sorted(all.clone()),
        "all three ignore SIGTERM, so all three needed SIGKILL"
    );
    tree.reap_root();
    assert_eq!(listed_by_ps(&all), Vec::<u32>::new(), "ps lists none");
    assert!(
        report
            .survivors
            .iter()
            .all(|s| s.kind == SurvivorKind::Suspect),
        "{:?}",
        report.survivors
    );
    assert!(report.problems.is_empty(), "{:?}", report.problems);
}

#[test]
fn the_ppid_walk_alone_reaches_the_child_and_its_setsid_grandchild_but_not_the_orphan() {
    let root = TempRoot::new();
    let id = unique_run_id();
    let tree = Tree::start(&root, &id, HOSTILE, &["root", "setsid", "orphan"]);
    let table = guarded(&[&tree]);
    // A marker nothing carries, so only the recorded root and its descendants are found.
    let other = unique_run_id();
    let mut request = request(&other);
    request.roots = vec![tree.root()];
    assert_the_kill_set_is_ours(&table, &request);

    let found = find_tree(&table, &request).unwrap();
    assert_eq!(
        pids_of(&found),
        sorted(vec![tree.pid("root"), tree.pid("setsid")])
    );

    let report = stop_tree(&table, &request, None).unwrap();
    assert_eq!(
        sorted(report.signalled),
        sorted(vec![tree.pid("root"), tree.pid("setsid")])
    );
    assert!(
        tree.alive(tree.pid("orphan")),
        "the orphan is not found by the walk"
    );
}

#[test]
fn the_descendants_of_a_marker_hit_are_found_though_they_carry_no_marker() {
    let root = TempRoot::new();
    let id = unique_run_id();
    let mut tree = Tree::start(&root, &id, CHAIN, &["root", "a", "b"]);
    let table = guarded(&[&tree]);
    let request = request(&id);
    assert_the_kill_set_is_ours(&table, &request);
    // Only the root carries the marker.
    assert_eq!(
        sysproc::env_vars_of(tree.pid("a"), &["COLLIDE_RUN"]),
        EnvRead::Read(vec![None])
    );
    let found = find_tree(&table, &request).unwrap();
    assert_eq!(pids_of(&found), sorted(tree.all_pids()));

    let report = stop_tree(&table, &request, None).unwrap();
    tree.reap_root();
    assert_eq!(listed_by_ps(&tree.all_pids()), Vec::<u32>::new());
    assert!(
        report
            .survivors
            .iter()
            .all(|s| s.kind == SurvivorKind::Suspect)
    );
}

#[test]
fn another_runs_processes_are_not_touched() {
    let root = TempRoot::new();
    let (first_id, second_id) = (unique_run_id(), unique_run_id());
    let mut first = Tree::start(&root, &first_id, CHAIN, &["root", "a", "b"]);
    let second = Tree::start(&root, &second_id, CHAIN, &["root", "a", "b"]);
    let table = guarded(&[&first, &second]);
    let request = request(&first_id);
    assert_the_kill_set_is_ours(&table, &request);
    assert_eq!(
        pids_of(&find_tree(&table, &request).unwrap()),
        sorted(first.all_pids())
    );

    stop_tree(&table, &request, None).unwrap();

    first.reap_root();
    assert_eq!(listed_by_ps(&first.all_pids()), Vec::<u32>::new());
    assert!(
        second.all_pids().iter().all(|&p| second.alive(p)),
        "the second run's processes are all still running"
    );
}

#[test]
fn a_root_whose_start_time_is_not_the_recorded_one_is_never_signalled() {
    let root = TempRoot::new();
    let id = unique_run_id();
    let tree = Tree::start(&root, &id, CHAIN, &["root", "a", "b"]);
    let table = guarded(&[&tree]);
    let other = unique_run_id();
    let mut request = request(&other);
    // The pid of a live process, with a start time that is not its own: as if its pid were reused.
    request.roots = vec![Root {
        pid: tree.root_pid(),
        started_us: tree.root().started_us + 1,
    }];
    let report = stop_tree(&table, &request, None).unwrap();
    assert!(report.signalled.is_empty(), "{:?}", report.signalled);
    assert!(tree.all_pids().iter().all(|&p| tree.alive(p)));
}

#[test]
fn a_recorded_sleep_orphan_is_stopped_by_the_cleanup_hook_and_an_unrecorded_one_is_reported() {
    let root = TempRoot::new();
    let id = unique_run_id();
    let run_start = now_us() - 1_000_000;
    let mut tree = Tree::start(&root, &id, SLEEP_ORPHANS, &["root", "orphan_a", "orphan_b"]);
    let (recorded, unrecorded) = (tree.pid("orphan_a"), tree.pid("orphan_b"));
    // Apple's `sleep` hides its environment and its parent is launchd: no scan finds either.
    // The fixture notes its pid before it execs `sleep` and before its parent exits, so both are waited for.
    for pid in [recorded, unrecorded] {
        wait_until(
            "the orphan is exec'd into sleep and reparented to launchd",
            Duration::from_secs(10),
            || {
                sysproc::env_vars_of(pid, &["COLLIDE_RUN"]) == EnvRead::Hidden
                    && sysproc::parent_pid(pid) == Some(1)
            },
        );
    }
    // The hook that started the first one recorded it, as the plan has hooks do.
    let run_dir = root.join("run-dir");
    fs::create_dir_all(&run_dir).unwrap();
    record_detached(
        &run_dir,
        &Detached::Process {
            pid: recorded,
            started_unix_us: sysproc::start_time_us(recorded).unwrap(),
        },
    )
    .unwrap();

    let table = guarded(&[&tree]);
    let mut request = request(&id);
    request.run_start_us = Some(run_start);
    request.detached = read_detached(&run_dir).unwrap();
    assert_the_kill_set_is_ours(&table, &request);
    // The cleanup hook's stand-in: it stops each recorded process, by pid and the recorded start time, and nothing
    // else. It signals through the guarded table, so it can reach only what the test started.
    let cleanup = |detached: &[Detached]| -> io::Result<()> {
        for item in detached {
            if let Detached::Process {
                pid,
                started_unix_us,
            } = item
                && sysproc::start_time_us(*pid) == Some(*started_unix_us)
            {
                table.signal(*pid, libc::SIGKILL)?;
            }
        }
        Ok(())
    };

    let report = stop_tree(&table, &request, Some(&cleanup)).unwrap();

    assert_eq!(report.cleanup, CleanupOutcome::Ran);
    assert!(
        !report.signalled.contains(&recorded) && !report.signalled.contains(&unrecorded),
        "the scans never saw the orphans: {:?}",
        report.signalled
    );
    assert!(!tree.alive(recorded), "the recorded orphan was stopped");
    assert!(
        tree.alive(unrecorded),
        "a stop never kills what it cannot name"
    );
    let pids: Vec<u32> = report.survivors.iter().map(|s| s.pid).collect();
    assert!(!pids.contains(&recorded), "{:?}", report.survivors);
    let suspect = report
        .survivors
        .iter()
        .find(|s| s.pid == unrecorded)
        .unwrap_or_else(|| {
            panic!(
                "the unrecorded orphan is in the report: {:?}",
                report.survivors
            )
        });
    assert_eq!(suspect.kind, SurvivorKind::Suspect);
    assert!(suspect.command.ends_with("/sleep"), "{}", suspect.command);

    // The test removes what it started.
    // SAFETY: `kill` takes two integers; the pid is the unrecorded orphan this test's fixture started, and its start
    // time is the one recorded when the fixture reported it.
    if tree.alive(unrecorded) {
        unsafe { libc::kill(unrecorded as libc::pid_t, libc::SIGKILL) };
    }
    tree.reap_root();
}

#[test]
fn a_stop_with_an_empty_run_id_is_refused_and_signals_nothing() {
    let root = TempRoot::new();
    let id = unique_run_id();
    let tree = Tree::start(&root, &id, CHAIN, &["root", "a", "b"]);
    let table = guarded(&[&tree]);
    let mut request = request(&id);
    request.marker = String::new();
    assert!(find_tree(&table, &request).is_err());
    assert!(stop_tree(&table, &request, None).is_err());
    assert!(tree.all_pids().iter().all(|&p| tree.alive(p)));
}

/// The guard must stay strict about a stranger and must not call the test's own process a stranger while it is
/// ending. Signal 0 is sent in these tests: it checks a pid and delivers nothing.
#[test]
fn the_guard_refuses_a_stranger_and_lets_an_ending_process_of_the_tests_pass() {
    let root = TempRoot::new();
    let owned = Arc::new(Owned::default());
    let mut child = Command::new(PYTHON)
        .args(["-c", "import time; time.sleep(60)"])
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", root.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("python3 starts");
    owned.add(child.id());
    let guard = Guarded::of(vec![owned.clone()]);
    // A stranger that can be read: this test's own parent (cargo), which no test started.
    let stranger = sysproc::parent_pid(std::process::id()).expect("a parent");
    let refused = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = guard.signal(stranger, 0);
    }));
    assert!(refused.is_err(), "a readable stranger is refused");
    assert!(guard.signal(child.id(), 0).is_ok(), "the test's own is not");

    // The test's child, read by a scan and then ended before the stop's signal reaches the guard.
    let seen = guard.record(child.id()).expect("the child is running");
    child.kill().expect("the test's own child ends");
    child.wait().expect("and is reaped");
    let error = guard.signal(child.id(), 0).expect_err("it is gone");
    assert_eq!(error.raw_os_error(), Some(libc::ESRCH), "{seen:?}");
    // The one refusal above is the stranger's, and it is the only one.
    assert_eq!(guard.refused_pids(), [stranger]);
    guard.assert_vanished_were_ours();
}

#[test]
fn the_default_grace_is_the_plans() {
    assert_eq!(StopRequest::new("x").grace, DEFAULT_GRACE);
}
