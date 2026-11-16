//! The daemon's side of the shim link (ts-0057-ck): phases started and adopted by a daemon that runs inside the test
//! process, with real `collide shim` processes and python3 children.
//!
//! What is checked here: a phase's states and the events they leave, the exit code and totals kept from `exit.json`,
//! a shim killed with SIGKILL (the phase is `lost` even though the agent's stream said success), a shim stopped with
//! SIGSTOP (reported as unresponsive, and a second run unaffected), a daemon restarted over running shims, over shims
//! that ended and over shims that died, a damaged `exit.json`, and the names a phase may have.
//!
//! The daemon, every shim and every child run in a folder of the test's own under `/tmp`, with a cleared environment.
//! A guard stops the shims and children a test started, by pid and start time.

mod common;

use collide_core::events::EventBody;
use collide_core::shim::ExitEnd;
use collide_core::state::PhaseState;
use collide_host::base_env::CaptureOptions;
use collide_host::daemon::{Daemon, DaemonOptions, DaemonPaths};
use collide_host::phases::{
    Health, LinkError, PhaseEnd, PhaseKey, PhaseOptions, PhaseSpec, Phases,
};
use collide_host::shim::{ChildStdio, files};
use collide_host::stop::SurvivorKind;
use common::procs::{Guarded, Owned};
use common::{PYTHON, TempRoot, kill_child_of, signal_if_same, wait_until};
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const WAIT_STDIN: &str = "import sys; sys.stdin.read()";
const ECHO: &str = "import sys\nfor line in sys.stdin:\n    print('echo:' + line.strip())";
/// Says the agent succeeded, as a stream-json agent does, and then goes on running.
const RESULT_THEN_WAIT: &str = "import sys; print('{\"type\":\"result\",\"subtype\":\"success\"}'); sys.stdout.flush(); sys.stdin.read()";

#[derive(Clone, Default)]
struct LogBuf(Arc<Mutex<Vec<u8>>>);

impl LogBuf {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
    }
}

impl Write for LogBuf {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn fake_shell(root: &TempRoot) -> String {
    root.script("fake-shell", "#!/bin/sh\neval \"$4\"\n")
        .to_str()
        .unwrap()
        .to_string()
}

/// Short waits, so a test that stops a shim does not take seconds to see it.
fn quick() -> PhaseOptions {
    PhaseOptions {
        reply_wait: Duration::from_millis(500),
        unresponsive_after: Duration::from_millis(300),
        ..PhaseOptions::default()
    }
}

/// The guard of a test's daemon: it signals a pid only when `child.json` under the test's own runs directory names it,
/// or the pid is below such a child.
fn guard_of(root: &TempRoot, owned: Vec<Arc<Owned>>) -> Arc<Guarded> {
    Arc::new(Guarded::under(
        DaemonPaths::under(root.path()).runs_dir,
        owned,
    ))
}

fn options(root: &TempRoot, log: &LogBuf) -> DaemonOptions {
    let mut options = DaemonOptions::new(
        DaemonPaths::under(root.path()),
        fake_shell(root),
        Box::new(log.clone()),
    );
    options.process_tree = common::test_process_tree();
    options.capture = CaptureOptions {
        timeout: Duration::from_secs(10),
        cap_bytes: 1024 * 1024,
        fallback: BTreeMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string())]),
    };
    options.shim_exe = Some(PathBuf::from(env!("CARGO_BIN_EXE_collide")));
    options.phases = quick();
    options.processes = guard_of(root, Vec::new());
    options
}

fn start(root: &TempRoot) -> (Daemon, LogBuf) {
    let log = LogBuf::default();
    let daemon = Daemon::start(options(root, &log)).expect("the daemon starts");
    (daemon, log)
}

/// A key whose run id no other process on the machine carries: a stop finds processes by their run id, so a test never
/// uses a bare `r1`.
fn key(run: &str, phase: &str) -> PhaseKey {
    PhaseKey::new(&unique_run(run), phase).expect("a good name")
}

fn unique_run(name: &str) -> String {
    static PREFIX: OnceLock<String> = OnceLock::new();
    let prefix = PREFIX.get_or_init(|| {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!("test-{}-{nanos}", std::process::id())
    });
    format!("{prefix}-{name}")
}

fn python(script: &str, cwd: &Path) -> PhaseSpec {
    PhaseSpec {
        program: PYTHON.into(),
        args: vec!["-u".into(), "-c".into(), script.into()],
        env: BTreeMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string())]),
        cwd: cwd.to_path_buf(),
        stdio: ChildStdio::Pipes,
    }
}

fn dir_of(paths: &DaemonPaths, key: &PhaseKey) -> PathBuf {
    paths.runs_dir.join(&key.run).join(&key.phase)
}

/// Stops what a test started: each tracked shim by pid and start time, and each phase's child by `child.json`.
struct Cleanup {
    dirs: Vec<PathBuf>,
    shims: Vec<(u32, u64)>,
}

impl Cleanup {
    fn new() -> Cleanup {
        Cleanup {
            dirs: Vec::new(),
            shims: Vec::new(),
        }
    }

    /// Remembers a phase's shim (once the daemon knows its pid) and its directory.
    fn track(&mut self, phases: &Phases, paths: &DaemonPaths, key: &PhaseKey) {
        self.dirs.push(dir_of(paths, key));
        let mut pid = None;
        wait_until("the shim's pid is known", Duration::from_secs(10), || {
            pid = phases.shim_pid(key);
            pid.is_some()
        });
        let pid = pid.unwrap();
        if let Some(started) = collide_host::sysproc::start_time_us(pid) {
            self.shims.push((pid, started));
        }
    }
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        for (pid, started) in &self.shims {
            signal_if_same(*pid, *started, libc::SIGCONT);
            signal_if_same(*pid, *started, libc::SIGKILL);
        }
        for dir in &self.dirs {
            kill_child_of(dir);
        }
    }
}

fn wait_state(phases: &Phases, key: &PhaseKey, what: &str, want: impl Fn(&PhaseState) -> bool) {
    wait_until(what, Duration::from_secs(15), || {
        phases.state(key).is_some_and(|state| want(&state))
    });
}

fn running(state: &PhaseState) -> bool {
    matches!(state, PhaseState::Running {})
}

fn ended(state: &PhaseState) -> bool {
    matches!(state, PhaseState::Ended {})
}

fn lost(state: &PhaseState) -> bool {
    matches!(state, PhaseState::Lost {})
}

fn transitions(daemon: &Daemon, key: &PhaseKey) -> Vec<(Option<PhaseState>, PhaseState)> {
    daemon
        .events_since(0)
        .unwrap()
        .into_iter()
        .filter_map(|event| match event.body {
            EventBody::PhaseTransition {
                run,
                phase,
                from,
                to,
                ..
            } if run == key.run && phase == key.phase => Some((from, to)),
            _ => None,
        })
        .collect()
}

// ---- a phase the daemon started ---------------------------------------------------------------------------------

#[test]
fn a_started_phase_runs_ends_and_keeps_its_code_and_totals() {
    let root = TempRoot::new();
    let (daemon, _log) = start(&root);
    let phases = daemon.phases().clone();
    let k = key("r1", "p1");
    let mut cleanup = Cleanup::new();
    let script =
        "import sys, time\nt = time.time()\nwhile time.time() - t < 0.2:\n    pass\nsys.exit(7)";
    phases
        .start_phase(&k, &python(script, root.path()))
        .expect("the phase starts");
    cleanup.track(&phases, daemon.paths(), &k);

    // Exit code 7 is not a result: the phase failed, with no cause known.
    wait_state(&phases, &k, "the phase fails", |s| {
        matches!(s, PhaseState::Failed { cause: None })
    });
    let record = phases.exit_record(&k).expect("the daemon holds the record");
    assert_eq!(record.end, ExitEnd::Exited { code: 7 });
    assert!(record.cpu_user_us > 0, "{record:?}");
    let on_disk = collide_core::shim::parse_exit_record(
        &fs::read(dir_of(daemon.paths(), &k).join(files::EXIT_RECORD)).unwrap(),
    )
    .unwrap();
    assert_eq!(record, on_disk, "the daemon's record is exit.json's");

    // pending, running, then the end: each move one the state machine allows.
    let moves = transitions(&daemon, &k);
    assert_eq!(moves.len(), 3, "{moves:?}");
    assert_eq!(moves[0], (None, PhaseState::Pending {}));
    assert_eq!(
        moves[1],
        (Some(PhaseState::Pending {}), PhaseState::Running {})
    );
    assert_eq!(moves[2].0, Some(PhaseState::Running {}));

    // The shim was told the record is held, and ended.
    wait_until("the shim has ended", Duration::from_secs(10), || {
        phases.shim_exit(&k).is_some()
    });
    assert!(phases.shim_exit(&k).unwrap().success());
}

#[test]
fn a_clean_exit_ends_the_phase_and_the_end_hook_is_told() {
    let root = TempRoot::new();
    let seen: Arc<Mutex<Vec<PhaseEnd>>> = Arc::default();
    let log = LogBuf::default();
    let mut opts = options(&root, &log);
    let sink = seen.clone();
    opts.on_phase_end = Some(Box::new(move |_, end| {
        sink.lock().unwrap().push(end.clone())
    }));
    let daemon = Daemon::start(opts).unwrap();
    let phases = daemon.phases().clone();
    let k = key("r1", "p1");
    let mut cleanup = Cleanup::new();
    phases
        .start_phase(&k, &python("import sys; sys.exit(0)", root.path()))
        .unwrap();
    cleanup.track(&phases, daemon.paths(), &k);
    wait_state(&phases, &k, "the phase ends", ended);
    wait_until("the hook ran", Duration::from_secs(5), || {
        !seen.lock().unwrap().is_empty()
    });
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].key, k);
    assert_eq!(seen[0].state, PhaseState::Ended {});
    assert_eq!(
        seen[0].exit.as_ref().unwrap().end,
        ExitEnd::Exited { code: 0 }
    );
}

#[test]
fn a_write_reaches_the_child_and_closing_stdin_ends_it() {
    let root = TempRoot::new();
    let (daemon, _log) = start(&root);
    let phases = daemon.phases().clone();
    let k = key("r1", "p1");
    let mut cleanup = Cleanup::new();
    phases.start_phase(&k, &python(ECHO, root.path())).unwrap();
    cleanup.track(&phases, daemon.paths(), &k);
    // The journal's sequence also numbers the child's output, so the second input is after the first, not next to it.
    let first = phases.write(&k, "one\n").unwrap();
    assert_eq!(first, 0);
    assert!(phases.write(&k, "two\n").unwrap() > first);
    phases.close_stdin(&k).unwrap();
    wait_state(&phases, &k, "the phase ends", ended);
    let out = fs::read_to_string(dir_of(daemon.paths(), &k).join(files::STDOUT_LOG)).unwrap();
    assert_eq!(out, "echo:one\necho:two\n");
}

#[test]
fn a_phase_name_that_could_leave_the_runs_directory_is_refused() {
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
        assert!(PhaseKey::new(bad, "p1").is_err(), "run {bad:?}");
        assert!(PhaseKey::new("r1", bad).is_err(), "phase {bad:?}");
    }
    assert!(PhaseKey::new("run-0001.a_b", "implement-2").is_ok());
}

#[test]
fn a_phase_whose_socket_path_is_too_long_is_refused_before_anything_starts() {
    let root = TempRoot::new();
    let (daemon, _log) = start(&root);
    let phases = daemon.phases().clone();
    let k = PhaseKey::new(&"r".repeat(60), &"p".repeat(60)).unwrap();
    let error = phases
        .start_phase(&k, &python(WAIT_STDIN, root.path()))
        .expect_err("a path over 103 bytes");
    let message = error.to_string();
    assert!(message.contains("103"), "{message}");
    assert!(!dir_of(daemon.paths(), &k).join(files::SPEC).exists());
    assert!(phases.state(&k).is_none());
}

#[test]
fn a_phase_that_is_already_there_is_not_started_again() {
    let root = TempRoot::new();
    let (daemon, _log) = start(&root);
    let phases = daemon.phases().clone();
    let k = key("r1", "p1");
    let mut cleanup = Cleanup::new();
    phases
        .start_phase(&k, &python(WAIT_STDIN, root.path()))
        .unwrap();
    cleanup.track(&phases, daemon.paths(), &k);
    let error = phases
        .start_phase(&k, &python(WAIT_STDIN, root.path()))
        .expect_err("the name is taken");
    assert!(error.to_string().contains(&k.to_string()), "{error}");
    phases.close_stdin(&k).unwrap();
    wait_state(&phases, &k, "the phase ends", ended);
}

// ---- a shim that dies ------------------------------------------------------------------------------------------

#[test]
fn a_shim_killed_with_sigkill_makes_its_phase_lost_even_after_a_result() {
    let root = TempRoot::new();
    let (daemon, log) = start(&root);
    let phases = daemon.phases().clone();
    let k = key("r1", "p1");
    let mut cleanup = Cleanup::new();
    phases
        .start_phase(&k, &python(RESULT_THEN_WAIT, root.path()))
        .unwrap();
    cleanup.track(&phases, daemon.paths(), &k);
    wait_until(
        "the agent's result is in the log",
        Duration::from_secs(10),
        || {
            fs::read_to_string(dir_of(daemon.paths(), &k).join(files::STDOUT_LOG))
                .is_ok_and(|text| text.contains("\"result\""))
        },
    );
    wait_state(&phases, &k, "running", running);

    let pid = phases.shim_pid(&k).unwrap();
    let started = Instant::now();
    // SAFETY: `kill` takes two integers; the pid is the shim this test's daemon started.
    unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) };
    wait_state(&phases, &k, "the phase is lost", lost);
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
    assert!(
        !dir_of(daemon.paths(), &k).join(files::EXIT_RECORD).exists(),
        "a shim that was killed wrote no exit.json"
    );
    assert!(phases.exit_record(&k).is_none());
    let moves = transitions(&daemon, &k);
    assert_eq!(moves.last().unwrap().1, PhaseState::Lost {}, "{moves:?}");
    assert!(log.text().contains("lost"), "{}", log.text());
    // The phase is final: nothing writes to it any more.
    assert!(matches!(phases.write(&k, "x\n"), Err(LinkError::NoLink(_))));
}

#[test]
fn a_shim_that_died_while_the_daemon_was_down_makes_its_phase_lost_when_a_daemon_returns() {
    let root = TempRoot::new();
    let k = key("r1", "p1");
    let mut cleanup = Cleanup::new();
    let paths = DaemonPaths::under(root.path());
    {
        let (daemon, _log) = start(&root);
        let phases = daemon.phases().clone();
        phases
            .start_phase(&k, &python(RESULT_THEN_WAIT, root.path()))
            .unwrap();
        cleanup.track(&phases, daemon.paths(), &k);
        wait_state(&phases, &k, "running", running);
        let pid = phases.shim_pid(&k).unwrap();
        daemon.stop();
        // SAFETY: as above.
        unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) };
        wait_until("the shim is gone", Duration::from_secs(5), || {
            !common::pid_alive(pid) || phases.shim_exit(&k).is_some()
        });
    }
    let (daemon, _log) = start(&root);
    let phases = daemon.phases().clone();
    assert!(
        phases.state(&k).as_ref().is_some_and(lost),
        "adoption settles it before the socket is bound: {:?}",
        phases.state(&k)
    );
    let moves = transitions(&daemon, &k);
    assert_eq!(moves.last().unwrap().1, PhaseState::Lost {}, "{moves:?}");
    assert_eq!(phases.live(), 0);
    let _ = paths;
}

#[test]
fn a_damaged_exit_json_is_moved_aside_and_the_phase_is_lost() {
    let root = TempRoot::new();
    let paths = DaemonPaths::under(root.path());
    let k = key("r1", "p1");
    let dir = paths.runs_dir.join(&k.run).join(&k.phase);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(files::SPEC), "{}").unwrap();
    fs::write(
        dir.join(files::EXIT_RECORD),
        "{\"protocol\":1,\"end\":{\"typ",
    )
    .unwrap();
    let (daemon, log) = start(&root);
    assert!(daemon.phases().state(&k).as_ref().is_some_and(lost));
    assert!(
        !dir.join(files::EXIT_RECORD).exists(),
        "never read twice, never overwritten"
    );
    let moved: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("exit.json."))
        .collect();
    assert_eq!(moved.len(), 1, "{moved:?}");
    assert!(log.text().contains("exit.json"), "{}", log.text());
}

// ---- a daemon that restarts ----------------------------------------------------------------------------------

#[test]
fn a_restarted_daemon_adopts_the_running_shim_before_it_binds_and_reattaches_to_its_phase() {
    let root = TempRoot::new();
    let k = key("r1", "p1");
    let mut cleanup = Cleanup::new();
    let first_pid;
    {
        let (daemon, _log) = start(&root);
        let phases = daemon.phases().clone();
        phases.start_phase(&k, &python(ECHO, root.path())).unwrap();
        cleanup.track(&phases, daemon.paths(), &k);
        first_pid = phases.shim_pid(&k).unwrap();
        assert_eq!(phases.write(&k, "before\n").unwrap(), 0);
        wait_until("the line is echoed", Duration::from_secs(10), || {
            fs::read_to_string(dir_of(daemon.paths(), &k).join(files::STDOUT_LOG))
                .is_ok_and(|t| t.contains("echo:before"))
        });
        daemon.stop();
    }
    assert!(common::pid_alive(first_pid), "the shim outlives its daemon");

    let (daemon, log) = start(&root);
    let phases = daemon.phases().clone();
    // Adoption ran inside `start`, before the bind: the phase is already running and counted.
    assert!(phases.state(&k).as_ref().is_some_and(running));
    assert_eq!(phases.live(), 1);
    assert!(log.text().contains("adopted"), "{}", log.text());
    wait_until("the link is up", Duration::from_secs(10), || {
        matches!(phases.health(&k), Some(Health::Responsive))
    });
    assert_eq!(phases.shim_pid(&k), Some(first_pid), "the same shim");
    // Input goes on from where it was: the journal's sequence continues (the echo of `before` is in it).
    assert!(phases.write(&k, "after\n").unwrap() > 0);
    phases.close_stdin(&k).unwrap();
    wait_state(&phases, &k, "the phase ends", ended);
    let out = fs::read_to_string(dir_of(daemon.paths(), &k).join(files::STDOUT_LOG)).unwrap();
    assert_eq!(out, "echo:before\necho:after\n");

    // One start of the phase, one end: adoption added no running event of its own.
    let moves = transitions(&daemon, &k);
    assert_eq!(moves.len(), 3, "{moves:?}");
}

#[test]
fn a_phase_that_ended_while_the_daemon_was_down_keeps_its_code_and_totals_and_the_next_phase_starts()
 {
    let root = TempRoot::new();
    let first = key("r1", "p1");
    let second = key("r1", "p2");
    let mut cleanup = Cleanup::new();
    let paths = DaemonPaths::under(root.path());
    {
        let (daemon, _log) = start(&root);
        let phases = daemon.phases().clone();
        let script = "import sys, time\ntime.sleep(1.5)\nt = time.time()\nwhile time.time() - t < 0.2:\n    pass\nsys.exit(5)";
        phases
            .start_phase(&first, &python(script, root.path()))
            .unwrap();
        cleanup.track(&phases, &paths, &first);
        daemon.stop();
    }
    // No daemon runs. The child ends, the shim writes exit.json and has nobody to give it to.
    wait_until("exit.json is written", Duration::from_secs(15), || {
        dir_of(&paths, &first).join(files::EXIT_RECORD).exists()
    });

    // A daemon returns, with no app and no CLI, and a hook that stands in for the Stage 3b engine: when a phase ends
    // it starts the next one.
    let log = LogBuf::default();
    let mut opts = options(&root, &log);
    let next = second.clone();
    let cwd = root.path().to_path_buf();
    opts.on_phase_end = Some(Box::new(move |phases, end| {
        if end.key.phase == "p1" {
            phases
                .start_phase(&next, &python("import sys; sys.exit(0)", &cwd))
                .expect("the next phase starts");
        }
    }));
    let daemon = Daemon::start(opts).unwrap();
    let phases = daemon.phases().clone();
    assert!(
        phases
            .state(&first)
            .is_some_and(|s| matches!(s, PhaseState::Failed { cause: None })),
        "the first phase is settled by adoption: {:?}",
        phases.state(&first)
    );
    let record = phases.exit_record(&first).expect("the record was kept");
    assert_eq!(record.end, ExitEnd::Exited { code: 5 });
    assert!(record.cpu_user_us > 0, "{record:?}");

    wait_until("the next phase is known", Duration::from_secs(10), || {
        phases.state(&second).is_some()
    });
    cleanup.track(&phases, &paths, &second);
    wait_state(&phases, &second, "the next phase ends", ended);
    assert_eq!(
        phases.exit_record(&second).unwrap().end,
        ExitEnd::Exited { code: 0 }
    );
}

#[test]
fn adopting_the_same_ended_phase_again_records_nothing_more() {
    let root = TempRoot::new();
    let k = key("r1", "p1");
    let mut cleanup = Cleanup::new();
    {
        let (daemon, _log) = start(&root);
        let phases = daemon.phases().clone();
        phases
            .start_phase(&k, &python("import sys; sys.exit(0)", root.path()))
            .unwrap();
        cleanup.track(&phases, daemon.paths(), &k);
        wait_state(&phases, &k, "the phase ends", ended);
        wait_until("the shim has ended", Duration::from_secs(10), || {
            phases.shim_exit(&k).is_some()
        });
        assert_eq!(transitions(&daemon, &k).len(), 3);
    }
    for _ in 0..2 {
        let (daemon, _log) = start(&root);
        assert!(daemon.phases().state(&k).as_ref().is_some_and(ended));
        assert_eq!(transitions(&daemon, &k).len(), 3);
    }
}

// ---- a shim that stops answering ----------------------------------------------------------------------------

#[test]
fn a_stopped_shim_is_reported_unresponsive_and_a_second_run_is_not_stalled() {
    let root = TempRoot::new();
    let (daemon, _log) = start(&root);
    let phases = daemon.phases().clone();
    let stalled = key("r1", "p1");
    let other = key("r2", "p1");
    let mut cleanup = Cleanup::new();
    phases
        .start_phase(&stalled, &python(ECHO, root.path()))
        .unwrap();
    phases
        .start_phase(&other, &python(ECHO, root.path()))
        .unwrap();
    cleanup.track(&phases, daemon.paths(), &stalled);
    cleanup.track(&phases, daemon.paths(), &other);
    assert_eq!(phases.write(&stalled, "warm\n").unwrap(), 0);
    assert_eq!(phases.write(&other, "warm\n").unwrap(), 0);

    // SIGSTOP the first shim. The second run's shim and child go on.
    let pid = phases.shim_pid(&stalled).unwrap();
    // SAFETY: `kill` takes two integers; the pid is a shim this test's daemon started.
    unsafe { libc::kill(pid as libc::pid_t, libc::SIGSTOP) };

    // A write to the stopped shim waits for its answer for the reply wait, and then says the shim is not answering.
    let blocked = {
        let phases = phases.clone();
        let stalled = stalled.clone();
        std::thread::spawn(move || {
            let started = Instant::now();
            (phases.write(&stalled, "never\n"), started.elapsed())
        })
    };
    // Meanwhile the other run takes writes with no delay worth the name.
    std::thread::sleep(Duration::from_millis(100));
    let mut slowest = Duration::ZERO;
    let mut seq = 0;
    while !blocked.is_finished() {
        let started = Instant::now();
        let next = phases.write(&other, "go\n").unwrap();
        assert!(
            next > seq,
            "the sequence only goes forward: {next} after {seq}"
        );
        seq = next;
        slowest = slowest.max(started.elapsed());
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        seq > 5,
        "the other run was written to while the first was blocked: {seq}"
    );
    assert!(
        slowest < Duration::from_millis(250),
        "a write to the second run took {slowest:?} while the first shim was stopped"
    );
    let (result, took) = blocked.join().unwrap();
    assert!(matches!(result, Err(LinkError::Unresponsive)), "{result:?}");
    assert!(took >= Duration::from_millis(400), "{took:?}");

    // It is reported, to the daemon's status as well.
    wait_until(
        "the stopped shim is unresponsive",
        Duration::from_secs(5),
        || matches!(phases.health(&stalled), Some(Health::Unresponsive { .. })),
    );
    assert!(matches!(phases.health(&other), Some(Health::Responsive)));
    assert_eq!(phases.unresponsive(), vec![stalled.to_string()]);
    assert!(
        phases.state(&stalled).as_ref().is_some_and(running),
        "unresponsive is not lost"
    );

    // SIGCONT, and the shim answers again.
    // SAFETY: as above.
    unsafe { libc::kill(pid as libc::pid_t, libc::SIGCONT) };
    wait_until("the shim answers again", Duration::from_secs(10), || {
        matches!(phases.health(&stalled), Some(Health::Responsive))
    });
    assert!(phases.unresponsive().is_empty());
    phases
        .write(&stalled, "back\n")
        .expect("the shim takes writes again");
    phases.close_stdin(&stalled).unwrap();
    phases.close_stdin(&other).unwrap();
    wait_state(&phases, &stalled, "the first ends", ended);
    wait_state(&phases, &other, "the second ends", ended);
}

#[test]
fn a_stopped_shim_is_unresponsive_to_a_daemon_that_adopts_it_and_the_daemon_still_binds() {
    let root = TempRoot::new();
    let k = key("r1", "p1");
    let mut cleanup = Cleanup::new();
    let pid;
    {
        let (daemon, _log) = start(&root);
        let phases = daemon.phases().clone();
        phases
            .start_phase(&k, &python(WAIT_STDIN, root.path()))
            .unwrap();
        cleanup.track(&phases, daemon.paths(), &k);
        pid = phases.shim_pid(&k).unwrap();
        daemon.stop();
    }
    // SAFETY: as above.
    unsafe { libc::kill(pid as libc::pid_t, libc::SIGSTOP) };
    let started = Instant::now();
    let (daemon, _log) = start(&root);
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "a stopped shim held up the start: {:?}",
        started.elapsed()
    );
    let phases = daemon.phases().clone();
    assert!(phases.state(&k).as_ref().is_some_and(running));
    wait_until(
        "it is reported unresponsive",
        Duration::from_secs(15),
        || matches!(phases.health(&k), Some(Health::Unresponsive { .. })),
    );
    // SAFETY: as above.
    unsafe { libc::kill(pid as libc::pid_t, libc::SIGCONT) };
    wait_until("it answers again", Duration::from_secs(15), || {
        matches!(phases.health(&k), Some(Health::Responsive))
    });
    phases.close_stdin(&k).unwrap();
    wait_state(&phases, &k, "the phase ends", ended);
}

// ---- the stop of 0.5: a lost phase's child, and a run -----------------------------------------------------------
//
// SAFETY of these tests: the run id of each is unique to the test, the daemon's process table is a `Guarded` that
// panics before it signals a pid the test did not record (and logs the refusal, which each test asserts is empty),
// every child has a cleared environment but `PATH` and `COLLIDE_RUN`, and a guard kills what is left by pid and
// start time.

/// Kills what a test recorded when the test ends, however it ends.
struct KillOnDrop(Arc<Owned>);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        self.0.kill_all();
    }
}

/// A daemon whose phases stop through `guard`, with a lost-phase grace short enough for a test.
fn start_guarded(root: &TempRoot, guard: &Arc<Guarded>) -> (Daemon, LogBuf) {
    let log = LogBuf::default();
    let mut options = options(root, &log);
    options.processes = guard.clone();
    options.phases.lost_grace = Duration::from_millis(300);
    (Daemon::start(options).expect("the daemon starts"), log)
}

/// A phase whose child is the hostile tree of `common::procs::HOSTILE`, carrying `run` as its run id.
fn hostile_spec(root: &TempRoot, run: &str) -> (PhaseSpec, PathBuf) {
    hostile_spec_of(root, run, common::procs::HOSTILE)
}

fn hostile_spec_of(root: &TempRoot, run: &str, source: &str) -> (PhaseSpec, PathBuf) {
    let script = root.join(&format!("{run}.py"));
    fs::write(&script, source).unwrap();
    let pidfile = root.join(&format!("{run}.pids"));
    fs::write(&pidfile, "").unwrap();
    let spec = PhaseSpec {
        program: PYTHON.into(),
        args: vec![
            script.to_str().unwrap().into(),
            pidfile.to_str().unwrap().into(),
        ],
        env: BTreeMap::from([
            ("PATH".to_string(), "/usr/bin:/bin".to_string()),
            ("COLLIDE_RUN".to_string(), run.to_string()),
        ]),
        cwd: root.path().to_path_buf(),
        stdio: ChildStdio::Pipes,
    };
    (spec, pidfile)
}

/// Waits for the hostile tree to write its three pids, records them as the test's, and returns them.
fn recorded_hostile_pids(pidfile: &Path, owned: &Owned) -> Vec<u32> {
    let tags = ["root", "setsid", "orphan"];
    wait_until(
        "the hostile tree writes its pids",
        Duration::from_secs(20),
        || common::procs::read_pids(&pidfile.to_path_buf(), &tags).is_some(),
    );
    let pids = common::procs::read_pids(&pidfile.to_path_buf(), &tags).unwrap();
    for pid in pids.values() {
        owned.add(*pid);
    }
    pids.values().copied().collect()
}

/// The pid the fixture's root wrote as `root`.
fn pids_root(pidfile: &Path) -> u32 {
    common::procs::read_pids(&pidfile.to_path_buf(), &["root"]).expect("the root wrote its pid")["root"]
}

fn gone_by_ps(pids: &[u32]) {
    wait_until(
        &format!("ps lists none of {pids:?}"),
        Duration::from_secs(20),
        || common::procs::listed_by_ps(pids).is_empty(),
    );
}

#[test]
fn a_killed_shim_has_its_hostile_child_stopped_as_its_phase_turns_lost() {
    let root = TempRoot::new();
    let owned = Arc::new(Owned::default());
    let _kill = KillOnDrop(owned.clone());
    let guard = guard_of(&root, vec![owned.clone()]);
    let (daemon, log) = start_guarded(&root, &guard);
    let phases = daemon.phases();
    let k = key("hostile", "p1");
    // The child reads its stdin to the end and ignores hangup and terminate, so it is a child that ignores EOF.
    let (spec, pidfile) = hostile_spec_of(&root, &k.run, &common::procs::hostile_reading_stdin());
    let mut cleanup = Cleanup::new();
    phases.start_phase(&k, &spec).unwrap();
    cleanup.track(&phases, daemon.paths(), &k);
    let pids = recorded_hostile_pids(&pidfile, &owned);
    wait_state(&phases, &k, "running", running);
    assert!(
        common::procs::read_pids(&pidfile, &["eof"]).is_none(),
        "no EOF yet: the shim is alive and holds the child's stdin"
    );

    let shim = phases.shim_pid(&k).unwrap();
    // SAFETY: `kill` takes two integers; the pid is the shim this test's daemon started.
    unsafe { libc::kill(shim as libc::pid_t, libc::SIGKILL) };
    wait_state(&phases, &k, "the phase is lost", lost);
    gone_by_ps(&pids);
    guard.assert_nothing_refused();
    // The child saw the end of its input, and kept running: it wrote `eof` after the shim died and was still alive to
    // write it, so what ended it was the stop, not the EOF.
    let seen = common::procs::read_pids(&pidfile, &["eof"]).expect("the child saw EOF");
    assert_eq!(seen["eof"], pids_root(&pidfile), "the root wrote it");
    wait_until("the stop is logged", Duration::from_secs(10), || {
        log.text().contains("the stop of its lost child")
    });
}

#[test]
fn a_lost_phase_does_not_stop_the_live_phase_of_the_same_run() {
    let root = TempRoot::new();
    let owned = Arc::new(Owned::default());
    let _kill = KillOnDrop(owned.clone());
    let guard = guard_of(&root, vec![owned.clone()]);
    let (daemon, _log) = start_guarded(&root, &guard);
    let phases = daemon.phases();
    let lost_key = key("same", "p1");
    let live_key = key("same", "p2");
    let (spec, pidfile) = hostile_spec(&root, &lost_key.run);
    let mut cleanup = Cleanup::new();
    phases.start_phase(&lost_key, &spec).unwrap();
    cleanup.track(&phases, daemon.paths(), &lost_key);
    // The second phase of the same run carries the same run id: the marker scan would reach it.
    let mut sibling = python(WAIT_STDIN, root.path());
    sibling
        .env
        .insert("COLLIDE_RUN".to_string(), lost_key.run.clone());
    phases.start_phase(&live_key, &sibling).unwrap();
    cleanup.track(&phases, daemon.paths(), &live_key);
    let pids = recorded_hostile_pids(&pidfile, &owned);
    wait_state(&phases, &live_key, "running", running);

    let shim = phases.shim_pid(&lost_key).unwrap();
    // SAFETY: as above.
    unsafe { libc::kill(shim as libc::pid_t, libc::SIGKILL) };
    wait_state(&phases, &lost_key, "the phase is lost", lost);
    // With the marker scan off the stop reaches the child and what is below it: the root and its setsid child. The
    // double-forked orphan has parent pid 1 and is found only by the marker, so it stays until the test's guard ends it.
    let below_the_child =
        common::procs::read_pids(&pidfile, &["root", "setsid", "orphan"]).unwrap();
    gone_by_ps(&[below_the_child["root"], below_the_child["setsid"]]);
    let _ = pids;
    guard.assert_nothing_refused();
    assert!(
        phases.state(&live_key).as_ref().is_some_and(running),
        "the other phase of the run goes on"
    );
    phases.close_stdin(&live_key).unwrap();
    wait_state(&phases, &live_key, "the other phase ends", ended);
}

#[test]
fn stopping_a_run_leaves_zero_processes_and_the_phase_stopped() {
    let root = TempRoot::new();
    let owned = Arc::new(Owned::default());
    let _kill = KillOnDrop(owned.clone());
    let guard = guard_of(&root, vec![owned.clone()]);
    let (daemon, _log) = start_guarded(&root, &guard);
    let phases = daemon.phases();
    let k = key("stopme", "p1");
    let (spec, pidfile) = hostile_spec(&root, &k.run);
    let mut cleanup = Cleanup::new();
    phases.start_phase(&k, &spec).unwrap();
    cleanup.track(&phases, daemon.paths(), &k);
    let pids = recorded_hostile_pids(&pidfile, &owned);
    wait_state(&phases, &k, "running", running);

    let report = phases
        .stop_run(&k.run, Some(Duration::from_millis(300)))
        .expect("the run is known");
    guard.assert_nothing_refused();
    gone_by_ps(&pids);
    let mut signalled = report.signalled.clone();
    signalled.sort();
    let mut wanted = pids.clone();
    wanted.sort();
    assert_eq!(signalled, wanted, "the first scan found the whole tree");
    // What else runs on the machine is not the test's to judge: suspects are machine-wide, and a stop's report may
    // list some that are not the test's. What must hold is that nothing of the tree is a survivor.
    let unexpected: Vec<_> = report
        .survivors
        .iter()
        .filter(|s| s.kind != SurvivorKind::Suspect || pids.contains(&s.pid))
        .collect();
    assert!(unexpected.is_empty(), "{:?}", report.survivors);
    assert!(
        phases
            .state(&k)
            .as_ref()
            .is_some_and(|s| matches!(s, PhaseState::Stopped {})),
        "{:?}",
        phases.state(&k)
    );
    assert!(matches!(
        phases.run_state(&k.run),
        Some(collide_core::state::RunState::Stopped {})
    ));
    let moves = transitions(&daemon, &k);
    assert_eq!(moves.last().unwrap().1, PhaseState::Stopped {}, "{moves:?}");
}

#[test]
fn stopping_a_run_that_is_not_known_signals_nothing() {
    let root = TempRoot::new();
    let guard = guard_of(&root, Vec::new());
    let (daemon, _log) = start_guarded(&root, &guard);
    let error = daemon
        .phases()
        .stop_run(&unique_run("nobody"), Some(Duration::from_millis(100)))
        .expect_err("no such run");
    assert!(error.to_string().contains("no run"), "{error}");
    assert!(daemon.phases().stop_run("", None).is_err());
    guard.assert_nothing_refused();
}

#[test]
fn stopping_one_run_leaves_another_runs_process_running() {
    let root = TempRoot::new();
    let owned = Arc::new(Owned::default());
    let _kill = KillOnDrop(owned.clone());
    let guard = guard_of(&root, vec![owned.clone()]);
    let (daemon, _log) = start_guarded(&root, &guard);
    let phases = daemon.phases();
    let stopped = key("first", "p1");
    let other = key("second", "p1");
    let (spec, pidfile) = hostile_spec(&root, &stopped.run);
    let mut cleanup = Cleanup::new();
    phases.start_phase(&stopped, &spec).unwrap();
    cleanup.track(&phases, daemon.paths(), &stopped);
    let mut bystander = python(WAIT_STDIN, root.path());
    bystander
        .env
        .insert("COLLIDE_RUN".to_string(), other.run.clone());
    phases.start_phase(&other, &bystander).unwrap();
    cleanup.track(&phases, daemon.paths(), &other);
    let pids = recorded_hostile_pids(&pidfile, &owned);
    wait_state(&phases, &other, "running", running);

    phases
        .stop_run(&stopped.run, Some(Duration::from_millis(300)))
        .unwrap();
    gone_by_ps(&pids);
    guard.assert_nothing_refused();
    assert!(
        phases.state(&other).as_ref().is_some_and(running),
        "the other run was not stopped"
    );
    phases.close_stdin(&other).unwrap();
    wait_state(&phases, &other, "the other run ends by itself", ended);
}

#[test]
fn a_shim_the_daemon_starts_carries_the_runtime_marker_so_a_stop_never_sweeps_it() {
    let root = TempRoot::new();
    let (daemon, _log) = start(&root);
    let phases = daemon.phases();
    let k = key("marked", "p1");
    let mut cleanup = Cleanup::new();
    phases
        .start_phase(&k, &python(WAIT_STDIN, root.path()))
        .unwrap();
    cleanup.track(&phases, daemon.paths(), &k);
    let shim = phases.shim_pid(&k).unwrap();
    assert_eq!(
        collide_host::sysproc::env_vars_of(shim, &["COLLIDE_RUNTIME", "COLLIDE_RUN"]),
        collide_host::sysproc::EnvRead::Read(vec![Some("1".to_string()), None])
    );
    phases.close_stdin(&k).unwrap();
    wait_state(&phases, &k, "the phase ends", ended);
}
